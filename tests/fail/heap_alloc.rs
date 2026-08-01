use std::alloc::{GlobalAlloc, Layout, System};
use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

const CAPACITY: usize = 128;

#[repr(C)]
struct Data104([u8; 104]);

const _: () = assert!(size_of::<Data104>() == 104);

#[repr(C, align(128))]
struct AlignedBuffer([u8; CAPACITY]);

static mut BUFFER: AlignedBuffer = AlignedBuffer([0; CAPACITY]);

// Keeps the provenance produced by `Box::into_raw` alive across the logical
// deallocation. AtomicPtr stores and reloads the pointer together with its provenance.
static FIRST_PTR: AtomicPtr<u8> = AtomicPtr::new(ptr::null_mut());

struct SingleAllocationAllocator {
    restricted: AtomicBool,
    in_use: AtomicBool,
    buffer: NonNull<u8>,
}

impl SingleAllocationAllocator {
    fn buffer_ptr(&self) -> *mut u8 {
        let first = FIRST_PTR.load(Ordering::Acquire);
        if first.is_null() { self.buffer.as_ptr() } else { first }
    }

    fn enable_restricted_mode(&self) {
        self.restricted.store(true, Ordering::Release);
    }

    fn disable_restricted_mode(&self) {
        self.restricted.store(false, Ordering::Release);
    }
}

// `in_use` guarantees exclusive access to `buffer`.
unsafe impl Sync for SingleAllocationAllocator {}

unsafe impl GlobalAlloc for SingleAllocationAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !self.restricted.load(Ordering::Acquire) {
            return unsafe { System.alloc(layout) };
        }

        if layout.size() < 65
            || layout.size() > CAPACITY
            || layout.align() > CAPACITY
            || CAPACITY % layout.align() != 0
        {
            return unsafe { System.alloc(layout) };
        }

        if self.in_use.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err()
        {
            return ptr::null_mut();
        }

        self.buffer_ptr()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr == self.buffer_ptr() {
            self.in_use.store(false, Ordering::Release);
        } else {
            unsafe { System.dealloc(ptr, layout) };
        }
    }
}

#[global_allocator]
static ALLOCATOR: SingleAllocationAllocator = SingleAllocationAllocator {
    restricted: AtomicBool::new(false),
    in_use: AtomicBool::new(false),
    buffer: NonNull::new((&raw mut BUFFER).cast()).unwrap(),
};

fn run_allocations() -> Result<(), &'static str> {
    unsafe {
        let layout_104 = Layout::new::<Data104>();
        let first = Box::new(Data104(std::array::from_fn(|i| i as u8)));

        // Box::into_raw creates a raw-pointer tag derived through `&mut Data104`,
        // so its borrow-stack permission covers exactly 104 bytes.
        let first = Box::into_raw(first).cast::<u8>();
        FIRST_PTR.store(first, Ordering::Release);

        // This allocator only marks the static buffer as logically free. The
        // underlying Miri allocation and the globally stored tag remain alive.
        std::alloc::dealloc(first, layout_104);

        let layout_128 = Layout::from_size_align(CAPACITY, 8).unwrap();
        let second = std::alloc::alloc(layout_128);
        if second.is_null() {
            return Err("second allocation failed");
        }
        assert_eq!(second, first);

        // Under Stacked Borrows, writes 0..104 use FIRST_PTR's saved tag. The
        // write at offset 104 (0x68) should fail because that tag was only
        // granted for the Data104 range.
        for i in 0..CAPACITY {
            second.add(i).write_volatile(i as u8);
        }

        std::alloc::dealloc(second, layout_128);
    }

    Ok(())
}

fn main() {
    ALLOCATOR.enable_restricted_mode();
    let result = run_allocations();
    ALLOCATOR.disable_restricted_mode();

    result.unwrap();
    println!("the 128-byte write unexpectedly passed Stacked Borrows");
}
