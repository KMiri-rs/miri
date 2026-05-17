//! Mirch is the core component of the KernMiri. This module mainly provides abstractions for 
//! pseudo physical memory which can be used to simulate the behaviors in physical memory of a system. 


mod config;
mod physical_mem;
mod page_table;

pub use self::config::*;
pub use self::physical_mem::*;
pub use self::page_table::*;