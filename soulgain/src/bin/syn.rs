use std::time::Instant;

use soulgain::hypothesis::Hypothesis;
use soulgain::logic::WeightedInferenceEngine;
use soulgain::plasticity::Event;
use soulgain::types::UVal;
use soulgain::{Op, SoulGainVM};

use soulgain::evolution::Oracle;

struct DNAOracle;

impl Oracle for DNAOracle {
    fn evaluate(&self, input: Vec<UVal>) -> Vec<UVal> {
        // Rule: 0->1, 1->0, 2->3, 3->2
        input.iter().map(|v| {
            if let UVal::Number(n) = v {
                let res = match *n as i64 {
                    0 => 1,
                    1 => 0,
                    2 => 3,
                    3 => 2,
                    _ => 0,
                };
                UVal::Number(res as f64)
            } else {
                UVal::Nil
            }
        }).collect()
    }
}

fn main() {
    let mut vm = SoulGainVM::new(vec![]);

    println!("========================================");
    println!("   SOULGAIN: DNA TRANSCRIBER            ");
    println!("   Rule: [0<->1] and [2<->3]            ");
    println!("========================================");

    if let Ok(file) = std::fs::File::open("skills.json") {
        if let Ok(saved_skills) = serde_json::from_reader(file) {
            vm.skills = saved_skills;
        }
    }
    let _ = vm.plasticity.load_from_file("plasticity.json");

    let levels = vec![
        (vec![0.0], "Level 1: A -> T (0 -> 1)"),
        (vec![1.0], "Level 2: T -> A (1 -> 0)"),
        (vec![2.0], "Level 3: C -> G (2 -> 3)"),
        (vec![3.0], "Level 4: G -> C (3 -> 2)"),
        // The "Brain Test": Can it handle a sequence?
        (vec![0.0, 2.0], "Level 5: Sequence AC -> TG (0,2 -> 1,3)"),
    ];

    let oracle = DNAOracle;

    for (inputs, title) in levels {
        println!("\n--- {} ---", title);
        let vm_inputs: Vec<UVal> = inputs.iter().map(|&n| UVal::Number(n)).collect();
        let expected = oracle.evaluate(vm_inputs.clone());
        let start = Instant::now();

        let engine = WeightedInferenceEngine::new(&vm.plasticity, &vm.skills);
        let mut solution: Option<Vec<f64>> = None;

        // Parallel cognitive loop: logic search + lightweight hypothesis mutation.
        let attempts_limit = 2000usize;
        let logic_budget = 400usize;
        let max_depth = 10usize;
        let hypothesis_len = 6usize;

        for _ in 0..attempts_limit {
            // Logic gets the focus: deeper weighted search.
            let logic_solution = engine.deduce(&vm_inputs, &expected, logic_budget, max_depth);

            // Hypothesis mutates lightly to keep diversity alive.
            let hypothesis_solution = run_hypothesis_attempt(
                &vm,
                &vm_inputs,
                &expected,
                hypothesis_len,
            );

            if let Some(program) = logic_solution {
                solution = Some(program);
                break;
            }

            if let Some(program) = hypothesis_solution {
                solution = Some(program);
                break;
            }
        }

        if let Some(logic) = solution {
            println!("  [SUCCESS] Solved in {:?}", start.elapsed());
            println!("  Logic: {:?}", logic);

            let program = build_program(&vm_inputs, &logic);
            execute_solution(&mut vm, program.clone());
            imprint_solution(&mut vm, &logic, vm_inputs.len());
        } else {
            println!("  [FAIL] Stalled. The AI is struggling to differentiate the two swap pairs.");
            break;
        }

        let file = std::fs::File::create("skills.json").unwrap();
        serde_json::to_writer_pretty(file, &vm.skills).unwrap();
        let _ = vm.plasticity.save_to_file("plasticity.json");
    }
}

fn build_program(input: &[UVal], logic: &[f64]) -> Vec<f64> {
    let mut program = Vec::new();
    for value in input {
        if let UVal::Number(n) = value {
            program.push(Op::Literal.as_f64());
            program.push(*n);
        }
    }
    program.extend_from_slice(logic);
    if program.last() != Some(&Op::Halt.as_f64()) {
        program.push(Op::Halt.as_f64());
    }
    program
}

fn execute_solution(vm: &mut SoulGainVM, program: Vec<f64>) {
    vm.stack.clear();
    vm.program = program;
    vm.ip = 0;
    vm.run(10_000);
}

fn run_hypothesis_attempt(
    base_vm: &SoulGainVM,
    input: &[UVal],
    expected: &[UVal],
    logic_len: usize,
) -> Option<Vec<f64>> {
    let available_skills: Vec<i64> = base_vm.skills.macros.keys().cloned().collect();
    let hypothesis = Hypothesis::generate(logic_len, &available_skills);
    let program = build_program(input, &hypothesis.logic);

    if evaluate_program(base_vm, program, expected) {
        Some(hypothesis.logic)
    } else {
        None
    }
}

fn evaluate_program(base_vm: &SoulGainVM, program: Vec<f64>, expected: &[UVal]) -> bool {
    let mut test_vm = SoulGainVM::new(program);
    test_vm.skills = base_vm.skills.clone();
    test_vm.memory = base_vm.memory.clone();
    test_vm.plasticity = base_vm.plasticity.clone();

    test_vm.run(10_000);

    if test_vm.stack.len() != expected.len() {
        return false;
    }
    test_vm
        .stack
        .iter()
        .zip(expected.iter())
        .all(|(a, b)| a == b)
}

fn imprint_solution(vm: &mut SoulGainVM, logic: &[f64], input_depth: usize) {
    let norm_depth = std::cmp::min(input_depth, 5);
    let mut events = Vec::new();

    let mut last_event = Event::Opcode {
        opcode: Op::Literal.as_i64(),
        stack_depth: norm_depth,
    };

    if let Ok(mut mem) = vm.plasticity.memory.write() {
        for &op in logic {
            let opcode = op.round() as i64;
            let next_event = Event::Opcode {
                opcode,
                stack_depth: norm_depth,
            };
            mem.weights
                .entry(last_event)
                .or_insert_with(std::collections::HashMap::new)
                .insert(next_event, 10.0);
            events.push(next_event);
            last_event = next_event;
        }
    }

    events.push(Event::Reward(100));
    vm.plasticity.observe_batch(events);
}
