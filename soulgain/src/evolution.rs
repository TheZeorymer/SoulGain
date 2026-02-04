use rand::Rng;
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::Write;
use crate::plasticity::Event;
use crate::types::UVal;
use crate::{SoulGainVM, OP_HALT, OP_LITERAL, OP_ADD, OP_SUB, OP_MUL};

pub trait Oracle {
    fn evaluate(&self, input: Vec<UVal>) -> Vec<UVal>;
}

pub struct Trainer {
    pub vm: SoulGainVM,
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
            explore_rate: 0.3,
            program_buf: Vec::new(),
        }
    }

    pub fn synthesize<O: Oracle + ?Sized>(
        &mut self,
        oracle: &O,
        input: Vec<UVal>,
        attempts_limit: usize,
    ) -> Option<Vec<f64>> {
        let expected = oracle.evaluate(input.clone());
        let mut failed_attempts: HashSet<Vec<u64>> = HashSet::new();
        let mut best_program: Option<Vec<f64>> = None;
        let mut best_fitness = 0.0;
        let input_preamble_len = input.len() * 2; 
        
        // SMART START: Don't try 1 op for 5 numbers.
        // We need at least (inputs - 1) operations to reduce them to a single sum.
        let min_ops = input.len().saturating_sub(1).max(1);

        let _ = std::fs::File::create("text.txt");

        let mut attempt = 0;
        while attempt < attempts_limit {
            let r = self.rng.r#gen::<f64>();

            // RANDOMIZED DEPTH: Don't get stuck on length 1. 
            // Pick a length between min_ops and max_len every time.
            let current_len = self.rng.gen_range(min_ops..=self.max_program_len);

            let try_invention = best_fitness < 0.1 && r < 0.5;
            let try_speculation = !try_invention && best_fitness > 0.0 && r < 0.2;
            let try_mutation = !try_invention && !try_speculation && best_fitness > 0.0;

            let (current_program, logic_start, tag) = if try_invention {
                // Generate a skill that fits the current random length
                let id = self.generate_smart_skill_logic(current_len);
                let (_ev, start) = self.build_program(&input, 1);
                let mut p = self.program_buf.clone();
                if p.len() > start {
                    let last_idx = p.len() - 2; 
                    p[last_idx] = id as f64;
                }
                (p, start, format!("INVENT_{}", id))
            } else if try_speculation {
                let mut variant = best_program.clone().unwrap_or_else(|| {
                     let (_, _) = self.build_program(&input, current_len);
                     self.program_buf.clone()
                });
                let id = self.speculate_new_skill(&mut variant, input_preamble_len);
                (variant, input_preamble_len, format!("SPEC_{:?}", id))
            } else if try_mutation {
                let mut variant = best_program.clone().unwrap();
                self.mutate_program(&mut variant, input_preamble_len);
                (variant, input_preamble_len, "MUTATE".to_string())
            } else {
                let (_last_event, start) = self.build_program(&input, current_len);
                (self.program_buf.clone(), start, "RANDOM".to_string())
            };

            let logic_bits: Vec<u64> = current_program[logic_start..].iter().map(|f| f.to_bits()).collect();
            if failed_attempts.contains(&logic_bits) { continue; }
            failed_attempts.insert(logic_bits);
            attempt += 1;

            let mut exec_buf = current_program.clone();
            let result = self.execute_program(&mut exec_buf);
            let fitness = self.calculate_fitness(&result, &expected);

            self.log_logic(&tag, &current_program[logic_start..], fitness);

            if fitness > best_fitness {
                best_fitness = fitness;
                best_program = Some(current_program.clone());
                if fitness > 0.1 {
                    self.vm.plasticity.observe(Event::Reward((fitness * 100.0) as u8));
                }
            }

            if fitness >= 0.9999 { 
                let logic_slice = current_program[logic_start..].to_vec();
                let mut clean_logic = logic_slice;
                if clean_logic.last() == Some(&(OP_HALT as f64)) { clean_logic.pop(); }

                if !clean_logic.is_empty() {
                    let skill_id = self.register_or_find_skill(clean_logic);
                    println!("  [SUCCESS] Concept: Opcode {} | Len: {}", skill_id, current_len);
                    self.imprint_skill(skill_id, &input);
                    
                    let mut optimized = current_program[..logic_start].to_vec();
                    optimized.push(skill_id as f64);
                    optimized.push(OP_HALT as f64);
                    return Some(optimized);
                }
                return Some(current_program); 
            }
        }
        None
    }

    fn register_or_find_skill(&mut self, logic: Vec<f64>) -> i64 {
        for (id, macro_logic) in &self.vm.skills.macros {
            if *macro_logic == logic { return *id; }
        }
        let new_id = self.generate_random_id();
        self.vm.skills.define_skill(new_id, logic);
        new_id
    }

    fn generate_smart_skill_logic(&mut self, target_len: usize) -> i64 {
        let mut logic = Vec::new();
        // Force the skill to be exactly the random length we picked
        for _ in 0..target_len {
            let op = if !self.vm.skills.macros.is_empty() && self.rng.gen_bool(0.3) {
                let keys: Vec<_> = self.vm.skills.macros.keys().cloned().collect();
                keys[self.rng.gen_range(0..keys.len())]
            } else {
                let basic = [OP_ADD, OP_SUB, OP_MUL];
                basic[self.rng.gen_range(0..basic.len())]
            };
            logic.push(op as f64);
        }
        self.register_or_find_skill(logic)
    }

    fn log_logic(&self, tag: &str, logic: &[f64], fitness: f64) {
        let decoded: Vec<String> = logic.iter().map(|&op| {
            if op == OP_ADD as f64 { "ADD".into() }
            else if op == OP_SUB as f64 { "SUB".into() }
            else if op == OP_MUL as f64 { "MUL".into() }
            else if op == OP_HALT as f64 { "HALT".into() } // FIXED LOG LABEL
            else if op >= 1000.0 { format!("OP_{}", op as i64) }
            else { format!("LIT({})", op) }
        }).collect();
        let mut file = OpenOptions::new().create(true).append(true).open("text.txt").unwrap();
        writeln!(file, "[{}] Fit: {:.4} | Logic: {:?}", tag, fitness, decoded).unwrap();
    }

    // [KEEP HELPERS: speculate_new_skill, mutate_program, build_program, choose_op_with_stdp, imprint_skill, generate_random_id, calculate_fitness, execute_program]
    // (Omitted here for length, but ensure they are present in your file)
    
    fn speculate_new_skill(&mut self, program: &mut Vec<f64>, logic_start: usize) -> Option<i64> {
        let logic_len = program.len().saturating_sub(1).saturating_sub(logic_start); 
        if logic_len < 2 { return None; }
        let window_size = self.rng.gen_range(2..=std::cmp::min(5, logic_len));
        let max_start = (program.len() - 1).saturating_sub(window_size);
        if max_start < logic_start { return None; }
        let start_idx = self.rng.gen_range(logic_start..=max_start);
        let pattern = program[start_idx..start_idx + window_size].to_vec();
        let new_id = self.register_or_find_skill(pattern);
        program.drain(start_idx..start_idx + window_size);
        program.insert(start_idx, new_id as f64);
        Some(new_id)
    }

    fn mutate_program(&mut self, program: &mut Vec<f64>, logic_start: usize) {
        if program.len() <= logic_start + 1 { return; } 
        let mutable_range = logic_start..program.len().saturating_sub(1);
        if mutable_range.is_empty() { return; }
        let idx = self.rng.gen_range(mutable_range);
        let mut ops: Vec<i64> = vec![OP_ADD, OP_SUB, OP_MUL];
        for &custom_op in self.vm.skills.macros.keys() { ops.push(custom_op); }
        program[idx] = ops[self.rng.gen_range(0..ops.len())] as f64;
    }

    fn build_program(&mut self, input: &[UVal], target_len: usize) -> (Event, usize) {
        self.program_buf.clear();
        let mut stack_depth = 0usize;
        for value in input {
            if let UVal::Number(n) = value {
                self.program_buf.push(OP_LITERAL as f64);
                self.program_buf.push(*n);
                stack_depth += 1;
            }
        }
        let logic_start = self.program_buf.len();
        let mut last_event = Event::Opcode { opcode: OP_LITERAL, stack_depth };
        for _ in 0..target_len {
            let op = self.choose_op_with_stdp(last_event, stack_depth);
            self.program_buf.push(op as f64);
            // Rough stack tracking
            stack_depth = if op == OP_LITERAL as i64 { stack_depth + 1 } else { stack_depth.saturating_sub(1) };
            last_event = Event::Opcode { opcode: op, stack_depth };
        }
        self.program_buf.push(OP_HALT as f64);
        (last_event, logic_start)
    }

    fn choose_op_with_stdp(&mut self, last_event: Event, stack_depth: usize) -> i64 {
        let mut ops: Vec<i64> = vec![OP_ADD, OP_SUB, OP_MUL];
        for &custom_op in self.vm.skills.macros.keys() { ops.push(custom_op); }
        if let Ok(mem) = self.vm.plasticity.memory.read() {
            let mut best_op = ops[0];
            let mut best_weight = f64::MIN;
            for &op in &ops {
                let target = Event::Opcode { opcode: op, stack_depth };
                let mut weight = mem.weights.get(&(last_event, target)).copied().unwrap_or(0.0);
                if op >= 1000 { weight += 2.5; } 
                if weight > best_weight { best_weight = weight; best_op = op; }
            }
            if best_weight >= 9.0 { return best_op; }
        }
        if self.rng.gen_bool(self.explore_rate) { return ops[self.rng.gen_range(0..ops.len())]; }
        ops[0]
    }

    fn imprint_skill(&self, op_id: i64, sample_input: &[UVal]) {
        if let Ok(mut mem) = self.vm.plasticity.memory.write() {
            let context = Event::Opcode { opcode: OP_LITERAL, stack_depth: sample_input.len() };
            let target = Event::Opcode { opcode: op_id, stack_depth: sample_input.len() };
            mem.weights.insert((context, target), 10.0);
        }
    }

    fn generate_random_id(&mut self) -> i64 {
        loop {
            let id = self.rng.gen_range(1000..9999);
            if !self.vm.skills.macros.contains_key(&id) { return id; }
        }
    }

    fn calculate_fitness(&self, result: &[UVal], expected: &[UVal]) -> f64 {
        if result.is_empty() || result.len() != expected.len() { return 0.0; }
        let mut score = 0.0;
        for (got, want) in result.iter().zip(expected.iter()) {
            if let (UVal::Number(a), UVal::Number(b)) = (got, want) {
                score += 1.0 / (1.0 + (a - b).abs());
            }
        }
        score / expected.len() as f64
    }

    fn execute_program(&mut self, program: &mut Vec<f64>) -> Vec<UVal> {
        self.vm.stack.clear();
        self.vm.ip = 0;
        let previous = std::mem::replace(&mut self.vm.program, std::mem::take(program));
        self.vm.run();
        *program = std::mem::take(&mut self.vm.program);
        self.vm.program = previous;
        self.vm.stack.clone()
    }
}