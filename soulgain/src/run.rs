use crate::SoulGainVM;
use crate::plasticity::Event;
use crate::{
    OP_LITERAL, OP_ADD, OP_GT, OP_STORE, OP_LOAD, OP_HALT, 
    OP_REWARD, OP_INTUITION, OP_SUB
};
use crate::types::UVal;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

// --- ORIGINAL TESTS ---

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
    
    println!("Training the brain on bad code (String + Number)...");
    for _ in 0..10 {
        vm.stack.clear();
        vm.ip = 0;
        vm.stack.push(UVal::String(Arc::new("Text".to_string())));
        vm.stack.push(UVal::Number(42.0));
        vm.program = vec![OP_ADD as f64, OP_HALT as f64];
        vm.run();
    }

    thread::sleep(Duration::from_millis(50));
    let memory = vm.plasticity.memory.read().unwrap();
    let mut found_scar = false;

    for ((from, to), weight) in &memory.weights {
        if *weight > 0.01 {
            if let Event::Error(_) = to {
                println!("  [SCAR DETECTED] {:?} leads to {:?} (Strength: {:.4})", from, to, weight);
                found_scar = true;
            }
        }
    }
    if !found_scar { println!("  (No deep scars formed yet.)"); }
}

// --- NEW STRESS TESTS ---

/// Hammers the background worker with thousands of events to test MPSC lag and normalization speed.
pub fn stress_test_metabolic_pressure(vm: &mut SoulGainVM) {
    println!("\n--- [STRESS] Metabolic Pressure (10,000 Ops) ---");
    let start = Instant::now();
    
    vm.program = vec![
        OP_LITERAL as f64, 1.0, 
        OP_LITERAL as f64, 1.0, 
        OP_ADD as f64, 
        OP_REWARD as f64, 
        OP_HALT as f64
    ];

    for i in 0..10_000 {
        vm.ip = 0;
        vm.stack.clear();
        vm.run();
        if i % 2500 == 0 && i > 0 {
            println!("  Processed {} iterations...", i);
        }
    }

    println!("VM completed execution in: {:?}", start.elapsed());
    println!("Waiting for background thread to drain the synaptic queue...");
    
    // Give the worker time to catch up on the 50,000+ individual STDP updates
    thread::sleep(Duration::from_millis(500));
    println!("Total Stress Duration: {:?}", start.elapsed());
}

/// Tests if the VM can "learn" a long path and skip directly to the Reward using Intuition.
pub fn stress_test_intuition_skipping(vm: &mut SoulGainVM) {
    println!("\n--- [STRESS] Intuition & Predictive Pathing ---");
    
    // Path: Lit -> Lit -> Add -> Sub -> Reward
    vm.program = vec![
        OP_LITERAL as f64, 10.0,
        OP_LITERAL as f64, 5.0,
        OP_ADD as f64,      // We want the brain to associate ADD with REWARD
        OP_SUB as f64,
        OP_REWARD as f64,
        OP_HALT as f64,
    ];

    println!("Training the brain on a rewarded sequence (50 cycles)...");
    for _ in 0..50 {
        vm.ip = 0;
        vm.stack.clear();
        vm.run();
    }

    thread::sleep(Duration::from_millis(50));

    println!("Executing Intuition at instruction 0...");
    // Inject OP_INTUITION. If the brain is trained, it should jump IP forward.
    vm.program.insert(0, OP_INTUITION as f64);
    vm.ip = 0;
    vm.stack.clear();
    vm.run();

    println!("Final IP: {} (If > 1, Intuition jumped!)", vm.ip);
}