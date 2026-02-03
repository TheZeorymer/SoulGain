use std::collections::HashMap;
use std::sync::{Arc, RwLock, mpsc};
use std::thread;
use std::time::Instant; // Removed unused 'Duration'
use std::path::Path;
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter};
use serde::{Deserialize, Serialize};

// --- CONSTANTS FOR BIOLOGICAL TUNING ---
const A_PLUS: f64 = 0.1;       // Max synaptic strengthening (LTP)
const A_MINUS: f64 = 0.12;     // Max synaptic weakening (LTD)
const TAU: f64 = 0.020;        // Time constant (20ms) - The "window of causality"
const WINDOW_S: f64 = 0.1;     // Timing window for pair-based STDP
const NORMALIZATION_CAP: f64 = 5.0; // Max total weight output for a single neuron

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum VMError {
    StackUnderflow,
    InvalidOpcode(i64),
    InvalidJump(f64),
    ReturnStackUnderflow,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Event {
    Opcode { opcode: i64, stack_depth: usize },
    MemoryRead,
    MemoryWrite,
    Error(VMError),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistentMemory {
    pub weights: HashMap<(Event, Event), f64>,
}

impl PersistentMemory {
    pub fn new() -> Self {
        Self { weights: HashMap::new() }
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let file = OpenOptions::new().write(true).create(true).truncate(true).open(path)?;
        serde_json::to_writer_pretty(BufWriter::new(file), self)?;
        Ok(())
    }

    pub fn load_from_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::open(path)?;
        Ok(serde_json::from_reader(BufReader::new(file))?)
    }
}

pub struct Plasticity {
    // ASYNC ARCHITECTURE: We send events to a worker thread
    sender: mpsc::Sender<(Event, Instant)>,
    
    // SHARED STATE: The VM execution runs on the main thread, 
    // but the weights are updated by the worker. We use RwLock for safety.
    pub memory: Arc<RwLock<PersistentMemory>>,
}

impl Plasticity {
    pub fn new() -> Self {
        // FIX: Explicitly tell Rust this channel carries (Event, Instant)
        let (tx, rx) = mpsc::channel::<(Event, Instant)>();
        
        let memory = Arc::new(RwLock::new(PersistentMemory::new()));
        
        // Clone the Arc to pass to the worker thread
        let mem_clone = memory.clone();

        // --- THE STDP WORKER THREAD ---
        thread::spawn(move || {
            let mut recent_events: Vec<(Event, Instant)> = Vec::new();

            // Process events as they arrive
            while let Ok((current_event, current_time)) = rx.recv() {
                // Drop old events outside the STDP window
                recent_events.retain(|(_, t)| {
                    current_time.duration_since(*t).as_secs_f64() < WINDOW_S
                });

                // Acquire write lock to update the brain
                let mut mem = mem_clone.write().unwrap();

                for (past_event, past_time) in &recent_events {
                    let delta_t = current_time.duration_since(*past_time).as_secs_f64();
                    if delta_t <= 0.0 || delta_t >= WINDOW_S {
                        continue;
                    }

                    // LTP: past -> current
                    let ltp_change = A_PLUS * (-delta_t / TAU).exp();
                    let ltp_weight = mem.weights.entry((*past_event, current_event)).or_insert(0.0);
                    *ltp_weight += ltp_change;

                    // LTD: current -> past (anti-causal pairing)
                    let ltd_change = A_MINUS * (-delta_t / TAU).exp();
                    let ltd_weight = mem.weights.entry((current_event, *past_event)).or_insert(0.0);
                    *ltd_weight -= ltd_change;

                    // Competitive normalization for past_event outgoing weights
                    let mut sum = 0.0;
                    for ((from, _), w) in mem.weights.iter() {
                        if *from == *past_event {
                            sum += *w;
                        }
                    }

                    if sum > NORMALIZATION_CAP {
                        let factor = NORMALIZATION_CAP / sum;
                        for ((from, _), w) in mem.weights.iter_mut() {
                            if *from == *past_event {
                                *w *= factor;
                            }
                        }
                    }
                }

                // Update the trace with the current event
                recent_events.push((current_event, current_time));
            }
        });

        Self {
            sender: tx,
            memory,
        }
    }

    pub fn observe(&self, event: Event) {
        // Capture the EXACT timestamp on the main thread
        let now = Instant::now();
        // Fire and forget - don't block execution
        let _ = self.sender.send((event, now));
    }

    pub fn decay_long_term(&self) {
        // Main thread can occasionally trigger a decay pass
        if let Ok(mut mem) = self.memory.write() {
            for w in mem.weights.values_mut() {
                *w *= 0.999;
            }
        }
    }

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
