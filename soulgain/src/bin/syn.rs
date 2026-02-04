use std::time::Instant;
use soulgain::evolution::{Oracle, Trainer};
use soulgain::types::UVal;
use soulgain::SoulGainVM;

// --- THE NEW CHALLENGE: FIBONACCI ---
// F(0)=0, F(1)=1, F(n)=F(n-1)+F(n-2)
struct FibOracle;

impl Oracle for FibOracle {
    fn evaluate(&self, input: Vec<UVal>) -> Vec<UVal> {
        if let Some(UVal::Number(n)) = input.first() {
            let n = *n as u64;
            let res = fib_recursive(n);
            vec![UVal::Number(res as f64)]
        } else {
            vec![]
        }
    }
}

fn fib_recursive(n: u64) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        _ => fib_recursive(n - 1) + fib_recursive(n - 2),
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
    
    // Clear intuition for a fresh problem type?
    // We KEEP it because maybe the multiplication/math hacks it learned are useful!
    if std::path::Path::new("plasticity.json").exists() {
        let _ = vm.plasticity.load_from_file("plasticity.json");
        println!("[System] Intuition Restored.");
    }

    // Increase complexity limit. Loops require more instructions.
    // Length 20 allows for setup, loop body, and condition checks.
    let mut trainer = Trainer::new(vm, 20); 

    // --- 2. THE CURRICULUM ---
    let levels = vec![
        (2.0, "Level 0: Fib(2) -> 1 (Basic Logic)"),   // 0, 1, 1
        (4.0, "Level 1: Fib(4) -> 3 (Short Loop)"),    // 0, 1, 1, 2, 3
        (6.0, "Level 2: Fib(6) -> 8 (True Loop)"),     // ... 5, 8
        (10.0, "Level 3: Fib(10) -> 55 (Complex State)"), 
    ];

    let oracle = FibOracle;

    for (input_val, title) in levels {
        println!("\n--- {} ---", title);
        
        // Input is just ONE number: N
        let input = vec![UVal::Number(input_val)];

        let start = Instant::now();
        // Give it 10,000 attempts because discovering a loop structure is VERY hard randomly
        let result = trainer.synthesize(&oracle, input, 10000);
        
        if let Some(prog) = result {
            println!("  Found in: {:?}", start.elapsed());
            println!("  Logic Used: {:?}", prog);
        } else {
            println!("  [System] Stalled at {}. Logic too complex.", title);
            // Don't break immediately, let it try the next level just in case
            // break; 
        }

        // Save progress
        let file = std::fs::File::create("skills.json").unwrap();
        serde_json::to_writer_pretty(file, &trainer.vm.skills).unwrap();
        let _ = trainer.vm.plasticity.save_to_file("plasticity.json");
    }

    println!("\n[System] Benchmark Cycle Complete.");
}