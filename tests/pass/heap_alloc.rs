use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::UnsafeCell;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

const CAPACITY: usize = 128;

#[repr(C, align(128))]
struct AlignedBuffer([u8; CAPACITY]);

struct SingleAllocationAllocator {
    restricted: AtomicBool,
    in_use: AtomicBool,
    buffer: UnsafeCell<AlignedBuffer>,
}

impl SingleAllocationAllocator {
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
        if dbg!(!self.restricted.load(Ordering::Acquire)) {
            return unsafe { System.alloc(layout) };
        }

        if layout.size() == 0
            || layout.size() > CAPACITY
            || layout.align() > CAPACITY
            || !CAPACITY.is_multiple_of(layout.align())
        {
            return ptr::null_mut();
        }

        if self.in_use.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err()
        {
            return ptr::null_mut();
        }

        self.buffer.get().cast::<u8>()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr == self.buffer.get().cast::<u8>() {
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
    buffer: UnsafeCell::new(AlignedBuffer([0; CAPACITY])),
};

fn print_bytes() {
    let buffer_ptr = &raw const ALLOCATOR.buffer;
    dbg!(unsafe { &(*(&*buffer_ptr).get()).0 });
}

fn run_allocations() -> Result<(), &'static str> {
    let mut map = std::collections::BTreeMap::<u32, u32>::new();
    map.insert(1, 2);
    dbg!(map);
    print_bytes();

    dbg!(ALLOCATOR.in_use.load(Ordering::Acquire));
    let mut vec = std::collections::VecDeque::with_capacity(CAPACITY);
    for i in 0..CAPACITY {
        vec.push_back(i as u8);
    }
    unsafe { dbg!(ptr::write(vec.as_mut_slices().0.as_mut_ptr().add(127), 0)) };
    print_bytes();
    Ok(())
}

fn main() {
    unsafe {
        let ptr = ALLOCATOR.buffer.get() as usize as *mut u8;
        dbg!(ptr);
        ptr.write_bytes(111, CAPACITY);
    };
    ALLOCATOR.enable_restricted_mode();
    let result = run_allocations();
    ALLOCATOR.disable_restricted_mode();

    result.unwrap();
    println!("104 bytes 和 128 bytes 的分配、写入及释放均成功");
}
