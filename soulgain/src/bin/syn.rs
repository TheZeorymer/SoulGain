use std::sync::Arc;
use std::time::Instant;

use rand::Rng;
use soulgain::SoulGainVM;
use soulgain::evolution::Trainer;
use soulgain::logic::{aggregate_trace_logic, all_ops, category_of, logic_of, validate_ops};
use soulgain::types::UVal;
use soulgain::vm::Op;

fn run_cps_stress_test() {
    let program = vec![
        Op::Literal.as_f64(),
        1.0,
        Op::Literal.as_f64(),
        2.0,
        Op::Add.as_f64(),
        Op::Drop.as_f64(),
        Op::Jmp.as_f64(),
        0.0,
    ];

    let mut vm = SoulGainVM::new(program);
    let cycles = 2_000_000usize;
    let start = Instant::now();
    vm.run(cycles);
    let elapsed = start.elapsed().as_secs_f64();
    let cps = (cycles as f64 / elapsed) as u64;

    println!("\n=== Stress Test Oracle (CPS) ===");
    println!(
        "cycles: {cycles}, elapsed: {:.3}s, cycles/sec: {cps}",
        elapsed
    );
}

fn run_string_to_math_torture() {
    let mut vm = SoulGainVM::new(vec![
        Op::Parse.as_f64(),
        Op::Literal.as_f64(),
        5.0,
        Op::Add.as_f64(),
        Op::Halt.as_f64(),
    ]);
    vm.stack.push(UVal::String(Arc::new("37.5".to_string())));
    vm.run(1000);
    println!("\n=== String -> Math Torture ===");
    println!("result stack: {:?}", vm.stack);
}

fn run_sort_torture() {
    let program = vec![
        Op::Over.as_f64(),
        Op::Over.as_f64(),
        Op::Gt.as_f64(),
        Op::JmpIf.as_f64(),
        6.0,
        Op::Halt.as_f64(),
        Op::Swap.as_f64(),
        Op::Halt.as_f64(),
    ];

    let mut vm = SoulGainVM::new(program);
    vm.stack.push(UVal::Number(9.0));
    vm.stack.push(UVal::Number(2.0));
    vm.run(1000);
    println!("\n=== Sort Torture (2-value compare/swap) ===");
    println!("result stack: {:?}", vm.stack);
}

fn generate_examples<F>(n: usize, f: F) -> Vec<(Vec<UVal>, Vec<UVal>)>
where
    F: Fn(f64, f64) -> f64,
{
    let mut rng = rand::thread_rng();
    (0..n)
        .map(|_| {
            let a = rng.gen_range(1..20) as f64;
            let b = rng.gen_range(1..20) as f64;
            let m = [3.0, 4.0, 5.0, 7.0][rng.gen_range(0..4)];
            let out = f(a + b, m);
            (
                vec![UVal::Number(a), UVal::Number(b), UVal::Number(m)],
                vec![UVal::Number(out)],
            )
        })
        .collect()
}

fn run_synthesis_pretrain() {
    println!("\n=== Synthesis Pretrain (multi-example) ===");
    let vm = SoulGainVM::new(vec![]);
    let mut trainer = Trainer::new(vm, 10);

    let add_mod_examples = generate_examples(4, |sum, m| sum % m);
    let mul_mod_examples: Vec<(Vec<UVal>, Vec<UVal>)> = add_mod_examples
        .iter()
        .map(|(input, _)| {
            let (a, b, m) = match (&input[0], &input[1], &input[2]) {
                (UVal::Number(a), UVal::Number(b), UVal::Number(m)) => (*a, *b, *m),
                _ => (1.0, 1.0, 2.0),
            };
            (input.clone(), vec![UVal::Number((a * b) % m)])
        })
        .collect();

    let add_mod_prog = trainer.synthesize(&add_mod_examples, 120);
    let mul_mod_prog = trainer.synthesize(&mul_mod_examples, 120);

    println!("AddMod examples: {:?}", add_mod_examples);
    println!("AddMod synthesized: {:?}", add_mod_prog);
    println!("MulMod examples: {:?}", mul_mod_examples);
    println!("MulMod synthesized: {:?}", mul_mod_prog);
}

fn main() {
    println!("=== SoulGain Op Logic Table ===");
    for op in all_ops() {
        let info = logic_of(*op);
        let category = category_of(*op);
        println!(
            "{:>10} ({:>2}) -> stack_delta: {:+}, may_branch: {}, category: {:?}",
            format!("{:?}", op),
            op.as_i64(),
            info.stack_delta,
            info.may_branch,
            category
        );
    }

    let valid_program = vec![Op::Literal, Op::Literal, Op::Add, Op::Halt];
    let invalid_underflow = vec![Op::Add, Op::Halt];
    let invalid_no_halt = vec![Op::Literal, Op::Dup];

    println!("\n=== Validation Examples ===");
    println!("valid_program: {:?}", validate_ops(&valid_program));
    println!("invalid_underflow: {:?}", validate_ops(&invalid_underflow));
    println!("invalid_no_halt: {:?}", validate_ops(&invalid_no_halt));

    println!("\n=== Trace Aggregation ===");
    let trace = vec![Op::Literal, Op::Dup, Op::JmpIf, Op::Drop, Op::Halt];
    let summary = aggregate_trace_logic(&trace);
    println!("trace: {:?}", trace);
    println!("summary: {:?}", summary);

    run_string_to_math_torture();
    run_sort_torture();
    run_synthesis_pretrain();
    run_cps_stress_test();
}
