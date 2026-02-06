use std::time::Instant;

use soulgain::SoulGainVM;
use soulgain::logic::{aggregate_trace_logic, all_ops, logic_of, validate_ops};
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

fn main() {
    println!("=== SoulGain Op Logic Table ===");
    for op in all_ops() {
        let info = logic_of(*op);
        println!(
            "{:>10} ({:>2}) -> stack_delta: {:+}, may_branch: {}",
            format!("{:?}", op),
            op.as_i64(),
            info.stack_delta,
            info.may_branch
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

    run_cps_stress_test();
}
