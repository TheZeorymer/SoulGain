use std::time::Instant;
use soulgain::evolution::{Oracle, Trainer};
use soulgain::types::UVal;
use soulgain::SoulGainVM;

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

    let mut trainer = Trainer::new(vm, 30); 

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
        let start = Instant::now();
        
        // 1,000,000 attempts to find the switching logic
        let result = trainer.synthesize(&oracle, vm_inputs.clone(), 1_000_000);
        
        if let Some(prog) = result {
            println!("  [SUCCESS] Evolved in {:?}", start.elapsed());
            println!("  Logic: {:?}", prog);
        } else {
            println!("  [FAIL] Stalled. The AI is struggling to differentiate the two swap pairs.");
            break; 
        }

        let file = std::fs::File::create("skills.json").unwrap();
        serde_json::to_writer_pretty(file, &trainer.vm.skills).unwrap();
        let _ = trainer.vm.plasticity.save_to_file("plasticity.json");
    }
}