use std::sync::Arc;

use crate::memory::MemorySystem;
use crate::plasticity::{Event, Plasticity, VMError};
use crate::types::{SkillLibrary, UVal};

pub const SKILL_OPCODE_BASE: i64 = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum Op {
    Literal = 0,
    Add = 1,
    Sub = 2,
    Mul = 3,
    Eq = 5,
    Store = 6,
    Load = 7,
    Halt = 8,
    Gt = 9,
    Not = 10,
    Jmp = 11,
    JmpIf = 12,
    Call = 13,
    Ret = 14,
    Intuition = 15,
    Reward = 16,
    Evolve = 17,
}

impl Op {
    pub fn from_i64(value: i64) -> Option<Self> {
        match value {
            0 => Some(Op::Literal),
            1 => Some(Op::Add),
            2 => Some(Op::Sub),
            3 => Some(Op::Mul),
            5 => Some(Op::Eq),
            6 => Some(Op::Store),
            7 => Some(Op::Load),
            8 => Some(Op::Halt),
            9 => Some(Op::Gt),
            10 => Some(Op::Not),
            11 => Some(Op::Jmp),
            12 => Some(Op::JmpIf),
            13 => Some(Op::Call),
            14 => Some(Op::Ret),
            15 => Some(Op::Intuition),
            16 => Some(Op::Reward),
            17 => Some(Op::Evolve),
            _ => None,
        }
    }

    pub fn as_i64(self) -> i64 {
        self as i64
    }

    pub fn as_f64(self) -> f64 {
        self as i64 as f64
    }
}

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

    fn decode_opcode(raw: f64) -> Result<i64, VMError> {
        if !raw.is_finite() {
            return Err(VMError::InvalidOpcode(-1));
        }
        let rounded = raw.round();
        if (rounded - raw).abs() > 1e-9 {
            return Err(VMError::InvalidOpcode(rounded as i64));
        }
        Ok(rounded as i64)
    }

    pub fn run(&mut self, max_cycles: usize) {
        let mut cycles = 0usize;
        while self.ip < self.program.len() && cycles < max_cycles {
            let raw = self.program[self.ip];
            self.ip += 1;
            cycles += 1;

            let opcode = match Self::decode_opcode(raw) {
                Ok(op) => op,
                Err(e) => {
                    self.plasticity.observe(Event::Error(e));
                    continue;
                }
            };

            if opcode >= SKILL_OPCODE_BASE {
                if !self.execute_skill(opcode, max_cycles.saturating_sub(cycles)) {
                    break;
                }
                continue;
            }

            match Op::from_i64(opcode) {
                Some(op) => {
                    if !self.execute_opcode(op) {
                        break;
                    }
                }
                None => self.plasticity.observe(Event::Error(VMError::InvalidOpcode(opcode))),
            }
        }
    }

    fn execute_skill(&mut self, opcode: i64, max_cycles: usize) -> bool {
        if let Some(macro_code) = self.skills.get_skill(opcode).cloned() {
            let mut sub_vm = SoulGainVM::new(macro_code);
            sub_vm.stack = std::mem::take(&mut self.stack);
            sub_vm.skills = self.skills.clone();
            sub_vm.memory = self.memory.clone();
            sub_vm.plasticity = self.plasticity.clone();
            sub_vm.run(max_cycles.max(1));
            self.stack = std::mem::take(&mut sub_vm.stack);
            self.last_event = sub_vm.last_event;
            true
        } else {
            self.plasticity.observe(Event::Error(VMError::InvalidOpcode(opcode)));
            true
        }
    }

    fn execute_opcode(&mut self, opcode: Op) -> bool {
        let opcode_event = Event::Opcode { opcode: opcode.as_i64(), stack_depth: self.stack.len() };
        self.last_event = Some(opcode_event);
        self.plasticity.observe(opcode_event);

        match opcode {
            Op::Literal => {
                if self.ip >= self.program.len() {
                    return false;
                }
                let v = self.program[self.ip];
                self.ip += 1;
                self.stack.push(UVal::Number(v));
            }
            Op::Add => {
                if self.stack.len() < 2 {
                    self.plasticity.observe(Event::Error(VMError::StackUnderflow));
                    return true;
                }
                let b = self.stack.pop().unwrap();
                let a = self.stack.pop().unwrap();
                match (a, b) {
                    (UVal::Number(na), UVal::Number(nb)) => self.stack.push(UVal::Number(na + nb)),
                    (UVal::String(sa), UVal::String(sb)) => {
                        let mut new_s = (*sa).clone();
                        new_s.push_str(&sb);
                        self.stack.push(UVal::String(Arc::new(new_s)));
                    }
                    _ => self.plasticity.observe(Event::Error(VMError::InvalidOpcode(opcode.as_i64()))),
                }
            }
            Op::Sub => {
                if self.stack.len() < 2 {
                    self.plasticity.observe(Event::Error(VMError::StackUnderflow));
                    return true;
                }
                let b = self.stack.pop().unwrap();
                let a = self.stack.pop().unwrap();
                if let (UVal::Number(na), UVal::Number(nb)) = (a, b) {
                    self.stack.push(UVal::Number(na - nb));
                }
            }
            Op::Mul => {
                if self.stack.len() < 2 {
                    self.plasticity.observe(Event::Error(VMError::StackUnderflow));
                    return true;
                }
                let b = self.stack.pop().unwrap();
                let a = self.stack.pop().unwrap();
                if let (UVal::Number(na), UVal::Number(nb)) = (a, b) {
                    self.stack.push(UVal::Number(na * nb));
                }
            }
            Op::Eq => {
                if self.stack.len() < 2 {
                    self.plasticity.observe(Event::Error(VMError::StackUnderflow));
                    return true;
                }
                let b = self.stack.pop().unwrap();
                let a = self.stack.pop().unwrap();
                self.stack.push(UVal::Bool(a == b));
            }
            Op::Gt => {
                if self.stack.len() < 2 {
                    self.plasticity.observe(Event::Error(VMError::StackUnderflow));
                    return true;
                }
                let b = self.stack.pop().unwrap();
                let a = self.stack.pop().unwrap();
                if let (UVal::Number(na), UVal::Number(nb)) = (a, b) {
                    self.stack.push(UVal::Bool(na > nb));
                }
            }
            Op::Not => {
                if let Some(val) = self.stack.pop() {
                    self.stack.push(UVal::Bool(!val.is_truthy()));
                } else {
                    self.plasticity.observe(Event::Error(VMError::StackUnderflow));
                }
            }
            Op::Store => {
                if self.stack.len() < 2 {
                    self.plasticity.observe(Event::Error(VMError::StackUnderflow));
                    return true;
                }
                let val = self.stack.pop().unwrap();
                let addr_val = self.stack.pop().unwrap();
                if let UVal::Number(addr) = addr_val {
                    if self.memory.write(addr, val) {
                        self.plasticity.observe(Event::MemoryWrite);
                        self.last_event = Some(Event::MemoryWrite);
                    }
                } else {
                    self.plasticity.observe(Event::Error(VMError::InvalidOpcode(opcode.as_i64())));
                }
            }
            Op::Load => {
                if let Some(UVal::Number(addr)) = self.stack.pop() {
                    if let Some(v) = self.memory.read(addr) {
                        self.stack.push(v);
                        self.plasticity.observe(Event::MemoryRead);
                        self.last_event = Some(Event::MemoryRead);
                    } else {
                        self.stack.push(UVal::Nil);
                    }
                } else {
                    self.plasticity.observe(Event::Error(VMError::StackUnderflow));
                }
            }
            Op::Intuition => {
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
            Op::Jmp => {
                if self.ip >= self.program.len() {
                    return false;
                }
                let target = self.program[self.ip];
                self.ip += 1;
                if !target.is_finite() || target < 0.0 {
                    self.plasticity.observe(Event::Error(VMError::InvalidJump(-1)));
                    return true;
                }
                let new_ip = target.round() as usize;
                if new_ip >= self.program.len() {
                    self.plasticity.observe(Event::Error(VMError::InvalidJump(new_ip as i64)));
                    return true;
                }
                self.ip = new_ip;
            }
            Op::JmpIf => {
                if self.ip >= self.program.len() {
                    self.plasticity.observe(Event::Error(VMError::InvalidJump(-1)));
                    return false;
                }
                if self.stack.is_empty() {
                    self.plasticity.observe(Event::Error(VMError::StackUnderflow));
                    return true;
                }
                let target = self.program[self.ip];
                self.ip += 1;
                let condition = self.stack.pop().unwrap();
                if condition.is_truthy() {
                    if !target.is_finite() || target < 0.0 {
                        self.plasticity.observe(Event::Error(VMError::InvalidJump(-1)));
                        return true;
                    }
                    let new_ip = target.round() as usize;
                    if new_ip >= self.program.len() {
                        self.plasticity.observe(Event::Error(VMError::InvalidJump(new_ip as i64)));
                        return true;
                    }
                    self.ip = new_ip;
                }
            }
            Op::Call => {
                if self.ip >= self.program.len() {
                    return false;
                }
                let target = self.program[self.ip];
                self.ip += 1;
                if !target.is_finite() || target < 0.0 {
                    self.plasticity.observe(Event::Error(VMError::InvalidJump(-1)));
                    return true;
                }
                let new_ip = target.round() as usize;
                if new_ip >= self.program.len() {
                    self.plasticity.observe(Event::Error(VMError::InvalidJump(new_ip as i64)));
                    return true;
                }
                self.call_stack.push(self.ip);
                self.ip = new_ip;
            }
            Op::Ret => {
                if let Some(return_ip) = self.call_stack.pop() {
                    self.ip = return_ip;
                } else {
                    self.plasticity.observe(Event::Error(VMError::ReturnStackUnderflow));
                }
            }
            Op::Reward => {
                self.plasticity.observe(Event::Reward(100));
                self.last_event = Some(Event::Reward(100));
            }
            Op::Evolve => {
                if let Some(UVal::Number(id)) = self.stack.pop() {
                    self.skills.define_skill(id as i64, self.program.clone());
                    self.plasticity.observe(Event::Reward(100));
                    self.last_event = Some(Event::Reward(100));
                } else {
                    self.plasticity.observe(Event::Error(VMError::InvalidEvolve(-1)));
                }
            }
            Op::Halt => return false,
        }

        if let Some(ev) = self.last_event {
            self.plasticity.observe(ev);
        }

        true
    }

    fn find_next_opcode(&self, target_opcode: i64) -> Option<usize> {
        self.program
            .iter()
            .enumerate()
            .find_map(|(idx, &raw)| {
                if raw == target_opcode as f64 {
                    Some(idx)
                } else {
                    None
                }
            })
    }
}
