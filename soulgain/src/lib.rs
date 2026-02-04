pub mod types;
pub mod vm;
pub mod memory;
pub mod plasticity;
pub mod evolution;
// Add this line to src/lib.rs
pub mod hypothesis;
pub use types::{UVal, SkillLibrary};
pub use memory::MemorySystem;
pub use plasticity::{Plasticity, Event, VMError};
pub use vm::{Op, SoulGainVM, SKILL_OPCODE_BASE};
