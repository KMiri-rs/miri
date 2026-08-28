use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct KMiriConfigToml {
    #[serde(default = "config_page_table")]
    page_table: bool,
    /// The upper limit of physical memory for the kernel.
    total_mem_size: u64,
    /// The key is symbol defined in asm or ld sciprt.
    /// The value is physical address.
    #[serde(default)]
    layout: BTreeMap<String, u64>,
    #[serde(default)]
    kalloc: Vec<KAlloc>,
}

/// Enable page table by default.
/// FIXME: default to false if asterinas migrates to toml config.
fn config_page_table() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize)]
pub struct KAlloc {
    pub name: String,
    pub base_addr: usize,
    pub size: usize,
    pub align: usize,
}

impl KMiriConfigToml {
    pub fn new(path: &Path) -> Option<Self> {
        let str = fs::read_to_string(path).ok()?;
        basic_toml::from_str(&str).ok()
    }

    pub fn page_table(&self) -> bool {
        self.page_table
    }

    pub fn symbol_addr(&self, symbol: &str) -> Option<u64> {
        self.layout.get(symbol).copied()
    }

    pub fn layout_symbols(&self) -> impl Iterator<Item = (&str, u64)> {
        self.layout.iter().map(|(name, addr)| (name.as_str(), *addr))
    }

    pub fn get_kalloc(&self, base_addr: usize) -> Option<&KAlloc> {
        self.kalloc.iter().find(|kalloc| kalloc.base_addr == base_addr)
    }
}

#[test]
fn layout() {
    let config: KMiriConfigToml = basic_toml::from_str(
        "
[layout]
_stext   = 0x80000000
_etext   = 0x8001b600
_sflash  = 0x80000000
_eflash  = 0x80200000
_sapps   = 0x80100000
_eapps   = 0x80200000
_ssram   = 0x80200000
_esram   = 0x80400000
_sappmem = 0x8021b680
_eappmem = 0x80400000
",
    )
    .unwrap();
    assert_eq!(config.layout["_stext"], 0x80000000);
    assert_eq!(config.layout["_sapps"], 0x80100000);
    assert_eq!(config.layout["_eappmem"], 0x80400000);
}
