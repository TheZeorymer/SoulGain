use rand::Rng;

use crate::plasticity::{Event, VMError};
use crate::types::UVal;
use crate::{SoulGainVM, OP_ADD, OP_HALT, OP_LITERAL, OP_MUL, OP_SUB};

pub trait Oracle {
    fn evaluate(&self, input: Vec<UVal>) -> Vec<UVal>;
}

pub struct Trainer {
    vm: SoulGainVM,
    rng: rand::rngs::ThreadRng,
    max_program_len: usize,
    explore_rate: f64,
    program_buf: Vec<f64>,
}

impl Trainer {
    pub fn new(vm: SoulGainVM, max_program_len: usize) -> Self {
        Self {
            vm,
            rng: rand::thread_rng(),
            max_program_len,
            explore_rate: 0.2,
            program_buf: Vec::new(),
        }
    }

    pub fn synthesize<O: Oracle + ?Sized>(
        &mut self,
        oracle: &O,
        input: Vec<UVal>,
        attempts: usize,
    ) -> Option<Vec<f64>> {
        let expected = oracle.evaluate(input.clone());

        for _ in 0..attempts {
            let last_event = self.build_program(&input);
            
            // FIX: Extract the buffer to avoid double mutable borrow of 'self'
            let mut program = std::mem::take(&mut self.program_buf);
            let result = self.execute_program(&mut program);
            self.program_buf = program; // Move it back

            if result == expected {
                self.vm.plasticity.observe(Event::Reward);
                return Some(self.program_buf.clone());
            }

            self.vm.plasticity.observe(last_event);
            self.vm.plasticity.observe(Event::Error(VMError::InvalidOpcode(-1)));
        }

        None
    }

    fn build_program(&mut self, input: &[UVal]) -> Event {
        self.program_buf.clear();
        self.program_buf.reserve(input.len().saturating_mul(2) + self.max_program_len + 1);
        let mut stack_depth = 0usize;

        for value in input {
            if let UVal::Number(n) = value {
                self.program_buf.push(OP_LITERAL as f64);
                self.program_buf.push(*n);
                stack_depth += 1;
            }
        }

        let extra_len = self.rng.gen_range(1..=self.max_program_len.max(1));
        let mut last_event = Event::Opcode {
            opcode: OP_LITERAL,
            stack_depth,
        };

        for _ in 0..extra_len {
            if stack_depth < 2 {
                break;
            }
            let op = self.choose_op_with_stdp(last_event, stack_depth);
            self.program_buf.push(op as f64);
            stack_depth = stack_depth.saturating_sub(1);
            last_event = Event::Opcode {
                opcode: op,
                stack_depth,
            };
        }

        self.program_buf.push(OP_HALT as f64);
        last_event
    }

    fn choose_op_with_stdp(&mut self, last_event: Event, stack_depth: usize) -> i64 {
        let ops = [OP_ADD, OP_SUB, OP_MUL];
        if self.rng.gen_bool(self.explore_rate) {
            return ops[self.rng.gen_range(0..ops.len())];
        }

        if let Ok(mem) = self.vm.plasticity.memory.read() {
            let mut best_op = ops[0];
            let mut best_weight = f64::MIN;
            for op in ops {
                let to_event = Event::Opcode { opcode: op, stack_depth };
                let weight = mem
                    .weights
                    .get(&(last_event, to_event))
                    .copied()
                    .unwrap_or(0.0);
                if weight > best_weight {
                    best_weight = weight;
                    best_op = op;
                }
            }
            return best_op;
        }

        ops[self.rng.gen_range(0..ops.len())]
    }

    fn execute_program(&mut self, program: &mut Vec<f64>) -> Vec<UVal> {
        self.vm.stack.clear();
        self.vm.call_stack.clear();
        self.vm.ip = 0;
        let previous = std::mem::replace(&mut self.vm.program, std::mem::take(program));
        self.vm.run();
        *program = std::mem::take(&mut self.vm.program);
        self.vm.program = previous;
        self.vm.stack.clone()
    }
}
