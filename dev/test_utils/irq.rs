//! KMiri interrupt shims shared by the IRQ regression tests.

#![allow(dead_code)]

unsafe extern "Rust" {
    fn kern_miri_irq_is_enabled() -> bool;
    fn kern_miri_irq_disable();
    fn kern_miri_irq_enable();
    fn kern_miri_register_timer_irq(handler: extern "Rust" fn());
    fn kern_miri_test_raise_irq(handler: extern "Rust" fn());
}

pub struct IrqGuard {
    was_enabled: bool,
}

impl IrqGuard {
    pub fn new() -> Self {
        let was_enabled = unsafe { kern_miri_irq_is_enabled() };
        unsafe { kern_miri_irq_disable() };
        Self { was_enabled }
    }
}

impl Drop for IrqGuard {
    fn drop(&mut self) {
        if self.was_enabled {
            unsafe { kern_miri_irq_enable() };
        }
    }
}

pub fn raise_irq(handler: extern "Rust" fn()) {
    unsafe { kern_miri_test_raise_irq(handler) };
}

pub fn register_timer_irq(handler: extern "Rust" fn()) {
    unsafe { kern_miri_register_timer_irq(handler) };
}
