use std::time::Instant;
use soulgain::evolution::{Oracle, Trainer};
use soulgain::types::UVal;
use soulgain::SoulGainVM;

struct SumOracle { target_count: usize }
impl Oracle for SumOracle {
    fn evaluate(&self, input: Vec<UVal>) -> Vec<UVal> {
        let sum = input.iter().take(self.target_count).filter_map(|v| if let UVal::Number(n) = v { Some(*n) } else { None }).sum::<f64>();
        vec![UVal::Number(sum)]
    }
}

fn main() {
    let mut vm = SoulGainVM::new(vec![]);

    // --- 1. BOOT ENGINE ---
    if let Ok(file) = std::fs::File::open("skills.json") {
        if let Ok(saved_skills) = serde_json::from_reader(file) {
            vm.skills = saved_skills;
            println!("[System] Skills Restored. Count: {}", vm.skills.macros.len());
        }
    }
    
    if std::path::Path::new("plasticity.json").exists() {
        let _ = vm.plasticity.load_from_file("plasticity.json");
        println!("[System] Intuition Restored.");
    }

    let mut trainer = Trainer::new(vm, 10);
    let master_input = (1..=20).map(|i| UVal::Number(i as f64)).collect::<Vec<_>>();

    // --- 2. STARTING FROM THE BOTTOM ---
    let levels = vec![
        (2, "Level -1: Sum 2 (The Absolute Base)"),
        (5, "Level 0: Sum 5 (Verification)"),
        (7, "Level 1: Sum 7 (Extension)"),
    ];

    for (count, title) in levels {
        println!("\n--- {} ---", title);
        let oracle = SumOracle { target_count: count };
        let input = master_input[0..count].to_vec();

        let start = Instant::now();
        // We set attempts high so it really explores the search space
        let result = trainer.synthesize(&oracle, input, 3000);
        
        if let Some(prog) = result {
            println!("  Found in: {:?}", start.elapsed());
            println!("  Logic Used: {:?}", prog);
        } else {
            println!("  [System] Stalled at {}. Logic too complex for current weights.", title);
            break;
        }

        // Save progress immediately
        let file = std::fs::File::create("skills.json").unwrap();
        serde_json::to_writer_pretty(file, &trainer.vm.skills).unwrap();
        let _ = trainer.vm.plasticity.save_to_file("plasticity.json");
    }

    println!("\n[System] Benchmark Cycle Complete.");
}