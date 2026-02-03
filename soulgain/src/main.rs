pub mod types;
pub mod memory;
pub mod plasticity;
pub mod run;
pub mod evolution;

use types::UVal;
use memory::MemorySystem;
use plasticity::{Event, Plasticity, VMError};
use evolution::{Oracle, Trainer}; // Add this line
use std::sync::Arc;
// --- OPCODE DEFINITIONS ---
pub const OP_LITERAL: i64 = 0;
pub const OP_ADD: i64 = 1;
pub const OP_SUB: i64 = 2; // New: Subtraction
pub const OP_MUL: i64 = 3; // New: Multiplication
pub const OP_EQ: i64 = 5;  // New: Equality Check
pub const OP_STORE: i64 = 6;
pub const OP_LOAD: i64 = 7;
pub const OP_HALT: i64 = 8;
pub const OP_GT: i64 = 9;  // New: Greater Than
pub const OP_NOT: i64 = 10; // New: Logical Not
pub const OP_JMP: i64 = 11;
pub const OP_JMP_IF: i64 = 12;
pub const OP_CALL: i64 = 13;
pub const OP_RET: i64 = 14;
pub const OP_INTUITION: i64 = 15;
pub const OP_REWARD: i64 = 16;
pub const OP_EVOLVE: i64 = 17;

const BRAIN_PATH: &str = "brain_test.json";

pub struct SoulGainVM {
    pub stack: Vec<UVal>,
    pub call_stack: Vec<usize>,
    pub memory: MemorySystem,
    pub ip: usize,
    pub program: Vec<f64>,
    pub plasticity: Plasticity,
    last_event: Option<Event>,
}

impl SoulGainVM {
    pub fn new(program: Vec<f64>) -> Self {
        Self {
            stack: Vec::new(),
            call_stack: Vec::new(),
            memory: MemorySystem::new(),
            ip: 0,
            program,
            plasticity: Plasticity::new(),
            last_event: None,
        }
    }

    fn decode_opcode(x: f64) -> Result<i64, VMError> {
        if !x.is_finite() { return Err(VMError::InvalidOpcode(-1)); }
        let i = x.round();
        if (i - x).abs() > 1e-9 { return Err(VMError::InvalidOpcode(i as i64)); }
        Ok(i as i64)
    }

    pub fn run(&mut self) {
        while self.ip < self.program.len() {
            let raw = self.program[self.ip];
            self.ip += 1;

            let opcode = match Self::decode_opcode(raw) {
                Ok(op) => op,
                Err(e) => {
                    self.plasticity.observe(Event::Error(e));
                    continue;
                }
            };

            // Capture the event for STDP learning (Time-based now!)
            let opcode_event = Event::Opcode { opcode, stack_depth: self.stack.len() };
            self.last_event = Some(opcode_event);
            self.plasticity.observe(opcode_event);

            match opcode {
                OP_LITERAL => {
                    if self.ip >= self.program.len() { break; }
                    let v = self.program[self.ip];
                    self.ip += 1;
                    self.stack.push(UVal::Number(v));
                }

                OP_ADD => {
                    if self.stack.len() < 2 {
                        self.plasticity.observe(Event::Error(VMError::StackUnderflow));
                        continue;
                    }
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    
                    match (a, b) {
                        (UVal::Number(na), UVal::Number(nb)) => self.stack.push(UVal::Number(na + nb)),
                        (UVal::String(sa), UVal::String(sb)) => {
                            let mut new_s = (*sa).clone();
                            new_s.push_str(&sb);
                            self.stack.push(UVal::String(Arc::new(new_s)));
                        },
                        _ => self.plasticity.observe(Event::Error(VMError::InvalidOpcode(opcode))),
                    }
                }
                
                OP_SUB => {
                     if self.stack.len() < 2 { self.plasticity.observe(Event::Error(VMError::StackUnderflow)); continue; }
                     let b = self.stack.pop().unwrap();
                     let a = self.stack.pop().unwrap();
                     if let (UVal::Number(na), UVal::Number(nb)) = (a, b) {
                         self.stack.push(UVal::Number(na - nb));
                     }
                }

                OP_EQ => {
                    if self.stack.len() < 2 { self.plasticity.observe(Event::Error(VMError::StackUnderflow)); continue; }
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    self.stack.push(UVal::Bool(a == b));
                }

                OP_GT => {
                    if self.stack.len() < 2 { self.plasticity.observe(Event::Error(VMError::StackUnderflow)); continue; }
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    if let (UVal::Number(na), UVal::Number(nb)) = (a, b) {
                        self.stack.push(UVal::Bool(na > nb));
                    }
                }

                OP_STORE => {
                    if self.stack.len() < 2 { 
                        self.plasticity.observe(Event::Error(VMError::StackUnderflow));
                        continue; 
                    }
                    let val = self.stack.pop().unwrap();     // Can now be String/Object
                    let addr_val = self.stack.pop().unwrap(); // Address
                    
                    if let UVal::Number(addr) = addr_val {
                        if self.memory.write(addr, val) {
                            self.plasticity.observe(Event::MemoryWrite);
                            self.last_event = Some(Event::MemoryWrite);
                        }
                    } else {
                        self.plasticity.observe(Event::Error(VMError::InvalidOpcode(opcode)));
                    }
                }

                OP_LOAD => {
                    if self.stack.is_empty() {
                         self.plasticity.observe(Event::Error(VMError::StackUnderflow));
                         continue;
                    }
                    let addr_val = self.stack.pop().unwrap();
                    if let UVal::Number(addr) = addr_val {
                        if let Some(v) = self.memory.read(addr) {
                            self.stack.push(v);
                            self.plasticity.observe(Event::MemoryRead);
                            self.last_event = Some(Event::MemoryRead);
                        } else {
                            self.stack.push(UVal::Nil);
                        }
                    }
                }

                OP_INTUITION => {
                    if let Some(last_event) = self.last_event {
                        if let Some(next_event) = self.plasticity.best_next_event(last_event) {
                            if let Event::Opcode { opcode: predicted_opcode, .. } = next_event {
                                if let Some(new_ip) = self.find_next_opcode(predicted_opcode) {
                                    self.ip = new_ip;
                                }
                            }
                        }
                    }
                }

                OP_JMP => {
                    if self.ip >= self.program.len() { break; }
                    let target = self.program[self.ip];
                    self.ip += 1;
                    if !target.is_finite() || target < 0.0 {
                        self.plasticity.observe(Event::Error(VMError::InvalidJump(-1)));
                        continue;
                    }
                    let new_ip = target.round() as usize;
                    if new_ip >= self.program.len() {
                        self.plasticity.observe(Event::Error(VMError::InvalidJump(new_ip as i64)));
                        continue;
                    }
                    self.ip = new_ip;
                }

                OP_JMP_IF => {
                    if self.ip >= self.program.len() {
                        self.plasticity.observe(Event::Error(VMError::InvalidJump(-1)));
                        break;
                    }
                    if self.stack.is_empty() {
                        self.plasticity.observe(Event::Error(VMError::StackUnderflow));
                        continue;
                    }
                    let target = self.program[self.ip];
                    self.ip += 1;
                    let condition = self.stack.pop().unwrap();
                    if condition.is_truthy() {
                        if !target.is_finite() || target < 0.0 {
                            self.plasticity.observe(Event::Error(VMError::InvalidJump(-1)));
                            continue;
                        }
                        let new_ip = target.round() as usize;
                        if new_ip >= self.program.len() {
                            self.plasticity.observe(Event::Error(VMError::InvalidJump(new_ip as i64)));
                            continue;
                        }
                        self.ip = new_ip;
                    }
                }

                OP_CALL => {
                    if self.ip >= self.program.len() { break; }
                    let target = self.program[self.ip];
                    self.ip += 1;
                    if !target.is_finite() || target < 0.0 {
                        self.plasticity.observe(Event::Error(VMError::InvalidJump(-1)));
                        continue;
                    }
                    let new_ip = target.round() as usize;
                    if new_ip >= self.program.len() {
                        self.plasticity.observe(Event::Error(VMError::InvalidJump(new_ip as i64)));
                        continue;
                    }
                    self.call_stack.push(self.ip);
                    self.ip = new_ip;
                }

                OP_RET => {
                    if let Some(return_ip) = self.call_stack.pop() {
                        self.ip = return_ip;
                    } else {
                        self.plasticity.observe(Event::Error(VMError::ReturnStackUnderflow));
                    }
                }

                OP_REWARD => {
                    self.plasticity.observe(Event::Reward);
                    self.last_event = Some(Event::Reward);
                }

                OP_EVOLVE => {
                    if self.stack.len() < 2 {
                        self.plasticity.observe(Event::Error(VMError::StackUnderflow));
                        continue;
                    }
                    let value = self.stack.pop().unwrap();
                    let addr_val = self.stack.pop().unwrap();
                    let (addr, new_value) = match (addr_val, value) {
                        (UVal::Number(addr), UVal::Number(val)) => (addr, val),
                        _ => {
                            self.plasticity.observe(Event::Error(VMError::InvalidEvolve(-1)));
                            continue;
                        }
                    };
                    if !addr.is_finite() || addr < 0.0 {
                        self.plasticity.observe(Event::Error(VMError::InvalidEvolve(-1)));
                        continue;
                    }
                    let index = addr.round() as usize;
                    if index >= self.program.len() {
                        self.plasticity.observe(Event::Error(VMError::InvalidEvolve(index as i64)));
                        continue;
                    }
                    self.program[index] = new_value;
                }

                OP_HALT => break,
                
                _ => self.plasticity.observe(Event::Error(VMError::InvalidOpcode(opcode))),
            }
            
            // Metabolic decay
            self.plasticity.decay_long_term();
        }
    }

    fn find_next_opcode(&self, opcode: i64) -> Option<usize> {
        for (idx, raw) in self.program.iter().enumerate().skip(self.ip) {
            if let Ok(decoded) = Self::decode_opcode(*raw) {
                if decoded == opcode {
                    return Some(idx);
                }
            }
        }
        None
    }
}
use std::time::Instant;

struct AdditionGoal;
impl Oracle for AdditionGoal {
    fn evaluate(&self, _input: Vec<UVal>) -> Vec<UVal> {
        vec![UVal::Number(2.0)]
    }
}

struct SubtractionGoal;
impl Oracle for SubtractionGoal {
    fn evaluate(&self, _input: Vec<UVal>) -> Vec<UVal> {
        // Goal: 10 - 3 = 7
        vec![UVal::Number(7.0)]
    }
}

struct FinancialGoal;
impl Oracle for FinancialGoal {
    fn evaluate(&self, _input: Vec<UVal>) -> Vec<UVal> {
        vec![UVal::Number(60.0)]
    }
}
struct DeepAdditionGoal;

impl Oracle for DeepAdditionGoal {
    fn evaluate(&self, _input: Vec<UVal>) -> Vec<UVal> {
        // Goal: 1+1+1+1+1+1 = 6
        vec![UVal::Number(6.0)]
    }
}
fn main() {
    println!("SoulGain Curriculum: Deep Addition (5-step chain)");

    let mut vm = SoulGainVM::new(vec![]);
    let _ = vm.plasticity.load_from_file(BRAIN_PATH);
    let mut trainer = Trainer::new(vm, 10);

    let deep_inputs = vec![
        UVal::Number(1.0),
        UVal::Number(1.0),
        UVal::Number(1.0),
        UVal::Number(1.0),
        UVal::Number(1.0),
        UVal::Number(1.0),
    ];

    // RUN 1: Learning the chain
    println!("\n--- Deep Addition Run 1 (Cold) ---");
    let start1 = Instant::now();
    if let Some(program) = trainer.synthesize(&DeepAdditionGoal, deep_inputs.clone(), 20000) {
        println!("SUCCESS! Time: {:?}\nProgram: {:?}", start1.elapsed(), program);
    }

    // RUN 2: Testing the intuition
    println!("\n--- Deep Addition Run 2 (Guided) ---");
    let start2 = Instant::now();
    if let Some(program) = trainer.synthesize(&DeepAdditionGoal, deep_inputs, 20000) {
        println!("SUCCESS! Time: {:?}\nProgram: {:?}", start2.elapsed(), program);
    }

    // Print the "Deep" synapse
    println!("\n--- Brain Check ---");
    let deep_event = Event::Opcode { opcode: 1, stack_depth: 4 };
    if let Some(best) = trainer.vm.plasticity.best_next_event(deep_event) {
        println!("At depth 4, the brain now strongly suggests: {:?}", best);
    }

    let _ = trainer.vm.plasticity.save_to_file(BRAIN_PATH);
}