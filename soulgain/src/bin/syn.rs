use rand::Rng;
use soulgain::evolution::Trainer;
use soulgain::types::UVal;
use soulgain::vm::Op;
use soulgain::SoulGainVM;

// --- CONSTANTS FOR PERSISTENCE ---
const SKILLS_PATH: &str = "skills.json";
const PLASTICITY_PATH: &str = "plasticity.json";
const ATTEMPTS_LIMIT: usize = 100_000; // Increased to 100k as requested

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

fn print_separator(title: &str) {
    println!("\n{}", "=".repeat(80));
    println!("  {}", title);
    println!("{}", "=".repeat(80));
}

fn main() {
    print_separator("SoulGain High-Intensity Synthesis & Persistence Run");

    // Initialize VM and try to load existing state
    let mut vm = SoulGainVM::new(vec![]);
    
    if let Ok(_) = vm.plasticity.load_from_file(PLASTICITY_PATH) {
        println!("✓ Loaded existing plasticity from {}", PLASTICITY_PATH);
    }
    
    // Note: SkillLibrary currently lacks a built-in load_from_file in the provided snippet,
    // but the Trainer will populate new skills into the VM's registry.

    let mut trainer = Trainer::new(vm, 15); // Increased max program length for complexity

    // --- TEST 1: MODULAR ARITHMETIC ---
    println!("\n[Task 1] Addition Modulo (Attempts: {})", ATTEMPTS_LIMIT);
    let add_examples = random_examples(5, false);
    if let Some(program) = trainer.synthesize(&add_examples, ATTEMPTS_LIMIT) {
        println!("✓ Synthesized AddMod: {:?}", program);
    }

    // --- TEST 2: EVEN/ODD LOGIC ---
    println!("\n[Task 2] Even/Odd Detection (Attempts: {})", ATTEMPTS_LIMIT);
    let even_inputs = vec![vec![UVal::Number(4.0)], vec![UVal::Number(7.0)], vec![UVal::Number(12.0)]];
    let even_examples: Vec<(Vec<UVal>, Vec<UVal>)> = even_inputs.iter().map(|input| {
        let n = if let Some(UVal::Number(num)) = input.first() { *num } else { 0.0 };
        (input.clone(), vec![UVal::Bool((n as i64) % 2 == 0)])
    }).collect();

    if let Some(program) = trainer.synthesize(&even_examples, ATTEMPTS_LIMIT) {
        println!("✓ Synthesized Even/Odd: {:?}", program);
    }

    // --- PERSISTENCE BLOCK ---
    print_separator("SAVING BRAIN STATE");
    
    // Save Plasticity weights
    match trainer.vm.plasticity.save_to_file(PLASTICITY_PATH) {
        Ok(_) => println!("✓ Plasticity saved to {}", PLASTICITY_PATH),
        Err(e) => println!("✗ Plasticity save failed: {}", e),
    }

    // To save skills.json, we manually serialize the SkillLibrary
    let skills_file = std::fs::File::create(SKILLS_PATH).expect("Failed to create skills file");
    if let Ok(_) = serde_json::to_writer_pretty(skills_file, &trainer.vm.skills) {
        println!("✓ Skills saved to {}", SKILLS_PATH);
    }

    println!("\nRun complete. Check {} and {} for persisted data.", SKILLS_PATH, PLASTICITY_PATH);
}