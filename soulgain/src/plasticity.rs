use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter};
use std::path::Path;
use std::sync::{Arc, RwLock, mpsc};
use std::thread;
use std::time::{Duration, Instant};

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
    Context(u8),
    MemoryRead,
    MemoryWrite,
    Reward(u8),
    Error(VMError),
}

#[derive(Clone, Debug)]
pub struct PersistentMemory {
    pub weights: HashMap<Event, HashMap<Event, f64>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WeightEntry {
    from: Event,
    to: Event,
    weight: f64,
}

impl PersistentMemory {
    pub fn new() -> Self {
        Self {
            weights: HashMap::new(),
        }
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        let entries: Vec<WeightEntry> = self
            .weights
            .iter()
            .flat_map(|(from, outgoing)| {
                outgoing.iter().map(|(to, weight)| WeightEntry {
                    from: *from,
                    to: *to,
                    weight: *weight,
                })
            })
            .collect();
        serde_json::to_writer_pretty(BufWriter::new(file), &entries)?;
        Ok(())
    }

    pub fn load_from_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::open(path)?;
        let entries: Vec<WeightEntry> = serde_json::from_reader(BufReader::new(file))?;
        let mut weights: HashMap<Event, HashMap<Event, f64>> = HashMap::new();
        for entry in entries {
            weights
                .entry(entry.from)
                .or_insert_with(HashMap::new)
                .insert(entry.to, entry.weight);
        }
        Ok(Self { weights })
    }
}

#[derive(Clone)]
pub struct Plasticity {
    sender: mpsc::Sender<PlasticityMessage>,
    pub memory: Arc<RwLock<PersistentMemory>>,
}

enum PlasticityMessage {
    Single(Event, Instant),
    Batch(Vec<Event>),
}

impl Plasticity {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<PlasticityMessage>();
        let memory = Arc::new(RwLock::new(PersistentMemory::new()));
        let mem_clone = memory.clone();

        thread::spawn(move || {
            let mut recent_events: Vec<(Event, Instant)> = Vec::new();

            // The closure now correctly iterates over recent_events using .iter()
            // In src/plasticity.rs

            // Find this line (around line 105):
            // let mut process_event = |current_event: Event, current_time: Instant, recent_events: &mut Vec<(Event, Instant)>| {

            // Change it to (remove 'mut'):
            let process_event =
                |current_event: Event,
                 current_time: Instant,
                 recent_events: &mut Vec<(Event, Instant)>| {
                    recent_events
                        .retain(|(_, t)| current_time.duration_since(*t).as_secs_f64() < WINDOW_S);

                    let mut updates: Vec<(Event, Event, f64)> = Vec::new();
                    let mut normalize_sources: HashSet<Event> = HashSet::new();

                    // FIX: Use .iter() to iterate over the vector immutably
                    for (past_event, past_time) in recent_events.iter() {
                        let delta_t = current_time.duration_since(*past_time).as_secs_f64();
                        // Basic sanity check for time
                        if delta_t <= 0.0 || delta_t >= WINDOW_S {
                            continue;
                        }

                        match current_event {
                            Event::Reward(intensity) => {
                                let scale = intensity as f64 / 100.0;
                                if scale > 0.0 {
                                    let reward_change =
                                        (REWARD_BOOST * scale) * (-delta_t / TAU).exp();
                                    updates.push((*past_event, current_event, reward_change));
                                    normalize_sources.insert(*past_event);
                                }
                                continue;
                            }
                            Event::Error(_) => {
                                let penalty = -REWARD_BOOST * (-delta_t / TAU).exp();
                                updates.push((*past_event, current_event, penalty));
                                normalize_sources.insert(*past_event);
                                continue;
                            }
                            _ => {}
                        }

                        // STDP Rules
                        let ltp_change = A_PLUS * (-delta_t / TAU).exp();
                        updates.push((*past_event, current_event, ltp_change));

                        let ltd_change = A_MINUS * (-delta_t / TAU).exp();
                        updates.push((current_event, *past_event, -ltd_change));

                        normalize_sources.insert(*past_event);
                    }

                    // Apply Updates
                    if !updates.is_empty() {
                        let mut mem = mem_clone.write().unwrap();
                        for (from, to, delta) in updates {
                            let weight = mem
                                .weights
                                .entry(from)
                                .or_insert_with(HashMap::new)
                                .entry(to)
                                .or_insert(0.0);
                            *weight += delta;
                        }

                        // Normalize weights to prevent explosion
                        for past_event in normalize_sources {
                            let mut sum = 0.0;
                            for (from, outgoing) in mem.weights.iter() {
                                if *from == past_event {
                                    for (_to, w) in outgoing.iter() {
                                        sum += *w;
                                    }
                                }
                            }
                            if sum > NORMALIZATION_CAP {
                                let factor = NORMALIZATION_CAP / sum;
                                for (from, outgoing) in mem.weights.iter_mut() {
                                    if *from == past_event {
                                        for w in outgoing.values_mut() {
                                            *w *= factor;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    recent_events.push((current_event, current_time));
                };

            while let Ok(message) = rx.recv() {
                match message {
                    PlasticityMessage::Single(event, time) => {
                        process_event(event, time, &mut recent_events);
                    }
                    PlasticityMessage::Batch(events) => {
                        if events.is_empty() {
                            continue;
                        }
                        let now = Instant::now();
                        let len = events.len();

                        // Spread batch events over the window to simulate sequence
                        let step = if len > 1 {
                            WINDOW_S / (len as f64)
                        } else {
                            0.0
                        };

                        for (idx, event) in events.into_iter().enumerate() {
                            // Calculate a simulated past time for this event
                            let offset = (len - 1 - idx) as f64 * step;
                            let event_time = if offset > 0.0 {
                                now - Duration::from_secs_f64(offset)
                            } else {
                                now
                            };
                            process_event(event, event_time, &mut recent_events);
                        }
                    }
                }
            }
        });

        Self { sender: tx, memory }
    }

    pub fn observe(&self, event: Event) {
        let now = Instant::now();
        let _ = self.sender.send(PlasticityMessage::Single(event, now));
    }

    pub fn observe_batch(&self, events: Vec<Event>) {
        let _ = self.sender.send(PlasticityMessage::Batch(events));
    }

    pub fn decay_long_term(&self) {
        if let Ok(mut mem) = self.memory.write() {
            for outgoing in mem.weights.values_mut() {
                for w in outgoing.values_mut() {
                    *w *= 0.999;
                }
            }
        }
    }

    pub fn best_next_event(&self, from: Event) -> Option<Event> {
        let mem = self.memory.read().ok()?;
        mem.weights.get(&from).and_then(|outgoing| {
            outgoing
                .iter()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(dst, _)| *dst)
        })
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let mem = self
            .memory
            .read()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "plasticity lock poisoned"))?;
        mem.save_to_file(path)
    }

    pub fn load_from_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let loaded = PersistentMemory::load_from_file(path)?;
        let mut mem = self
            .memory
            .write()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "plasticity lock poisoned"))?;
        *mem = loaded;
        Ok(())
    }
}
