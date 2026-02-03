use soulgain::{run, SoulGainVM};

const BRAIN_PATH: &str = "brain_test.json";

fn main() {
    println!("SoulGain substrate (STDP Enabled) running.");

    let mut vm = SoulGainVM::new(vec![]);
    if vm.plasticity.load_from_file(BRAIN_PATH).is_ok() {
        println!("Loaded brain from {}", BRAIN_PATH);
    }
    
    // Call the test functions defined in run.rs
    run::test_numeric_logic(&mut vm);
    run::test_string_concatenation(&mut vm);
    run::test_boolean_logic(&mut vm);
    run::test_memory_persistence(&mut vm);
    run::test_learning_from_failure(&mut vm);

    if let Err(err) = vm.plasticity.save_to_file(BRAIN_PATH) {
        eprintln!("Failed to save brain: {}", err);
    }
}
