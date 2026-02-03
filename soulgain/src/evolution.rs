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
}

impl Trainer {
    pub fn new(vm: SoulGainVM, max_program_len: usize) -> Self {
        Self {
            vm,
            rng: rand::thread_rng(),
            max_program_len,
        }
    }

    pub fn synthesize<O: Oracle>(
        &mut self,
        oracle: &O,
        input: Vec<UVal>,
        attempts: usize,
    ) -> Option<Vec<f64>> {
        let expected = oracle.evaluate(input.clone());

        for _ in 0..attempts {
            let program = self.random_program(&input);
            let result = self.execute_program(program.clone());

            if result == expected {
                self.vm.plasticity.observe(Event::Reward);
                return Some(program);
            }

            self.vm
                .plasticity
                .observe(Event::Opcode { opcode: -1, stack_depth: self.vm.stack.len() });
            self.vm
                .plasticity
                .observe(Event::Error(VMError::InvalidOpcode(-1)));
        }

        None
    }

    fn random_program(&mut self, input: &[UVal]) -> Vec<f64> {
        let mut program = Vec::new();

        for value in input {
            if let UVal::Number(n) = value {
                program.push(OP_LITERAL as f64);
                program.push(*n);
            }
        }

        let ops = [OP_ADD, OP_SUB, OP_MUL];
        let extra_len = self.rng.gen_range(1..=self.max_program_len.max(1));

        for _ in 0..extra_len {
            let op = ops[self.rng.gen_range(0..ops.len())];
            program.push(op as f64);
        }

        program.push(OP_HALT as f64);
        program
    }

    fn execute_program(&mut self, program: Vec<f64>) -> Vec<UVal> {
        self.vm.stack.clear();
        self.vm.call_stack.clear();
        self.vm.ip = 0;
        self.vm.program = program;
        self.vm.run();
        self.vm.stack.clone()
    }
}
