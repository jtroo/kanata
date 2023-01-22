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
    allocations: Mutex<Vec<usize>>,
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
        for p in self.allocations.lock().iter().rev().copied() {
            unsafe { drop(Box::from_raw(p as *mut usize)) };
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
    pub(super) unsafe fn new() -> Arc<Self> {
        Arc::new(Self {
            allocations: Mutex::new(vec![]),
        })
    }

    /// Returns a `&'static T` by leaking the existing box.
    pub(super) fn bref<T>(&self, v: Box<T>) -> &'static T {
        let p = Box::into_raw(v);
        if (p as usize) < 16 {
            panic!("bref bad ptr");
        }
        self.allocations.lock().push(p as usize);
        Box::leak(unsafe { Box::from_raw(p) })
    }

    /// Returns a `&'static T` by leaking a newly created Box of `v`.
    pub(super) fn sref<T>(&self, v: T) -> &'static T {
        let p = Box::into_raw(Box::new(v));
        if (p as usize) < 16 {
            panic!("sref bad ptr");
        }
        self.allocations.lock().push(p as usize);
        Box::leak(unsafe { Box::from_raw(p) })
    }

    fn bref_slice<T>(&self, v: Box<[T]>) -> &'static [T] {
        // An empty slice has no backing allocation. `Box<[T]>` is a fat pointer so the leaked return
        // will contain a length of 0 and an invalid pointer.
        if !v.is_empty() {
            self.allocations.lock().push(v.as_ptr() as usize);
        }
        Box::leak(v)
    }

    /// Returns a &'static [&'static T] from a `Vec<T>` by converting to a boxed slice and leaking it.
    pub(super) fn sref_vec<T>(&self, v: Vec<T>) -> &'static [T] {
        self.bref_slice(v.into_boxed_slice())
    }

    /// Returns a `&'static [&'static T]` by leaking a newly created box and boxed slice of `v`.
    pub(super) fn sref_slice<T>(&self, v: T) -> &'static [&'static T] {
        self.bref_slice(vec![self.sref(v)].into_boxed_slice())
    }
}
