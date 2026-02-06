pub mod evolution;
pub mod memory;
pub mod plasticity;
pub mod types;
pub mod vm;
// Add this line to src/lib.rs
pub mod hypothesis;
pub mod intuition;
pub mod logic;
pub mod run;
pub use memory::MemorySystem;
pub use plasticity::{Event, Plasticity, VMError};
pub use types::{SkillLibrary, UVal};
pub use vm::{Op, SoulGainVM, SKILL_OPCODE_BASE};

pub use logic::{
    aggregate_trace_logic, category_of, logic_of, validate_ops, LogicInfo, LogicValidationError,
    OpCategory, TraceLogicSummary,
};
