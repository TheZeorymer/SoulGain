use crate::SoulGainVM;
// Import all the new opcodes we defined in main.rs
use crate::{OP_LITERAL, OP_ADD, OP_SUB, OP_EQ, OP_GT, OP_STORE, OP_LOAD, OP_HALT};
use crate::types::UVal;
use std::sync::Arc;

pub fn test_numeric_logic() {
    println!("--- Testing Numeric Logic ---");
    // 10.5 + 20.5
    let program = vec![
        OP_LITERAL as f64, 10.5,
        OP_LITERAL as f64, 20.5,
        OP_ADD as f64,
        OP_HALT as f64,
    ];

    let mut vm = SoulGainVM::new(program);
    vm.run();
    println!("10.5 + 20.5 = {:?}", vm.stack.last().unwrap());
}

pub fn test_string_concatenation() {
    println!("\n--- Testing String Concatenation ---");
    // We initialize with a HALT just to get the VM struct
    let mut vm = SoulGainVM::new(vec![OP_HALT as f64]);
    
    // Manually pushing strings to the stack because our bytecode loader 
    // currently only supports f64 literals. 
    // (Future TODO: Add OP_LITERAL_STR to load strings from a pool)
    vm.stack.push(UVal::String(Arc::new("Hello, ".to_string())));
    vm.stack.push(UVal::String(Arc::new("World!".to_string())));
    
    // Program: ADD, HALT
    vm.program = vec![OP_ADD as f64, OP_HALT as f64];
    vm.run();
    
    println!("Result: {}", vm.stack.last().unwrap());
}

pub fn test_boolean_logic() {
    println!("\n--- Testing Boolean Logic & STDP ---");
    // Correct RPN Order for "10 > 5":
    // 1. Push 10
    // 2. Push 5
    // 3. GT (Checks 10 > 5)
    let program = vec![
        OP_LITERAL as f64, 10.0, // A (LHS)
        OP_LITERAL as f64, 5.0,  // B (RHS)
        OP_GT as f64,            // Checks A > B
        OP_HALT as f64,
    ];

    let mut vm = SoulGainVM::new(program);
    vm.run();
    println!("10.0 > 5.0 is: {}", vm.stack.last().unwrap());
}

pub fn test_memory_persistence() {
    println!("\n--- Testing UVal Memory Storage ---");
    let mut vm = SoulGainVM::new(vec![]);
    
    // Manually setup stack for the Store operation:
    // Stack: [Address (100.0), Value ("Soul Data")]
    vm.stack.push(UVal::Number(100.0)); 
    vm.stack.push(UVal::String(Arc::new("Soul Data".to_string())));
    
    vm.program = vec![
        OP_STORE as f64, // Writes "Soul Data" to 100.0
        
        // Logic to read it back
        OP_LITERAL as f64, 100.0,
        OP_LOAD as f64,
        OP_HALT as f64,
    ];
    
    vm.run();
    println!("Memory at 100.0: {}", vm.stack.last().unwrap());
}