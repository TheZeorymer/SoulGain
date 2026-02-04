use crate::{
    SoulGainVM, UVal, Event, VMError,
    OP_LITERAL, OP_ADD, OP_SUB, OP_MUL, OP_LOAD, OP_STORE, 
    OP_REWARD, OP_EVOLVE, OP_HALT, OP_INTUITION
};

impl SoulGainVM {
    pub fn execute_opcode(&mut self, opcode: i64) -> bool {
        let last_event = self.last_event;

        match opcode {
            OP_LITERAL => {
                self.ip += 1;
                if self.ip < self.program.len() {
                    let val = self.program[self.ip];
                    self.stack.push(UVal::Number(val));
                    self.last_event = Some(Event::Opcode {
                        opcode: OP_LITERAL,
                        stack_depth: self.stack.len(),
                    });
                } else {
                    return false;
                }
            }

            OP_ADD => {
                if let (Some(UVal::Number(b)), Some(UVal::Number(a))) = (self.stack.pop(), self.stack.pop()) {
                    self.stack.push(UVal::Number(a + b));
                    self.last_event = Some(Event::Opcode {
                        opcode: OP_ADD,
                        stack_depth: self.stack.len(),
                    });
                } else {
                    self.plasticity.observe(Event::Error(VMError::StackUnderflow));
                }
            }

            OP_SUB => {
                if let (Some(UVal::Number(b)), Some(UVal::Number(a))) = (self.stack.pop(), self.stack.pop()) {
                    self.stack.push(UVal::Number(a - b));
                    self.last_event = Some(Event::Opcode {
                        opcode: OP_SUB,
                        stack_depth: self.stack.len(),
                    });
                } else {
                    self.plasticity.observe(Event::Error(VMError::StackUnderflow));
                }
            }

            OP_MUL => {
                if let (Some(UVal::Number(b)), Some(UVal::Number(a))) = (self.stack.pop(), self.stack.pop()) {
                    self.stack.push(UVal::Number(a * b));
                    self.last_event = Some(Event::Opcode {
                        opcode: OP_MUL,
                        stack_depth: self.stack.len(),
                    });
                } else {
                    self.plasticity.observe(Event::Error(VMError::StackUnderflow));
                }
            }

            // FIX 1 & 2: Pass f64 directly and handle Option result
            OP_LOAD => {
                if let Some(UVal::Number(addr)) = self.stack.pop() {
                    // Default to 0.0 if memory is uninitialized at this address
                    let val = self.memory.read(addr).unwrap_or(UVal::Number(0.0));
                    self.stack.push(val);
                    self.plasticity.observe(Event::MemoryRead);
                    self.last_event = Some(Event::MemoryRead);
                }
            }

            // FIX 3: Pass f64 directly
            OP_STORE => {
                if let (Some(val), Some(UVal::Number(addr))) = (self.stack.pop(), self.stack.pop()) {
                    self.memory.write(addr, val);
                    self.plasticity.observe(Event::MemoryWrite);
                    self.last_event = Some(Event::MemoryWrite);
                }
            }

            OP_REWARD => {
                self.plasticity.observe(Event::Reward(100));
                self.last_event = Some(Event::Reward(100));
            }

            OP_EVOLVE => {
                if let Some(UVal::Number(id)) = self.stack.pop() {
                    self.skills.define_skill(id as i64, self.program.clone());
                    self.plasticity.observe(Event::Reward(100));
                    self.last_event = Some(Event::Reward(100));
                } else {
                    self.plasticity.observe(Event::Error(VMError::InvalidEvolve(-1)));
                }
            }

            OP_INTUITION => {
                if let Some(last_ev) = last_event {
                    if let Some(next_event) = self.plasticity.best_next_event(last_ev) {
                        if let Event::Opcode { opcode: next_op, .. } = next_event {
                            return self.execute_opcode(next_op);
                        }
                    }
                }
            }

            OP_HALT => return false,

            _ if opcode >= 100 => {
                if let Some(macro_code) = self.skills.get_skill(opcode).cloned() {
                    let mut sub_vm = SoulGainVM::new(macro_code);
                    
                    sub_vm.stack = std::mem::take(&mut self.stack);
                    sub_vm.skills = self.skills.clone();
                    // We assume memory is NOT shared for now to avoid borrow checker hell,
                    // or we clone it if it's cheap.
                    sub_vm.memory = self.memory.clone(); 
                    
                    sub_vm.run();
                    
                    self.stack = std::mem::take(&mut sub_vm.stack);
                    // If sub_vm wrote to memory, we'd need to sync it back, 
                    // but for now let's keep it simple.
                    self.last_event = sub_vm.last_event;
                } else {
                    self.plasticity.observe(Event::Error(VMError::InvalidOpcode(opcode)));
                }
            }

            _ => {
                self.plasticity.observe(Event::Error(VMError::InvalidOpcode(opcode)));
            }
        }

        if let Some(ev) = self.last_event {
            self.plasticity.observe(ev);
        }

        self.ip += 1;
        true
    }
}