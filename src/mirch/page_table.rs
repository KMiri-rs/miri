use std::cell::RefCell;
use std::collections::BTreeMap;

use crate::mirch::{paddr_to_mem, page_size};
use crate::*;

pub const NR_LEVELS: usize = 4;
pub const PTE_SIZE: usize = 8;

const BOOT_PT_PADDR: usize = 0x1000;
const BOOT_PT_LINEAR_PDPT_PADDR: usize = 0x2000;
const BOOT_PT_KERNEL_PDPT_PADDR: usize = 0x3000;

pub fn kernel_code_paddr_to_vaddr(paddr: usize) -> usize {
    paddr + super::kernel_code_base_vaddr()
}

pub fn kernel_code_vaddr_to_paddr(vaddr: usize) -> usize {
    vaddr - super::kernel_code_base_vaddr()
}

/// Inits a boot page table to enable paging system at the pseudo physical memory.
///
/// Boot pagetable support up to 1GB of pseudo physical memory.
pub unsafe fn init_boot_pt() -> PageTable {
    let page_table = PageTable::new(BOOT_PT_PADDR);

    *(paddr_to_mem(BOOT_PT_PADDR) as *mut usize) = BOOT_PT_LINEAR_PDPT_PADDR;

    // linear mapping
    let pt_linear_offset_level_4 =
        PageTable::pte_index(mirch::boot_pt_linear_mapping_base_vaddr(), 4);
    let pt_linear_offset_level_3 =
        PageTable::pte_index(mirch::boot_pt_linear_mapping_base_vaddr(), 3);

    *(paddr_to_mem(BOOT_PT_PADDR) as *mut usize).add(pt_linear_offset_level_4) =
        BOOT_PT_LINEAR_PDPT_PADDR;
    *(paddr_to_mem(BOOT_PT_LINEAR_PDPT_PADDR) as *mut usize).add(pt_linear_offset_level_3) =
        0x0 | PageTable::HUGE_BIT_MASK;

    // kernel code mapping
    let pt_kernel_offset_level_4 = PageTable::pte_index(mirch::kernel_code_base_vaddr(), 4);
    let pt_kernel_offset_level_3 = PageTable::pte_index(mirch::kernel_code_base_vaddr(), 3);

    *(paddr_to_mem(BOOT_PT_PADDR) as *mut usize).add(pt_kernel_offset_level_4) =
        BOOT_PT_KERNEL_PDPT_PADDR;
    *(paddr_to_mem(BOOT_PT_KERNEL_PDPT_PADDR) as *mut usize).add(pt_kernel_offset_level_3) =
        0x0 | PageTable::HUGE_BIT_MASK;

    super::type_pages_at(BOOT_PT_PADDR, 3, PTE_SIZE, mirch::TypedKind::PageTable).unwrap();

    page_table
}

/// Page table abstraction supported by Mirch.
///
/// Mirch implements a classic 4-level page table structure with the following characteristics:
/// - 8-byte Page Table Entries (PTEs)
/// - Standard 4KB page size
/// - Supports 48-bit virtual addresses (256TB address space)
#[derive(Debug)]
pub struct PageTable {
    root_paddr: usize,
    typed_page_paddr_to_vaddr: RefCell<BTreeMap<usize, usize>>,
}

impl PageTable {
    const PTE_PER_PAGE: usize = page_size() / PTE_SIZE;
    const PTE_INDEX_BITS: usize = Self::PTE_PER_PAGE.ilog2() as usize;
    const LEVEL_MASK: usize = Self::PTE_PER_PAGE - 1;
    const HUGE_BIT_MASK: usize = 1 << 7;

    /// The index of a VA's PTE in a page table node at the given level.
    fn pte_index(va: usize, level: usize) -> usize {
        va >> (page_size().ilog2() as usize + Self::PTE_INDEX_BITS * (level - 1)) & Self::LEVEL_MASK
    }

    /// Creates a new `PageTable` where `root_paddr` is `paddr`.
    /// Used when OS invoking `kern_miri_set_root_page_table`.
    pub fn new(paddr: usize) -> Self {
        Self { root_paddr: paddr, typed_page_paddr_to_vaddr: RefCell::new(BTreeMap::new()) }
    }

    /// Gets the root paddr of this `PageTable`.
    /// Used when OS invoking `kern_miri_get_root_page_table`
    pub fn root_paddr(&self) -> usize {
        self.root_paddr
    }

    /// Walks the page table to find the physical address corresponding to the given virtual address.
    pub fn page_walk(&self, vaddr: usize) -> Option<usize> {
        let mut current_paddr = self.root_paddr;
        let mut current_level = NR_LEVELS;

        while current_level >= 1 {
            let index = Self::pte_index(vaddr, current_level);

            let page_table_entry = unsafe {
                let pte_paddr = (current_paddr as *const usize).add(index) as usize;
                *(super::paddr_to_mem(pte_paddr) as *const usize)
            };

            const PTE_MASK: usize = 0xF_FFFF_FFFF_F000;
            current_paddr = page_table_entry & PTE_MASK;
            current_level -= 1;

            if page_table_entry & Self::HUGE_BIT_MASK > 0 {
                break;
            }
        }

        let page_offset =
            vaddr & ((super::page_size() << (current_level * Self::PTE_INDEX_BITS)) - 1);
        Some(current_paddr + page_offset)
    }

    /// Converts a physical address to a virtual address.
    ///
    /// TODO: This function is not used in the current implementation.
    /// It needs to work with a mechanism that adds a reverse mapping.
    pub fn paddr_to_vaddr(&self, paddr: usize) -> Option<usize> {
        let map = self.typed_page_paddr_to_vaddr.borrow();
        map.get(&paddr).map(|vaddr| *vaddr)
    }
}
