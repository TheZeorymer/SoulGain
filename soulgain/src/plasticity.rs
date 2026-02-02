use std::collections::HashMap;
use std::time::{Instant, Duration}; // We need time, not just counts
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter};
use std::path::Path;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum VMError {
    StackUnderflow,
    InvalidOpcode(i64),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Event {
    Opcode {
        opcode: i64,
        stack_depth: usize,
    },
    MemoryRead,
    MemoryWrite,
    Error(VMError),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistentMemory {
    // REPLACED: 'counts' is deleted. We only care about the weight of the connection.
    pub weights: HashMap<(Event, Event), f64>,
}

impl PersistentMemory {
    pub fn new() -> Self {
        Self {
            weights: HashMap::new(),
        }
    }

    /// The STDP Rule: The core logic of your new substrate.
    /// delta_t is in milliseconds.
    /// - Positive delta_t (Pre -> Post) = Potentiation (Strengthen)
    /// - Negative delta_t (Post -> Pre) = Depression (Weaken)
    pub fn apply_stdp(&mut self, from: Event, to: Event, delta_t: f64) {
        let weight = self.weights.entry((from, to)).or_insert(0.0);
        
        // Biological constants adapted for the VM
        let a_plus = 0.1;   // Max learning rate
        let a_minus = 0.12; // Max forgetting rate (slightly higher to prune bad logic)
        let tau = 20.0;     // Time window (20ms)

        if delta_t > 0.0 {
            // Causal Link: "From" happened before "To"
            *weight += a_plus * (-delta_t / tau).exp();
        } else {
            // Anti-Causal Link: "To" happened before "From" (or too late)
            // We actively punish this connection.
            *weight -= a_minus * (delta_t / tau).exp();
        }
        
        // Clamp weights to keep the system stable (-1.0 to 1.0)
        *weight = weight.clamp(-1.0, 1.0);
    }

    pub fn decay(&mut self, rate: f64) {
        for v in self.weights.values_mut() {
            *v *= rate;
        }
    }

    // Standard save/load boilerplate remains the same
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
    // REPLACED: 'short_counts' is gone.
    // We now use a 'registry' to track the EXACT TIME an event last happened.
    // This is the "Short Term Memory" trace.
    pub registry: HashMap<Event, Instant>,
    
    pub memory: PersistentMemory,
    pub long_decay: f64,
}

impl Plasticity {
    pub fn new() -> Self {
        Self {
            registry: HashMap::new(), // Stores ephemeral timing traces
            memory: PersistentMemory::new(),
            long_decay: 0.999,
        }
    }

    pub fn observe(&mut self, current_event: Event) {
        let now = Instant::now();

        // 1. Iterate through recent history (the registry)
        // We look for any event that happened within the last 100ms
        let mut updates = Vec::new();
        
        for (prev_event, &prev_time) in &self.registry {
            if prev_event == &current_event { continue; } // Don't link event to itself immediately

            // Calculate the time difference in milliseconds
            let duration = now.duration_since(prev_time).as_secs_f64() * 1000.0;

            // The STDP Window: Only learn if events are close in time (< 100ms)
            if duration < 100.0 {
                // We found a pair! Record it to update weights.
                updates.push((*prev_event, duration));
            }
        }

        // 2. Apply the STDP rule to the connections we found
        for (prev_event, duration) in updates {
            self.memory.apply_stdp(prev_event, current_event, duration);
        }

        // 3. Update the registry with the new event's timestamp
        self.registry.insert(current_event, now);
        
        // 4. Cleanup: Remove old traces from registry to prevent memory leaks
        // (Optional performance optimization for later, but good to keep in mind)
    }

    pub fn predict_next(&self, current: Event) -> Option<Event> {
        // Prediction now returns the event with the strongest CAUSAL weight
        self.memory.weights
            .iter()
            .filter(|((from, _), _)| *from == current)
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|((_, to), _)| *to)
    }

    pub fn decay_long_term(&mut self) {
        self.memory.decay(self.long_decay);
    }
}