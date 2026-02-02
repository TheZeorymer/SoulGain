use crate::SoulGainVM;
use crate::{OP_LITERAL, OP_ADD, OP_HALT};
use crate::types::UVal;
use std::sync::Arc;

pub fn test_numeric_logic() {
    let program = vec![
        OP_LITERAL as f64, 10.5,
        OP_LITERAL as f64, 20.5,
        OP_ADD as f64,
        OP_HALT as f64,
    ];

    let mut vm = SoulGainVM::new(program);
    vm.run();
    println!("Numeric Result: {:?}", vm.stack.last());
}

pub fn test_string_concatenation() {
    let mut vm = SoulGainVM::new(vec![OP_HALT as f64]);
    
    // Manually pushing strings
    vm.stack.push(UVal::String(Arc::new("Hello, ".to_string())));
    vm.stack.push(UVal::String(Arc::new("World!".to_string())));
    
    vm.program = vec![OP_ADD as f64, OP_HALT as f64];
    vm.run();
    
    println!("String Result: {}", vm.stack.last().unwrap());
} // <--- This was likely the missing one!