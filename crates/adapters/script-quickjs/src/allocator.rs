//! Bounded wrapper around rquickjs's allocator. The failure flag survives caught JS exceptions.
//!
//! SAFETY: all pointer operations delegate to the allocator that created the pointer. We never
//! dereference pointers ourselves. A failed realloc preserves the original allocation/accounting.
#![allow(unsafe_code)]
use rquickjs::allocator::{Allocator, RustAllocator};
use std::{cell::Cell, ptr, rc::Rc};

pub struct LimitedAllocator {
    pub limit: usize,
    pub denied: Rc<Cell<bool>>,
    used: usize,
}
impl LimitedAllocator {
    pub fn new(limit: usize, denied: Rc<Cell<bool>>) -> Self {
        Self {
            limit,
            denied,
            used: 0,
        }
    }
    // Conservatively charge alignment and allocator bookkeeping, even on wasm32.
    fn charge(size: usize) -> Option<usize> {
        let align = std::mem::align_of::<u64>();
        size.checked_add(align - 1)?
            .checked_div(align)?
            .checked_mul(align)?
            .checked_add(2 * std::mem::size_of::<usize>())
    }
    fn admit(&self, old: usize, size: usize) -> Option<usize> {
        let total = Self::charge(size).and_then(|n| self.used.checked_sub(old)?.checked_add(n));
        if total.is_none_or(|n| n > self.limit) {
            self.denied.set(true);
            return None;
        }
        total
    }
}
// SAFETY: the wrapped RustAllocator owns all allocations and satisfies alignment/size guarantees.
unsafe impl Allocator for LimitedAllocator {
    fn alloc(&mut self, size: usize) -> *mut u8 {
        let Some(total) = self.admit(0, size) else {
            return ptr::null_mut();
        };
        let p = RustAllocator.alloc(size);
        if p.is_null() {
            self.denied.set(true)
        } else {
            self.used = total
        }
        p
    }
    fn calloc(&mut self, count: usize, size: usize) -> *mut u8 {
        let Some(bytes) = count.checked_mul(size) else {
            self.denied.set(true);
            return ptr::null_mut();
        };
        if bytes == 0 {
            return ptr::null_mut();
        }
        let Some(total) = self.admit(0, bytes) else {
            return ptr::null_mut();
        };
        let p = RustAllocator.calloc(count, size);
        if p.is_null() {
            self.denied.set(true)
        } else {
            self.used = total
        }
        p
    }
    unsafe fn dealloc(&mut self, p: *mut u8) {
        if p.is_null() {
            return;
        }
        // SAFETY: QuickJS hands back only live pointers from this allocator.
        let charge = unsafe { RustAllocator::usable_size(p) } + 2 * std::mem::size_of::<usize>();
        self.used -= charge;
        unsafe { RustAllocator.dealloc(p) };
    }
    unsafe fn realloc(&mut self, p: *mut u8, size: usize) -> *mut u8 {
        if p.is_null() {
            return self.alloc(size);
        }
        if size == 0 {
            unsafe { self.dealloc(p) };
            return ptr::null_mut();
        }
        // SAFETY: same provenance as dealloc; accounting is updated only on success.
        let old = unsafe { RustAllocator::usable_size(p) } + 2 * std::mem::size_of::<usize>();
        let Some(total) = self.admit(old, size) else {
            return ptr::null_mut();
        };
        let next = unsafe { RustAllocator.realloc(p, size) };
        if next.is_null() {
            self.denied.set(true)
        } else {
            self.used = total
        }
        next
    }
    unsafe fn usable_size(p: *mut u8) -> usize {
        if p.is_null() {
            0
        } else {
            unsafe { RustAllocator::usable_size(p) }
        }
    }
}
