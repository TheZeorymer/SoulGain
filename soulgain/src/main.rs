pub mod types;
pub mod memory;
pub mod plasticity;
pub mod run;

use types::UVal;
use memory::MemorySystem;
use plasticity::{Event, Plasticity, VMError};
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

pub struct SoulGainVM {
    pub stack: Vec<UVal>,
    pub memory: MemorySystem,
    pub ip: usize,
    pub program: Vec<f64>,
    pub plasticity: Plasticity,
}

impl SoulGainVM {
    pub fn new(program: Vec<f64>) -> Self {
        Self {
            stack: Vec::new(),
            memory: MemorySystem::new(),
            ip: 0,
            program,
            plasticity: Plasticity::new(),
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
            self.plasticity.observe(Event::Opcode { opcode, stack_depth: self.stack.len() });

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
                        } else {
                            self.stack.push(UVal::Nil);
                        }
                    }
                }

                OP_HALT => break,
                
                _ => self.plasticity.observe(Event::Error(VMError::InvalidOpcode(opcode))),
            }
            
            // Metabolic decay
            self.plasticity.decay_long_term();
        }
    }
}
fn main() {
    println!("SoulGain substrate (STDP Enabled) running.");
    
    run::test_numeric_logic();
    run::test_string_concatenation();
    run::test_boolean_logic();      // New test
    run::test_memory_persistence(); // New test
}