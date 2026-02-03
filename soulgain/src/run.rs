use crate::SoulGainVM;
// Removed unused OP_EQ
use crate::{OP_LITERAL, OP_ADD, OP_GT, OP_STORE, OP_LOAD, OP_HALT};
use crate::types::UVal;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub fn test_numeric_logic(vm: &mut SoulGainVM) {
    println!("--- Testing Numeric Logic ---");
    vm.stack.clear();
    vm.call_stack.clear();
    vm.ip = 0;
    vm.program = vec![
        OP_LITERAL as f64, 10.5,
        OP_LITERAL as f64, 20.5,
        OP_ADD as f64,
        OP_HALT as f64,
    ];
    vm.run();
    println!("10.5 + 20.5 = {:?}", vm.stack.last().unwrap());
}

pub fn test_string_concatenation(vm: &mut SoulGainVM) {
    println!("\n--- Testing String Concatenation ---");
    vm.stack.clear();
    vm.call_stack.clear();
    vm.ip = 0;
    vm.program = vec![OP_HALT as f64];
    vm.stack.push(UVal::String(Arc::new("Hello, ".to_string())));
    vm.stack.push(UVal::String(Arc::new("World!".to_string())));
    vm.program = vec![OP_ADD as f64, OP_HALT as f64];
    vm.run();
    println!("Result: {}", vm.stack.last().unwrap());
}

pub fn test_boolean_logic(vm: &mut SoulGainVM) {
    println!("\n--- Testing Boolean Logic ---");
    vm.stack.clear();
    vm.call_stack.clear();
    vm.ip = 0;
    vm.program = vec![
        OP_LITERAL as f64, 10.0,
        OP_LITERAL as f64, 5.0,
        OP_GT as f64,
        OP_HALT as f64,
    ];
    vm.run();
    println!("10.0 > 5.0 is: {}", vm.stack.last().unwrap());
}

pub fn test_memory_persistence(vm: &mut SoulGainVM) {
    println!("\n--- Testing Memory Persistence ---");
    vm.stack.clear();
    vm.call_stack.clear();
    vm.ip = 0;
    vm.program = vec![];
    vm.stack.push(UVal::Number(100.0)); 
    vm.stack.push(UVal::String(Arc::new("Soul Data".to_string())));
    vm.program = vec![OP_STORE as f64, OP_LITERAL as f64, 100.0, OP_LOAD as f64, OP_HALT as f64];
    vm.run();
    println!("Memory at 100.0: {}", vm.stack.last().unwrap());
}

pub fn test_learning_from_failure(vm: &mut SoulGainVM) {
    println!("\n--- Testing STDP Pain Learning (Async) ---");
    vm.stack.clear();
    vm.call_stack.clear();
    vm.ip = 0;
    vm.program = vec![OP_HALT as f64];
    
    println!("Training the brain on bad code (String + Number)...");
    
    for _ in 0..10 {
        vm.stack.clear();
        vm.ip = 0;
        vm.stack.push(UVal::String(Arc::new("Text".to_string())));
        vm.stack.push(UVal::Number(42.0));
        vm.program = vec![OP_ADD as f64, OP_HALT as f64];
        vm.run();
    }

    // ASYNC FIX: Give the background thread a moment to process the final events
    thread::sleep(Duration::from_millis(50));

    println!("Examining the Soul's scars (Synaptic Weights):");
    
    // ASYNC FIX: We must acquire a read lock to see the weights
    let memory = vm.plasticity.memory.read().unwrap();
    let mut found_scar = false;

    for ((from, to), weight) in &memory.weights {
        if *weight > 0.01 { // Lower threshold for STDP initial learning
            if let crate::plasticity::Event::Error(_) = to {
                println!("  [SCAR DETECTED] {:?} leads to {:?} (Strength: {:.4})", from, to, weight);
                found_scar = true;
            }
        }
    }

    if !found_scar {
        println!("  (No deep scars formed yet.)");
    }
}
