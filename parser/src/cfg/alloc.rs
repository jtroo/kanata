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
///
/// To avoid leaks, types transformed to &'static by this struct
/// should not contain nested allocations,
/// or if they do, the nested allocations should also
/// be managed by this struct.
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
                drop(Box::<[u8]>::from_raw(std::ptr::slice_from_raw_parts_mut(
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

    /// Returns a `&'static str` by leaking a String.
    pub(crate) fn sref_str(&self, v: String) -> &'static str {
        if v.capacity() == 0 {
            ""
        } else {
            let len = v.len();
            let s = v.leak();
            self.allocations.lock().push(Allocation {
                ptr: s.as_ptr() as usize,
                len,
            });
            s
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_zero_capacity_strings_take_the_fast_path() {
        let a = unsafe { Allocations::new() };

        // Zero-capacity empty strings (`String::new()`, `""`, `with_capacity(0)`)
        // have no backing allocation, so they must take the `""` fast path: return a
        // static empty str and record NO tracked allocation. The old guard
        // `!v.capacity() == 0` (bitwise-NOT precedence) let these fall through to
        // the leak branch, recording the String's dangling buffer pointer
        // (`NonNull::dangling` == 0x1 for u8) as if it were a live allocation.
        for input in [String::new(), "".to_string(), String::with_capacity(0)] {
            assert_eq!(input.capacity(), 0);
            let tracked_before = a.allocations.lock().len();
            let out = a.sref_str(input);
            assert_eq!(out, "");
            assert_eq!(
                a.allocations.lock().len(),
                tracked_before,
                "zero-capacity string must not be tracked as an allocation"
            );
        }
    }

    #[test]
    fn empty_string_with_reserved_capacity_is_still_tracked() {
        // An empty string that nonetheless owns a heap allocation (capacity > 0)
        // must take the tracking branch so its allocation is freed on drop. This is
        // why the guard is `capacity() == 0` rather than `is_empty()`: `is_empty()`
        // would route this case to the `""` fast path and leak the reserved buffer.
        let a = unsafe { Allocations::new() };
        let mut reserved = String::from("x");
        reserved.clear();
        assert!(reserved.is_empty());
        assert!(reserved.capacity() >= 1);

        let tracked_before = a.allocations.lock().len();
        let out = a.sref_str(reserved);
        assert_eq!(out, "");
        assert_eq!(
            a.allocations.lock().len(),
            tracked_before + 1,
            "empty-but-allocated string must be tracked so it is freed"
        );
    }

    #[test]
    fn nonempty_string_is_tracked_with_a_real_pointer() {
        let a = unsafe { Allocations::new() };

        let tracked_before = a.allocations.lock().len();
        let out = a.sref_str("hello".to_string());
        assert_eq!(out, "hello");

        let allocs = a.allocations.lock();
        assert_eq!(allocs.len(), tracked_before + 1);
        // A tracked string must point at a real heap allocation, not the empty-slice
        // dangling sentinel, so `Allocations::drop` frees a valid box.
        let last = allocs.last().unwrap();
        assert_eq!(last.len, 5);
        assert_ne!(last.ptr, String::new().leak().as_ptr() as usize);
    }
}
