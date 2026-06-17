use std::alloc::Layout;
use std::collections::BTreeMap;

use rustc_abi::Align;
use rustc_middle::ty::Mutability;

use super::PageTable;
use super::config::*;
use crate::alloc::MiriAllocParams;
use crate::*;

static mut PHYSICAL_MEM: PhysicalMemory = PhysicalMemory::empty();

/// Inits a page-based pseudo physical memory for the KernMiri.
///
/// The memory size and page size are defined in [`self::config`].
pub fn init_pseudo_physical_mem(config: PhysConfig) {
    *physical_mem_mut() = PhysicalMemory::new(config);
}

/// Returns an immutable reference to `PhysicalMemory` instance.
pub fn physical_mem<'a>() -> &'a PhysicalMemory {
    #[allow(clippy::deref_addrof)]
    unsafe {
        &*(&raw const PHYSICAL_MEM)
    }
}

/// Returns a mutable reference to `PhysicalMemory` instance.
pub fn physical_mem_mut<'a>() -> &'a mut PhysicalMemory {
    #[allow(clippy::deref_addrof)]
    unsafe {
        &mut *(&raw mut PHYSICAL_MEM)
    }
}

/// Convert a physical address to a pointer that point to
/// the corresponding position of the simulated physical memory.
pub fn paddr_to_mem(paddr: usize) -> *mut u8 {
    unsafe { physical_mem().mem.add(paddr) }
}

/// Checks whether a pointer points to the simulated physical memory.
pub fn is_in_physical_mem(ptr: *const ()) -> bool {
    let physical_mem = physical_mem();
    #[allow(clippy::as_conversions)]
    (physical_mem.mem as usize..physical_mem.mem as usize + total_mem_size())
        .contains(&(ptr as usize))
}

/// Creates an `Allocation` at `paddr` with `layout`.
///
/// The `paddr` is the physical address in the OS. This method will
/// put the backend bytes of created allocation in the corresponding
/// position of the simulated physical memory.
pub fn create_allocation_at(
    paddr: usize,
    layout: Layout,
    params: MiriAllocParams,
) -> Allocation<Provenance, (), MiriAllocBytes> {
    unsafe {
        let start = paddr_to_mem(paddr);
        let buffer = std::slice::from_raw_parts(start, layout.size());
        let mut allocation = Allocation::<Provenance, (), MiriAllocBytes>::from_bytes(
            std::borrow::Cow::Borrowed(buffer),
            Align::from_bytes(layout.align() as u64).unwrap(),
            Mutability::Mut,
            params,
        );

        let offset = paddr % page_size();
        if offset + layout.size() <= page_size() {
            let init_masks = &physical_mem().init_masks;
            if let Some(mask_allocation) = init_masks.get(&(paddr - offset)) {
                let init_copy = mask_allocation
                    .init_mask()
                    .prepare_copy((offset..offset + layout.size()).into());
                allocation.init_mask_apply_copy(init_copy, (0..layout.size()).into(), 1);
            }
        }
        // FIXME: what should we do when the allocation needs crossing pages?
        allocation
    }
}

/// Frees `count` pages at `paddr` in the simulated physical memory.
pub fn free_allocations<'tcx>(
    this: &mut MiriInterpCx<'tcx>,
    paddr: usize,
    count: usize,
) -> InterpResult<'tcx, ()> {
    let mut global_state = this.machine.alloc_addresses.borrow_mut();
    let physical_mem = physical_mem_mut();

    for page_index in 0..count {
        let page_paddr = paddr + page_size() * page_index;
        let page_info = physical_mem.page_states[paddr / page_size()];

        if let PageState::Typed { page_type: _, slot_size } = page_info {
            for index in 0..page_size() / slot_size {
                let actual_paddr = page_paddr + index * slot_size;
                let pos = global_state
                    .int_to_ptr_map
                    .binary_search_by_key(&(actual_paddr as u64), |(addr, _)| *addr);
                if let Ok(pos) = pos {
                    let dead_id = global_state.int_to_ptr_map[pos].1;
                    global_state.int_to_ptr_map.remove(pos);
                    global_state.exposed.remove(&dead_id);
                    global_state.base_paddr.remove(&dead_id);
                    this.memory.alloc_map().remove(&dead_id);
                }
            }
        }
        if physical_mem.page_states[paddr / page_size()] == PageState::Unused {
            throw_ub_format!(
                "Page state UB: Attempting to release an unused page. The paddr is 0x{:x}",
                page_paddr
            );
        }
        physical_mem.set_page_state(page_paddr, PageState::Unused);
        physical_mem.remove_init_mask(page_paddr);
    }
    interp_ok(())
}

/// Types `count` pages starting from `paddr` in the simulated physical memory.
pub fn type_pages_at<'tcx>(
    paddr: usize,
    count: usize,
    slot_size: usize,
    page_type: TypedKind,
) -> InterpResult<'tcx, ()> {
    let physical_mem = physical_mem_mut();
    for page_index in 0..count {
        let page_paddr = paddr + page_size() * page_index;
        physical_mem.set_page_state(page_paddr, PageState::Typed { page_type, slot_size });
    }

    interp_ok(())
}

/// Copies `len` bytes from `src` to `dst` in the simulated physical memory.
pub fn physical_copy(dst: usize, src: usize, len: usize) {
    unsafe {
        let src_ptr = paddr_to_mem(src);
        let dst_ptr = paddr_to_mem(dst);

        core::ptr::copy(src_ptr, dst_ptr, len);
    }

    // todo: mask copy
}

/// Removes the initialization mask for the page at `paddr`.
pub fn remove_init_mask(paddr: usize) {
    physical_mem_mut().init_masks.remove(&paddr);
}

/// Checks the page state of the page at `paddr`.
pub fn check_page_state(paddr: usize, page_state: PageState) {
    let index = paddr / page_size();
    let physical_mem = physical_mem();
    if physical_mem.page_states[index] != page_state {
        panic!("Page state UB: current page state is {:?}", physical_mem.page_states[index]);
    }
}

/// Sets the page state of the page at `paddr`.
pub fn set_page_state(paddr: usize, page_state: PageState) {
    let index = paddr / page_size();
    physical_mem_mut().page_states[index] = page_state;
}

/// Sets the root page table.
pub fn set_page_table(page_table: PageTable) {
    physical_mem_mut().page_table = Some(page_table);
}

/// Walks the page table to find the physical address corresponding to the given virtual address.
///
///  If the page table is not set, it calls the provided function and return the result.
pub fn page_walk_or<F>(vaddr: usize, func: F) -> Option<usize>
where
    F: FnOnce() -> usize,
{
    if let Some(page_table) = &physical_mem().page_table {
        page_table.page_walk(vaddr)
    } else {
        Some(func())
    }
}

/// Inserts an initialization mask for the page at `paddr`.
/// Currently, only be called in the kern_miri_alloc_pages shim.
pub fn insert_init_mask(this: &MiriInterpCx<'_>, paddr: usize, params: MiriAllocParams) {
    unsafe {
        let layout = Layout::from_size_align_unchecked(page_size(), 1);
        let mut allocation = create_allocation_at(paddr, layout, params);
        allocation.write_uninit(this, (0..page_size()).into());

        physical_mem_mut().init_masks.insert(paddr, allocation);
    }
}

pub struct PhysicalMemory {
    pub mem: *mut u8,
    pub page_states: Vec<PageState>,
    pub init_masks: BTreeMap<usize, Allocation<Provenance, (), MiriAllocBytes>>,
    pub page_table: Option<PageTable>,
}

impl PhysicalMemory {
    pub const fn empty() -> Self {
        Self {
            mem: std::ptr::null_mut(),
            page_states: Vec::new(),
            init_masks: BTreeMap::new(),
            page_table: None,
        }
    }

    pub fn new(config: PhysConfig) -> Self {
        super::config::init(config);
        let mem = unsafe {
            std::alloc::alloc_zeroed(
                Layout::from_size_align(total_mem_size(), page_size()).unwrap(),
            )
        };

        let mut page_states = vec![PageState::Unused; total_page_num()];
        #[expect(
            clippy::needless_range_loop,
            reason = "kernel code section is the first part in all pages, but there are left space for free pages"
        )]
        for i in 0..kernel_code_page_num() {
            page_states[i] =
                PageState::Typed { page_type: TypedKind::Interpreter, slot_size: page_size() };
        }

        Self { mem, page_states, init_masks: BTreeMap::new(), page_table: None }
    }
}

impl PhysicalMemory {
    pub fn remove_init_mask(&mut self, paddr: usize) {
        self.init_masks.remove(&paddr);
    }

    pub fn check_page_state(&self, paddr: usize, page_state: PageState) {
        let index = paddr / page_size();
        if self.page_states[index] != page_state {
            panic!("Page state UB: current page state is {:?}", self.page_states[index]);
        }
    }

    pub fn set_page_state(&mut self, paddr: usize, page_state: PageState) {
        let index = paddr / page_size();
        self.page_states[index] = page_state;
    }
}

/// Additional state settings for the physical pages maintained by Miri.
/// Initially, all pages are set to `Unused`.
/// PageState transformation:
/// `Unused` --allocate--> `Untyped` --retype--> `Typed`.
/// `Untyped`/`Typed` --deallocate--> `Unused`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PageState {
    Unused,
    Untyped,
    /// Page type means what the page is used for.
    /// Slot size means the slot element is the page has the size and alignment.
    Typed {
        page_type: TypedKind,
        slot_size: usize,
    },
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TypedKind {
    Slab = 1,
    PageTable = 2,
    Stack = 3,
    Interpreter = 4,
}

impl TypedKind {
    pub fn from_usize(value: usize) -> Option<Self> {
        match value {
            1 => Some(TypedKind::Slab),
            2 => Some(TypedKind::PageTable),
            3 => Some(TypedKind::Stack),
            4 => Some(TypedKind::Interpreter),
            _ => None,
        }
    }
}
