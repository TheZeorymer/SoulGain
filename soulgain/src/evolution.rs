use rand::Rng;
use crate::plasticity::{Event};
use crate::types::UVal;
use crate::{SoulGainVM, OP_ADD, OP_HALT, OP_LITERAL, OP_MUL, OP_SUB};

pub trait Oracle {
    fn evaluate(&self, input: Vec<UVal>) -> Vec<UVal>;
}

pub struct Trainer {
    pub vm: SoulGainVM,
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
        let target_val = match expected.first() {
            Some(UVal::Number(n)) => *n,
            _ => 0.0,
        };

        for _ in 0..attempts {
            let program = self.generate_guided_program(&input);
            let result = self.execute_program(program.clone());

            if result == expected {
                // SUCCESS: Reinforce the successful timing/path
                self.vm.plasticity.observe(Event::Reward);
                return Some(program);
            } 
            
            // GRADIENT REWARD: Encourage programs that produce numbers near the target.
            // This helps build "Addition" or "Subtraction" weights even before the full chain is found.
            if let Some(UVal::Number(val)) = result.first() {
                let diff = (val - target_val).abs();
                if diff < 2.0 { 
                    self.vm.plasticity.observe(Event::Reward);
                }
            }
        }
        None
    }

    fn generate_guided_program(&mut self, input: &[UVal]) -> Vec<f64> {
        let mut program = Vec::new();
        let mut virtual_stack = 0;
        let mut last_event: Option<Event> = None;

        // 1. Push Inputs
        // These serve as the "context" for the STDP synapses.
        for value in input {
            if let UVal::Number(n) = value {
                program.push(OP_LITERAL as f64);
                program.push(*n);
                
                let evt = Event::Opcode { opcode: OP_LITERAL, stack_depth: virtual_stack };
                last_event = Some(evt);
                virtual_stack += 1;
            }
        }

        let ops = [OP_ADD, OP_SUB, OP_MUL];
        
        // 2. Guided Generation
        for _ in 0..self.max_program_len {
            // Safety: Don't pick an operator if it crashes the stack machine.
            if virtual_stack < 2 { break; }

            let mut next_opcode = -1.0;

            // STDP CONSULTATION
            if let Some(prev) = last_event {
                // High Trust in the Brain (90%)
                if self.rng.gen_bool(0.9) {
                    if let Some(suggested) = self.vm.plasticity.best_next_event(prev) {
                        // CRITICAL FILTER: The generator only accepts Opcode suggestions.
                        // It will ignore 'Reward' or 'Error' as next steps.
                        if let Event::Opcode { opcode, .. } = suggested {
                            if ops.contains(&opcode) {
                                next_opcode = opcode as f64;
                            }
                        }
                    }
                }
            }

            // FALLBACK: If brain is silent or confused, pick randomly.
            if next_opcode < 0.0 {
                next_opcode = ops[self.rng.gen_range(0..ops.len())] as f64;
            }

            program.push(next_opcode);
            virtual_stack -= 1; 

            let evt = Event::Opcode { 
                opcode: next_opcode as i64, 
                stack_depth: virtual_stack 
            };
            last_event = Some(evt);
        }

        program.push(OP_HALT as f64);
        program
    }

    fn execute_program(&mut self, program: Vec<f64>) -> Vec<UVal> {
        // Reset VM state for a fresh execution of the generated program
        self.vm.stack.clear();
        self.vm.call_stack.clear();
        self.vm.ip = 0;
        self.vm.program = program;
        self.vm.run();
        self.vm.stack.clone()
    }
}