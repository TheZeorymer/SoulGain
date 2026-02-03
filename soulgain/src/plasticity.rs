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
const TAU: f64 = 0.020;        // Time constant (20ms) - The "window of causality"
const NORMALIZATION_CAP: f64 = 5.0; // Max total weight output for a single neuron

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum VMError {
    StackUnderflow,
    InvalidOpcode(i64),
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
            let mut last_event_data: Option<(Event, Instant)> = None;

            // Process events as they arrive
            while let Ok((current_event, current_time)) = rx.recv() {
                // Acquire write lock to update the brain
                let mut mem = mem_clone.write().unwrap();

                if let Some((prev_event, prev_time)) = last_event_data {
                    // 1. CALCULATE DELTA T (in seconds)
                    let delta_t = current_time.duration_since(prev_time).as_secs_f64();

                    // 2. STDP EXPONENTIAL KERNEL
                    // We only care about events that happen within a causal window (e.g., 100ms)
                    if delta_t < 0.1 {
                        // Formula: A * e^(-dt / tau)
                        let weight_change = A_PLUS * (-delta_t / TAU).exp();
                        
                        let weight = mem.weights.entry((prev_event, current_event)).or_insert(0.0);
                        *weight += weight_change;
                    }

                    // 3. COMPETITIVE NORMALIZATION
                    // Ensure the total outgoing weight from 'prev_event' doesn't explode.
                    let mut sum = 0.0;
                    for ((from, _), w) in mem.weights.iter() {
                        if *from == prev_event { sum += *w; }
                    }

                    if sum > NORMALIZATION_CAP {
                        let factor = NORMALIZATION_CAP / sum;
                        for ((from, _), w) in mem.weights.iter_mut() {
                            if *from == prev_event { *w *= factor; }
                        }
                    }
                }

                // Update the trace
                last_event_data = Some((current_event, current_time));
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
}