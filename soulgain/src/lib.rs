pub mod types;
pub mod memory;
pub mod plasticity;
pub mod run;
pub mod evolution;
pub mod vm;

pub use vm::{
    SoulGainVM, OP_ADD, OP_CALL, OP_EQ, OP_EVOLVE, OP_GT, OP_HALT, OP_INTUITION, OP_JMP,
    OP_JMP_IF, OP_LITERAL, OP_LOAD, OP_MUL, OP_NOT, OP_RET, OP_REWARD, OP_STORE, OP_SUB,
};
