use std::collections::HashMap;
use std::sync::{Arc, RwLock, mpsc};
use std::thread;
use std::time::Instant;
use std::path::Path;
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter};
use serde::{Deserialize, Serialize};

// --- CONSTANTS ---
const A_PLUS: f64 = 0.1;       
const A_MINUS: f64 = 0.12;     
const TAU: f64 = 0.020;        
const WINDOW_S: f64 = 0.1;     
const NORMALIZATION_CAP: f64 = 5.0; 
const REWARD_BOOST: f64 = 0.5; 

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum VMError {
    StackUnderflow,
    InvalidOpcode(i64),
    InvalidJump(i64),
    ReturnStackUnderflow,
    InvalidEvolve(i64),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Event {
    Opcode { opcode: i64, stack_depth: usize },
    MemoryRead,
    MemoryWrite,
    Reward(u8),
    Error(VMError),
}

#[derive(Clone, Debug)]
pub struct PersistentMemory {
    pub weights: HashMap<(Event, Event), f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WeightEntry {
    from: Event,
    to: Event,
    weight: f64,
}

impl PersistentMemory {
    pub fn new() -> Self {
        Self { weights: HashMap::new() }
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let file = OpenOptions::new().write(true).create(true).truncate(true).open(path)?;
        let entries: Vec<WeightEntry> = self.weights.iter().map(|((from, to), weight)| {
            WeightEntry { from: *from, to: *to, weight: *weight }
        }).collect();
        serde_json::to_writer_pretty(BufWriter::new(file), &entries)?;
        Ok(())
    }

    pub fn load_from_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::open(path)?;
        let entries: Vec<WeightEntry> = serde_json::from_reader(BufReader::new(file))?;
        let mut weights = HashMap::with_capacity(entries.len());
        for entry in entries {
            weights.insert((entry.from, entry.to), entry.weight);
        }
        Ok(Self { weights })
    }
}

pub struct Plasticity {
    sender: mpsc::Sender<(Event, Instant)>,
    pub memory: Arc<RwLock<PersistentMemory>>,
}

impl Plasticity {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<(Event, Instant)>();
        let memory = Arc::new(RwLock::new(PersistentMemory::new()));
        let mem_clone = memory.clone();

        thread::spawn(move || {
            let mut recent_events: Vec<(Event, Instant)> = Vec::new();

            while let Ok((current_event, current_time)) = rx.recv() {
                recent_events.retain(|(_, t)| {
                    current_time.duration_since(*t).as_secs_f64() < WINDOW_S
                });

                let mut mem = mem_clone.write().unwrap();

                for (past_event, past_time) in &recent_events {
                    let delta_t = current_time.duration_since(*past_time).as_secs_f64();
                    if delta_t <= 0.0 || delta_t >= WINDOW_S { continue; }

                    if let Event::Reward(intensity) = current_event {
                        let scale = intensity as f64 / 100.0;
                        if scale > 0.0 {
                            let reward_change = (REWARD_BOOST * scale) * (-delta_t / TAU).exp();
                            let reward_weight = mem.weights.entry((*past_event, current_event)).or_insert(0.0);
                            *reward_weight += reward_change;
                        }
                        continue;
                    }

                    let ltp_change = A_PLUS * (-delta_t / TAU).exp();
                    let ltp_weight = mem.weights.entry((*past_event, current_event)).or_insert(0.0);
                    *ltp_weight += ltp_change;

                    let ltd_change = A_MINUS * (-delta_t / TAU).exp();
                    let ltd_weight = mem.weights.entry((current_event, *past_event)).or_insert(0.0);
                    *ltd_weight -= ltd_change;

                    let mut sum = 0.0;
                    for ((from, _), w) in mem.weights.iter() {
                        if *from == *past_event { sum += *w; }
                    }
                    if sum > NORMALIZATION_CAP {
                        let factor = NORMALIZATION_CAP / sum;
                        for ((from, _), w) in mem.weights.iter_mut() {
                            if *from == *past_event { *w *= factor; }
                        }
                    }
                }
                recent_events.push((current_event, current_time));
            }
        });

        Self { sender: tx, memory }
    }

    pub fn observe(&self, event: Event) {
        let now = Instant::now();
        let _ = self.sender.send((event, now));
    }

    pub fn decay_long_term(&self) {
        if let Ok(mut mem) = self.memory.write() {
            for w in mem.weights.values_mut() { *w *= 0.999; }
        }
    }

    pub fn best_next_event(&self, from: Event) -> Option<Event> {
        let mem = self.memory.read().ok()?;
        mem.weights
            .iter()
            .filter(|((src, _), _)| *src == from)
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|((_, dst), _)| *dst)
    }

    // --- MISSING METHODS ADDED BELOW ---

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let mem = self.memory.read().map_err(|_| {
            io::Error::new(io::ErrorKind::Other, "plasticity lock poisoned")
        })?;
        mem.save_to_file(path)
    }

    pub fn load_from_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let loaded = PersistentMemory::load_from_file(path)?;
        let mut mem = self.memory.write().map_err(|_| {
            io::Error::new(io::ErrorKind::Other, "plasticity lock poisoned")
        })?;
        *mem = loaded;
        Ok(())
    }
}