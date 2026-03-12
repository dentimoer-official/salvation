pub mod compiler;
pub mod runtime;

use sysinfo::System;

pub fn check_memory_size(enlarge_level: u32) -> u64 {
    let mut memory = System::new();
    memory.refresh_all();
    
    let size = memory.total_memory() / (1024 as u64).pow(enlarge_level);
    
    size
}