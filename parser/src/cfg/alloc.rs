//! This module contains a helper struct for generating 'static lifetime allocations while still
//! keeping track of them so that they can be freed later.

use parking_lot::Mutex;
use std::sync::Arc;

/// This struct tracks the allocations that are leaked by its provided methods and frees them when
/// dropped. The `new` function is unsafe because dropping the struct can create dangling
/// references. Care must be taken to ensure that all allocations made by this struct's methods are
/// no longer referenced when the struct gets dropped.
///
/// In practice, this is not difficult to do in the `cfg` module which only exposes a single public
/// method.
pub(crate) struct Allocations {
    allocations: Mutex<Vec<Allocation>>,
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct Allocation {
    ptr: usize,
    len: usize,
}

impl std::fmt::Debug for Allocations {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Allocations").finish()
    }
}

impl Drop for Allocations {
    fn drop(&mut self) {
        log::debug!(
            "freeing allocations of length {}",
            self.allocations.lock().len()
        );
        for a in self.allocations.lock().iter().rev().copied() {
            log::debug!("freeing ptr 0x{:x} len{}", a.ptr, a.len);
            unsafe {
                drop(Box::<[u8]>::from_raw(std::slice::from_raw_parts_mut(
                    a.ptr as *mut u8,
                    a.len,
                )))
            };
        }
    }
}

impl Allocations {
    /// Create a new allocations group.
    ///
    /// # Safety
    ///
    /// Ensure that all associated allocations are no longer referenced before dropping all
    /// clones of the `Arc`.
    pub(crate) unsafe fn new() -> Arc<Self> {
        Arc::new(Self {
            allocations: Mutex::new(vec![]),
        })
    }

    /// Returns a `&'static T` by leaking a newly created Box of `v`.
    pub(crate) fn sref<T>(&self, v: T) -> &'static T {
        let p = Box::into_raw(Box::new(v));
        if (p as usize) < 16 {
            panic!("sref bad ptr");
        }
        log::debug!(
            "sref type: {}, ptr:{p:?} sz:{}",
            std::any::type_name::<T>(),
            std::mem::size_of::<T>()
        );
        self.allocations.lock().push(Allocation {
            ptr: p as usize,
            len: std::mem::size_of::<T>(),
        });
        Box::leak(unsafe { Box::from_raw(p) })
    }

    pub(crate) fn bref_slice<T>(&self, v: Box<[T]>) -> &'static [T] {
        // An empty slice has no backing allocation. `Box<[T]>` is a fat pointer so the leaked return
        // will contain a length of 0 and an invalid pointer.
        if !v.is_empty() {
            let p = v.as_ptr();
            log::debug!(
                "bref_slice type: {}, ptr:{p:?} sz:{}",
                std::any::type_name::<T>(),
                std::mem::size_of::<T>()
            );
            self.allocations.lock().push(Allocation {
                ptr: p as usize,
                len: std::mem::size_of::<T>() * v.len(),
            });
        }
        Box::leak(v)
    }

    /// Returns a &'static [&'static T] from a `Vec<T>` by converting to a boxed slice and leaking it.
    pub(crate) fn sref_vec<T>(&self, v: Vec<T>) -> &'static [T] {
        log::debug!("sref_vec {}", std::any::type_name::<T>());
        self.bref_slice(v.into_boxed_slice())
    }

    /// Returns a `&'static [&'static T]` by leaking a newly created box and boxed slice of `v`.
    pub(crate) fn sref_slice<T>(&self, v: T) -> &'static [&'static T] {
        log::debug!("sref_slice {}", std::any::type_name::<T>());
        self.bref_slice(vec![self.sref(v)].into_boxed_slice())
    }
}
