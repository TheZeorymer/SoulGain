use std::time::Instant;

use soulgain::evolution::{Oracle, Trainer};
use soulgain::types::UVal;
use soulgain::SoulGainVM;

struct AddOracle;
struct SubOracle;
struct DeepAddOracle;
struct StoryOracle;

impl Oracle for AddOracle {
    fn evaluate(&self, input: Vec<UVal>) -> Vec<UVal> {
        let sum = numbers_from(input).iter().sum::<f64>();
        vec![UVal::Number(sum)]
    }
}

impl Oracle for SubOracle {
    fn evaluate(&self, input: Vec<UVal>) -> Vec<UVal> {
        let mut nums = numbers_from(input);
        let first = nums.get(0).copied().unwrap_or(0.0);
        let rest = nums.iter().skip(1).sum::<f64>();
        vec![UVal::Number(first - rest)]
    }
}

impl Oracle for DeepAddOracle {
    fn evaluate(&self, input: Vec<UVal>) -> Vec<UVal> {
        let sum = numbers_from(input).iter().sum::<f64>();
        vec![UVal::Number(sum)]
    }
}

impl Oracle for StoryOracle {
    fn evaluate(&self, input: Vec<UVal>) -> Vec<UVal> {
        let nums = numbers_from(input);
        let result = nums.get(0).copied().unwrap_or(0.0)
            - nums.get(1).copied().unwrap_or(0.0)
            + nums.get(2).copied().unwrap_or(0.0);
        vec![UVal::Number(result)]
    }
}
struct MulAddOracle;

impl Oracle for MulAddOracle {
    fn evaluate(&self, input: Vec<UVal>) -> Vec<UVal> {
        let nums = numbers_from(input);
        let a = nums.get(0).copied().unwrap_or(0.0);
        let b = nums.get(1).copied().unwrap_or(0.0);
        let c = nums.get(2).copied().unwrap_or(0.0);
        vec![UVal::Number(a * b + c)]
    }
}

struct Task {
    name: &'static str,
    input: Vec<UVal>,
    oracle: Box<dyn Oracle>,
    max_program_len: usize,
    attempts: usize,
}

fn main() {
    let tasks = vec![
        Task {
            name: "addition",
            input: vec![UVal::Number(5.0), UVal::Number(10.0)],
            oracle: Box::new(AddOracle),
            max_program_len: 3,
            attempts: 500,
        },
        Task {
            name: "subtraction",
            input: vec![UVal::Number(20.0), UVal::Number(7.0)],
            oracle: Box::new(SubOracle),
            max_program_len: 3,
            attempts: 500,
        },
        Task {
            name: "deep addition",
            input: vec![
                UVal::Number(1.0),
                UVal::Number(2.0),
                UVal::Number(3.0),
                UVal::Number(4.0),
                UVal::Number(5.0),
            ],
            oracle: Box::new(DeepAddOracle),
            max_program_len: 6,
            attempts: 1000,
        },
        Task {
            name: "story: 50 spend 30 get 60",
            input: vec![UVal::Number(50.0), UVal::Number(30.0), UVal::Number(40.0)],
            oracle: Box::new(StoryOracle),
            max_program_len: 4,
            attempts: 800,
        },
    
    Task {
    name: "weighted sum: (a * b) + c",
    input: vec![
        UVal::Number(4.0),
        UVal::Number(5.0),
        UVal::Number(6.0),
    ],
    oracle: Box::new(MulAddOracle),
    max_program_len: 4,
    attempts: 1500,
},
    ];


    println!("STDP-guided synthesis benchmark (first run vs learned run)");

    for task in tasks {
        let mut trainer = Trainer::new(SoulGainVM::new(vec![]), task.max_program_len);

        let start = Instant::now();
        let first = trainer.synthesize(task.oracle.as_ref(), task.input.clone(), task.attempts);
        let first_elapsed = start.elapsed();

        let start = Instant::now();
        let second = trainer.synthesize(task.oracle.as_ref(), task.input.clone(), task.attempts);
        let second_elapsed = start.elapsed();

        println!(
            "\nTask: {}\n  First run: {:?} (program found: {})\n  Learned run: {:?} (program found: {})",
            task.name,
            first_elapsed,
            first.is_some(),
            second_elapsed,
            second.is_some()
        );
    }
}

fn numbers_from(input: Vec<UVal>) -> Vec<f64> {
    input
        .into_iter()
        .filter_map(|val| match val {
            UVal::Number(n) => Some(n),
            _ => None,
        })
        .collect()
}
