use soulgain::{run, SoulGainVM};

const BRAIN_PATH: &str = "brain_test.json";

fn main() {
    println!("==============================================");
    println!("   SoulGain substrate (STDP Enabled) running  ");
    println!("==============================================");

    let mut vm = SoulGainVM::new(vec![]);

    if vm.plasticity.load_from_file(BRAIN_PATH).is_ok() {
        println!("[System] Loaded evolved weights from {}", BRAIN_PATH);
    } else {
        println!("[System] No brain file found. Initializing tabula rasa.");
    }

    run::test_numeric_logic(&mut vm);
    run::test_string_concatenation(&mut vm);
    run::test_boolean_logic(&mut vm);
    run::test_memory_persistence(&mut vm);
    run::test_learning_from_failure(&mut vm);

    run::stress_test_metabolic_pressure(&mut vm);
    run::stress_test_intuition_skipping(&mut vm);

    println!("\n[System] All tests completed.");

    if let Ok(mem) = vm.plasticity.memory.read() {
        println!("[System] Final Synaptic Count: {}", mem.weights.len());
    }

    if let Err(err) = vm.plasticity.save_to_file(BRAIN_PATH) {
        eprintln!("[Error] Failed to save evolved weights: {}", err);
    } else {
        println!("[System] Brain successfully saved to {}.", BRAIN_PATH);
    }
}
