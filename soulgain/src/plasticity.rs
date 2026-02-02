use std::collections::HashMap;
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
    pub counts: HashMap<(Event, Event), u64>,
    pub weights: HashMap<(Event, Event), f64>,
}

impl PersistentMemory {
    pub fn new() -> Self {
        Self {
            counts: HashMap::new(),
            weights: HashMap::new(),
        }
    }

    pub fn strengthen(&mut self, from: Event, to: Event) {
        let c = self.counts.entry((from, to)).or_insert(0);
        *c += 1;
        self.weights.insert((from, to), (*c as f64).ln_1p());
    }

    pub fn decay(&mut self, rate: f64) {
        for v in self.weights.values_mut() {
            *v *= rate;
        }
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        serde_json::to_writer_pretty(BufWriter::new(file), self)?;
        Ok(())
    }

    pub fn load_from_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::open(path)?;
        Ok(serde_json::from_reader(BufReader::new(file))?)
    }
}

pub struct Plasticity {
    last_event: Option<Event>,
    short_counts: HashMap<(Event, Event), u64>,
    pub memory: PersistentMemory,
    pub short_decay: f64,
    pub long_decay: f64,
    pub consolidate_after: u64,
}

impl Plasticity {
    pub fn new() -> Self {
        Self {
            last_event: None,
            short_counts: HashMap::new(),
            memory: PersistentMemory::new(),
            short_decay: 0.9,
            long_decay: 0.999,
            consolidate_after: 3,
        }
    }

    pub fn observe(&mut self, event: Event) {
        if let Some(prev) = self.last_event {
            let c = self.short_counts.entry((prev, event)).or_insert(0);
            *c += 1;

            if *c >= self.consolidate_after {
                self.memory.strengthen(prev, event);
            }
        }

        // decay short-term
        self.short_counts.retain(|_, v| {
            *v = (*v as f64 * self.short_decay) as u64;
            *v > 0
        });

        self.last_event = Some(event);
    }

    pub fn predict_next(&self, current: Event) -> Option<Event> {
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
