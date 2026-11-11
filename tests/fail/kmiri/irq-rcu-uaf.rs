// A small executable model of #3896 for KMiri. The Asterinas integration
// test with the same name exercises the real OSTD guard, RCU, and scheduler.

use std::marker::PhantomData;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

static IRQ_ENABLED: AtomicBool = AtomicBool::new(true);

struct IrqGuard {
    was_enabled: bool,
}

impl IrqGuard {
    fn new() -> Self {
        let was_enabled = IRQ_ENABLED.swap(false, Ordering::SeqCst);
        Self { was_enabled }
    }
}

impl Drop for IrqGuard {
    fn drop(&mut self) {
        if self.was_enabled {
            IRQ_ENABLED.store(true, Ordering::SeqCst);
        }
    }
}

struct Payload(usize);

struct MiniRcu {
    current: AtomicPtr<Payload>,
    retired: AtomicPtr<Payload>,
}

struct StaleRef<'a> {
    ptr: *const Payload,
    _guard: PhantomData<&'a IrqGuard>,
}

impl MiniRcu {
    fn new(value: Box<Payload>) -> Self {
        Self {
            current: AtomicPtr::new(Box::into_raw(value)),
            retired: AtomicPtr::new(ptr::null_mut()),
        }
    }

    // Like OSTD's BoxRef, retain a raw pointer while tying its lifetime to
    // the InAtomicMode witness.
    fn read_with<'a>(&'a self, _guard: &'a IrqGuard) -> StaleRef<'a> {
        StaleRef { ptr: self.current.load(Ordering::Acquire), _guard: PhantomData }
    }

    fn update(&self, value: Box<Payload>) {
        let old = self.current.swap(Box::into_raw(value), Ordering::AcqRel);
        assert!(self.retired.swap(old, Ordering::AcqRel).is_null());
    }

    // Models switch_to_task() completing the RCU grace period.
    fn simulated_task_switch(&self) {
        assert!(IRQ_ENABLED.load(Ordering::SeqCst));
        let old = self.retired.swap(ptr::null_mut(), Ordering::AcqRel);
        if !old.is_null() {
            // SAFETY: Deliberately frees the retired allocation while the
            // test retains a raw pointer to model the invalidated RCU read.
            unsafe { drop(Box::from_raw(old)) };
        }
    }
}

impl Drop for MiniRcu {
    fn drop(&mut self) {
        let current = self.current.swap(ptr::null_mut(), Ordering::AcqRel);
        if !current.is_null() {
            // SAFETY: `current` is the unique allocation still owned by the
            // RCU slot at this point.
            unsafe { drop(Box::from_raw(current)) };
        }
        let retired = self.retired.swap(ptr::null_mut(), Ordering::AcqRel);
        if !retired.is_null() {
            // SAFETY: The retired slot uniquely owns this allocation unless
            // the modeled grace period has already reclaimed it.
            unsafe { drop(Box::from_raw(retired)) };
        }
    }
}

fn main() {
    let rcu = MiniRcu::new(Box::new(Payload(0xAA)));
    let outer = IrqGuard::new();
    let inner = IrqGuard::new();
    let stale = rcu.read_with(&inner);

    drop(outer); // Non-LIFO drop re-enables IRQs while `inner` is still live.
    assert!(IRQ_ENABLED.load(Ordering::SeqCst));
    rcu.update(Box::new(Payload(0xBB)));
    rcu.simulated_task_switch();

    // SAFETY: This intentionally models BoxRef::deref_target() reading
    // through its invalidated pointer after the RCU grace period.
    let _stale_value = unsafe { ptr::read_volatile(ptr::addr_of!((*stale.ptr).0)) };
    //~^ ERROR: has been freed

    drop(inner);
}
