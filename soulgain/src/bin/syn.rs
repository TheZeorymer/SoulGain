use soulgain::logic::{aggregate_trace_logic, all_ops, logic_of, validate_ops};
use soulgain::vm::Op;

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
}
