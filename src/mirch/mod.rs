//! Mirch is the core component of the KernMiri. This module mainly provides abstractions for
//! pseudo physical memory which can be used to simulate the behaviors in physical memory of a system.

mod config;
mod config_toml;
mod page_table;
mod physical_mem;

pub use self::config::*;
pub use self::config_toml::KMiriConfigToml;
pub use self::page_table::*;
pub use self::physical_mem::*;
