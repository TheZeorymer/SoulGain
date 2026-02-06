use rand::Rng;
use soulgain::SoulGainVM;
use soulgain::evolution::Trainer;
use soulgain::types::UVal;
use soulgain::vm::Op;

fn random_examples(n: usize, mul: bool) -> Vec<(Vec<UVal>, Vec<UVal>)> {
    let mut rng = rand::thread_rng();
    (0..n)
        .map(|_| {
            let a = rng.gen_range(1..25) as f64;
            let b = rng.gen_range(1..25) as f64;
            let m = [3.0, 5.0, 7.0, 11.0][rng.gen_range(0..4)];
            let out = if mul { (a * b) % m } else { (a + b) % m };
            (
                vec![UVal::Number(a), UVal::Number(b), UVal::Number(m)],
                vec![UVal::Number(out)],
            )
        })
        .collect()
}

fn run_honest_synthesis() {
    println!("\n=== Honest Synthesis (multi-example stack mode) ===");
    let mut trainer = Trainer::new(SoulGainVM::new(vec![]), 12);

    let add_mod_examples = random_examples(5, false);
    let mul_mod_examples = random_examples(5, true);

    let add_prog = trainer.synthesize(&add_mod_examples, 140);
    let mul_prog = trainer.synthesize(&mul_mod_examples, 140);

    println!("AddMod examples: {:?}", add_mod_examples);
    println!("AddMod program: {:?}", add_prog);
    println!("MulMod examples: {:?}", mul_mod_examples);
    println!("MulMod program: {:?}", mul_prog);
}

fn run_even_odd_routing_feedback() {
    println!("\n=== Intuition Routing Loop (Even/Odd by value features) ===");

    let mut vm = SoulGainVM::new(vec![]);

    // Skills are macro programs; Intuition should learn to pick based on stack value features.
    // even_skill: n -> (n % 2 == 0)
    vm.skills.define_skill(
        2001,
        vec![
            Op::Literal.as_f64(),
            2.0,
            Op::Mod.as_f64(),
            Op::IsZero.as_f64(),
            Op::Halt.as_f64(),
        ],
    );

    // odd_skill: n -> (n % 2 != 0)
    vm.skills.define_skill(
        2002,
        vec![
            Op::Literal.as_f64(),
            2.0,
            Op::Mod.as_f64(),
            Op::IsZero.as_f64(),
            Op::Not.as_f64(),
            Op::Halt.as_f64(),
        ],
    );

    let mut rng = rand::thread_rng();
    let mut correct = 0usize;
    let rounds = 80usize;

    for _ in 0..rounds {
        let n = rng.gen_range(1..40) as f64;
        let expected_even = (n as i64) % 2 == 0;

        vm.program = vec![Op::Intuition.as_f64(), Op::Halt.as_f64()];
        vm.stack.clear();
        vm.stack.push(UVal::Number(n));
        vm.ip = 0;
        vm.run(200);

        let predicted = matches!(vm.stack.last(), Some(UVal::Bool(b)) if *b);
        let is_correct = predicted == expected_even;

        if is_correct {
            correct += 1;
            // Positive reinforcement signal; pending-credit logic can assign this to recent skill usage.
            vm.program = vec![Op::Reward.as_f64(), Op::Halt.as_f64()];
            vm.ip = 0;
            vm.run(20);
        }
    }

    println!("Even/Odd routing accuracy: {}/{}", correct, rounds);
}

fn main() {
    run_honest_synthesis();
    run_even_odd_routing_feedback();
}
