pub mod types;
pub mod vm;
pub mod memory;
pub mod plasticity;
pub mod evolution;

// FIX: Remove 'pub use vm::SoulGainVM;' because it is defined below!
pub use types::{UVal, SkillLibrary};
pub use memory::MemorySystem;
pub use plasticity::{Plasticity, Event, VMError};

// --- CORE OPCODES ---
pub const OP_LITERAL: i64 = 0;
pub const OP_ADD: i64 = 1;
pub const OP_SUB: i64 = 2;
pub const OP_MUL: i64 = 3;
pub const OP_LOAD: i64 = 4;
pub const OP_STORE: i64 = 5;
pub const OP_REWARD: i64 = 6;
pub const OP_EVOLVE: i64 = 7;
pub const OP_HALT: i64 = 8;
pub const OP_INTUITION: i64 = 9;

// The struct is defined HERE, so no need to import it from vm.rs
pub struct SoulGainVM {
    pub program: Vec<f64>,
    pub stack: Vec<UVal>,
    pub call_stack: Vec<usize>,
    pub ip: usize,
    pub memory: MemorySystem,
    pub plasticity: Plasticity,
    pub last_event: Option<Event>,
    pub skills: SkillLibrary,
}

impl SoulGainVM {
    pub fn new(program: Vec<f64>) -> Self {
        Self {
            program,
            stack: Vec::new(),
            call_stack: Vec::new(),
            ip: 0,
            memory: MemorySystem::new(),
            plasticity: Plasticity::new(),
            last_event: None,
            skills: SkillLibrary::new(),
        }
    }

    pub fn run(&mut self) {
        while self.ip < self.program.len() {
            let opcode = self.program[self.ip] as i64;
            if !self.step(opcode) {
                break;
            }
        }
    }

    pub fn step(&mut self, opcode: i64) -> bool {
        self.execute_opcode(opcode)
    }
}