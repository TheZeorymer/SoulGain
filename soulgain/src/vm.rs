use std::collections::VecDeque;
use std::sync::Arc;

use crate::intuition::{IntuitionEngine, SkillOutcome};
use crate::logic::{decode_ops_for_validation, logic_of, validate_ops};
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
    Swap = 18,
    Dup = 19,
    Over = 20,
    Drop = 21,
    And = 22,
    Or = 23,
    Xor = 24,
    IsZero = 25,
    Mod = 26,
    Inc = 27,
    Dec = 28,
    Parse = 29,
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
            18 => Some(Op::Swap),
            19 => Some(Op::Dup),
            20 => Some(Op::Over),
            21 => Some(Op::Drop),
            22 => Some(Op::And),
            23 => Some(Op::Or),
            24 => Some(Op::Xor),
            25 => Some(Op::IsZero),
            26 => Some(Op::Mod),
            27 => Some(Op::Inc),
            28 => Some(Op::Dec),
            29 => Some(Op::Parse),
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
    program_stack: Vec<ProgramFrame>,
    pub ip: usize,
    pub memory: MemorySystem,
    pub plasticity: Plasticity,
    pub last_event: Option<Event>,
    pub skills: SkillLibrary,
    pub intuition: IntuitionEngine,
    trace: Vec<Event>,
    recent_opcodes: VecDeque<i64>,
    tick: u64,
    total_reward: f64,
    error_count: u64,
}

#[derive(Debug)]
struct ProgramFrame {
    program: Vec<f64>,
    ip: usize,
    skill_invocation: Option<SkillInvocation>,
}

#[derive(Debug, Clone, Copy)]
struct SkillInvocation {
    skill_id: i64,
    reward_before: f64,
    errors_before: u64,
    expected_depth_min: u8,
}

impl SoulGainVM {
    pub fn new(program: Vec<f64>) -> Self {
        Self {
            program,
            stack: Vec::with_capacity(256),
            call_stack: Vec::new(),
            program_stack: Vec::new(),
            ip: 0,
            memory: MemorySystem::new(),
            plasticity: Plasticity::new(),
            last_event: None,
            skills: SkillLibrary::new(),
            intuition: IntuitionEngine::default(),
            trace: Vec::with_capacity(512),
            recent_opcodes: VecDeque::with_capacity(8),
            tick: 0,
            total_reward: 0.0,
            error_count: 0,
        }
    }

    #[inline(always)]
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

    fn record_event(&mut self, event: Event) {
        self.last_event = Some(event);
        self.trace.push(event);
    }

    fn record_error(&mut self, error: VMError) {
        self.error_count = self.error_count.saturating_add(1);
        self.record_event(Event::Error(error));
        self.flush_trace();
    }

    fn flush_trace(&mut self) {
        if self.trace.is_empty() {
            return;
        }
        let batch = std::mem::take(&mut self.trace);
        self.plasticity.observe_batch(batch);
    }

    fn restore_program(&mut self) -> bool {
        if let Some(frame) = self.program_stack.pop() {
            if let Some(invocation) = frame.skill_invocation {
                let success = self.error_count == invocation.errors_before;
                let reward_delta = self.total_reward - invocation.reward_before;
                let stack_match_after = self.stack.len() >= invocation.expected_depth_min as usize;
                self.intuition.update_after_execution(
                    invocation.skill_id,
                    SkillOutcome {
                        success,
                        reward_delta,
                        stack_match_after,
                        used_tick: self.tick,
                    },
                );
            }
            self.program = frame.program;
            self.ip = frame.ip;
            true
        } else {
            false
        }
    }

    pub fn run(&mut self, max_cycles: usize) {
        let mut cycles = 0usize;
        while cycles < max_cycles {
            if self.ip >= self.program.len() {
                if self.restore_program() {
                    continue;
                }
                self.flush_trace();
                break;
            }
            let raw = unsafe { *self.program.get_unchecked(self.ip) };
            self.ip += 1;
            cycles += 1;
            self.tick = self.tick.saturating_add(1);

            let opcode = match Self::decode_opcode(raw) {
                Ok(op) => op,
                Err(e) => {
                    self.record_error(e);
                    continue;
                }
            };

            if opcode >= SKILL_OPCODE_BASE {
                let opcode_event = Event::Opcode {
                    opcode,
                    stack_depth: self.stack.len(),
                };
                self.record_event(opcode_event);
                self.push_recent_opcode(opcode);
                self.execute_skill(opcode);
                continue;
            }

            match Op::from_i64(opcode) {
                Some(op) => {
                    if !self.execute_opcode(op) {
                        break;
                    }
                }
                None => self.record_error(VMError::InvalidOpcode(opcode)),
            }
        }
    }

    fn execute_skill(&mut self, opcode: i64) {
        if let Some(macro_code) = self.skills.get_skill(opcode).cloned() {
            let ctx = self
                .intuition
                .build_context(&self.stack, &self.recent_opcodes);
            self.intuition.bootstrap_pattern_if_empty(opcode, &ctx);
            let expected_depth_min = self
                .intuition
                .skill_meta
                .get(&opcode)
                .map(|m| m.pattern.min_depth)
                .unwrap_or(0);

            let frame = ProgramFrame {
                program: std::mem::take(&mut self.program),
                ip: self.ip,
                skill_invocation: Some(SkillInvocation {
                    skill_id: opcode,
                    reward_before: self.total_reward,
                    errors_before: self.error_count,
                    expected_depth_min,
                }),
            };
            self.program_stack.push(frame);
            self.program = macro_code;
            self.ip = 0;
        } else {
            self.record_error(VMError::InvalidOpcode(opcode));
        }
    }

    fn push_recent_opcode(&mut self, opcode: i64) {
        if self.recent_opcodes.len() >= 6 {
            let _ = self.recent_opcodes.pop_front();
        }
        self.recent_opcodes.push_back(opcode);
    }

    #[inline(always)]
    fn execute_opcode(&mut self, opcode: Op) -> bool {
        let info = logic_of(opcode);
        if info.stack_delta < 0 && self.stack.len() < (-info.stack_delta) as usize {
            self.record_error(VMError::StackUnderflow);
            return true;
        }

        let opcode_event = Event::Opcode {
            opcode: opcode.as_i64(),
            stack_depth: self.stack.len(),
        };
        self.record_event(opcode_event);
        self.push_recent_opcode(opcode.as_i64());

        match opcode {
            Op::Literal => {
                if self.ip >= self.program.len() {
                    return false;
                }
                let v = unsafe { *self.program.get_unchecked(self.ip) };
                self.ip += 1;
                self.stack.push(UVal::Number(v));
            }
            Op::Add => {
                if self.stack.len() < 2 {
                    self.record_error(VMError::StackUnderflow);
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
                    _ => self.record_error(VMError::InvalidOpcode(opcode.as_i64())),
                }
            }
            Op::Sub => {
                if self.stack.len() < 2 {
                    self.record_error(VMError::StackUnderflow);
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
                    self.record_error(VMError::StackUnderflow);
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
                    self.record_error(VMError::StackUnderflow);
                    return true;
                }
                let b = self.stack.pop().unwrap();
                let a = self.stack.pop().unwrap();
                self.stack.push(UVal::Bool(a == b));
            }
            Op::Gt => {
                if self.stack.len() < 2 {
                    self.record_error(VMError::StackUnderflow);
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
                    self.record_error(VMError::StackUnderflow);
                }
            }
            Op::Store => {
                if self.stack.len() < 2 {
                    self.record_error(VMError::StackUnderflow);
                    return true;
                }
                let val = self.stack.pop().unwrap();
                let addr_val = self.stack.pop().unwrap();
                if let UVal::Number(addr) = addr_val {
                    if self.memory.write(addr, val) {
                        self.record_event(Event::MemoryWrite);
                    }
                } else {
                    self.record_error(VMError::InvalidOpcode(opcode.as_i64()));
                }
            }
            Op::Load => {
                if let Some(UVal::Number(addr)) = self.stack.pop() {
                    if let Some(v) = self.memory.read(addr) {
                        self.stack.push(v);
                        self.record_event(Event::MemoryRead);
                    } else {
                        self.stack.push(UVal::Nil);
                    }
                } else {
                    self.record_error(VMError::StackUnderflow);
                }
            }
            Op::Intuition => {
                let candidates: Vec<i64> = self.skills.macros.keys().copied().collect();
                let ctx = self
                    .intuition
                    .build_context(&self.stack, &self.recent_opcodes);
                if let Some(skill_id) = self.intuition.select_skill(&ctx, &candidates, self.tick) {
                    self.execute_skill(skill_id);
                }
            }
            Op::Jmp => {
                if self.ip >= self.program.len() {
                    return false;
                }
                let target = self.program[self.ip];
                self.ip += 1;
                if !target.is_finite() || target < 0.0 {
                    self.record_error(VMError::InvalidJump(-1));
                    return true;
                }
                let new_ip = target.round() as usize;
                if new_ip >= self.program.len() {
                    self.record_error(VMError::InvalidJump(new_ip as i64));
                    return true;
                }
                self.ip = new_ip;
            }
            Op::JmpIf => {
                if self.ip >= self.program.len() {
                    self.record_error(VMError::InvalidJump(-1));
                    return false;
                }
                if self.stack.is_empty() {
                    self.record_error(VMError::StackUnderflow);
                    return true;
                }
                let target = self.program[self.ip];
                self.ip += 1;
                let condition = self.stack.pop().unwrap();
                if condition.is_truthy() {
                    if !target.is_finite() || target < 0.0 {
                        self.record_error(VMError::InvalidJump(-1));
                        return true;
                    }
                    let new_ip = target.round() as usize;
                    if new_ip >= self.program.len() {
                        self.record_error(VMError::InvalidJump(new_ip as i64));
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
                    self.record_error(VMError::InvalidJump(-1));
                    return true;
                }
                let new_ip = target.round() as usize;
                if new_ip >= self.program.len() {
                    self.record_error(VMError::InvalidJump(new_ip as i64));
                    return true;
                }
                self.call_stack.push(self.ip);
                self.ip = new_ip;
            }
            Op::Ret => {
                if let Some(return_ip) = self.call_stack.pop() {
                    self.ip = return_ip;
                } else {
                    self.record_error(VMError::ReturnStackUnderflow);
                }
            }
            Op::Reward => {
                self.total_reward += 100.0;
                self.record_event(Event::Reward(100));
                self.flush_trace();
            }
            Op::Evolve => {
                if let Some(UVal::Number(id)) = self.stack.pop() {
                    let skill_program = self.program.clone();
                    match decode_ops_for_validation(&skill_program).and_then(|ops| {
                        validate_ops(&ops).map_err(|_| VMError::InvalidEvolve(id as i64))
                    }) {
                        Ok(_) => {
                            self.skills.define_skill(id as i64, skill_program);
                            self.record_event(Event::Reward(100));
                            self.flush_trace();
                        }
                        Err(err) => self.record_error(err),
                    }
                } else {
                    self.record_error(VMError::InvalidEvolve(-1));
                }
            }
            Op::Halt => {
                self.flush_trace();
                if self.restore_program() {
                    return true;
                }
                return false;
            }
            Op::Swap => {
                if self.stack.len() < 2 {
                    self.record_error(VMError::StackUnderflow);
                    return true;
                }
                let len = self.stack.len();
                self.stack.swap(len - 1, len - 2);
            }
            Op::Dup => {
                if let Some(val) = self.stack.last().cloned() {
                    self.stack.push(val);
                } else {
                    self.record_error(VMError::StackUnderflow);
                }
            }
            Op::Over => {
                if self.stack.len() < 2 {
                    self.record_error(VMError::StackUnderflow);
                    return true;
                }
                let len = self.stack.len();
                let val = self.stack[len - 2].clone();
                self.stack.push(val);
            }
            Op::Drop => {
                if self.stack.pop().is_none() {
                    self.record_error(VMError::StackUnderflow);
                }
            }
            Op::And => {
                if self.stack.len() < 2 {
                    self.record_error(VMError::StackUnderflow);
                    return true;
                }
                let b = self.stack.pop().unwrap();
                let a = self.stack.pop().unwrap();
                self.stack.push(UVal::Bool(a.is_truthy() && b.is_truthy()));
            }
            Op::Or => {
                if self.stack.len() < 2 {
                    self.record_error(VMError::StackUnderflow);
                    return true;
                }
                let b = self.stack.pop().unwrap();
                let a = self.stack.pop().unwrap();
                self.stack.push(UVal::Bool(a.is_truthy() || b.is_truthy()));
            }
            Op::Xor => {
                if self.stack.len() < 2 {
                    self.record_error(VMError::StackUnderflow);
                    return true;
                }
                let b = self.stack.pop().unwrap();
                let a = self.stack.pop().unwrap();
                let result = a.is_truthy() ^ b.is_truthy();
                self.stack.push(UVal::Bool(result));
            }
            Op::IsZero => {
                if let Some(val) = self.stack.pop() {
                    self.stack.push(UVal::Bool(!val.is_truthy()));
                } else {
                    self.record_error(VMError::StackUnderflow);
                }
            }
            Op::Mod => {
                if self.stack.len() < 2 {
                    self.record_error(VMError::StackUnderflow);
                    return true;
                }
                let b = self.stack.pop().unwrap();
                let a = self.stack.pop().unwrap();
                if let (UVal::Number(na), UVal::Number(nb)) = (a, b) {
                    self.stack.push(UVal::Number(na % nb));
                } else {
                    self.record_error(VMError::InvalidOpcode(opcode.as_i64()));
                }
            }
            Op::Inc => match self.stack.pop() {
                Some(UVal::Number(n)) => self.stack.push(UVal::Number(n + 1.0)),
                Some(_) => self.record_error(VMError::InvalidOpcode(opcode.as_i64())),
                None => self.record_error(VMError::StackUnderflow),
            },
            Op::Dec => match self.stack.pop() {
                Some(UVal::Number(n)) => self.stack.push(UVal::Number(n - 1.0)),
                Some(_) => self.record_error(VMError::InvalidOpcode(opcode.as_i64())),
                None => self.record_error(VMError::StackUnderflow),
            },
            Op::Parse => match self.stack.pop() {
                Some(UVal::String(text)) => match text.parse::<f64>() {
                    Ok(n) => self.stack.push(UVal::Number(n)),
                    Err(_) => self.stack.push(UVal::Nil),
                },
                Some(UVal::Number(n)) => self.stack.push(UVal::Number(n)),
                Some(_) => self.stack.push(UVal::Nil),
                None => self.record_error(VMError::StackUnderflow),
            },
        }

        true
    }

    fn find_next_opcode(&self, target_opcode: i64) -> Option<usize> {
        self.program.iter().enumerate().find_map(|(idx, &raw)| {
            if raw == target_opcode as f64 {
                Some(idx)
            } else {
                None
            }
        })
    }
}
