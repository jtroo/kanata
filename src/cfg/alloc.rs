//! This module contains helpers for tracking and freeing allocations done as part of parsing the
//! kanata configuration and creating the keyberon layout.
//!
//! # Safety
//!
//! This module is not threadsafe - multiple configurations must not parse in parallel. A lock
//! should be implemented around config generation.

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::cell::Cell;

static NEW_ALLOCATIONS: Lazy<Mutex<Cell<Vec<usize>>>> = Lazy::new(|| Mutex::new(Cell::new(vec![])));

/// Contains all of the allocations made since the previous call to `claim_new_allocations`, or
/// since beginning of the program if no previous call has been done yet.
pub(crate) struct Allocations {
    allocations: Vec<usize>,
}

impl Drop for Allocations {
    fn drop(&mut self) {
        log::debug!("freeing allocations of length {}", self.allocations.len());
        for p in self.allocations.iter().rev().copied() {
            unsafe { drop(Box::from_raw(p as *mut usize)) };
        }
    }
}

/// Useful to free allocations made as part of parsing an invalid configuration.
///
/// # Safety
///
/// Ensure that nothing's actually referencing these allocations anymore.
pub(super) unsafe fn free_new_allocations() {
    let mut new = NEW_ALLOCATIONS.lock();
    for p in new.get_mut().iter().rev().copied() {
        drop(Box::from_raw(p as *mut usize));
    }
    new.get_mut().clear();
    new.get_mut().shrink_to_fit();
}

/// Use to take ownership of allocations for dropping them with the associated object.
///
/// # Safety
///
/// Ensure that the claimed allocations are dropped as part of dropping the object that's actually
/// using these allocations.
pub(super) unsafe fn claim_new_allocations() -> Allocations {
    let mut new = NEW_ALLOCATIONS.lock();
    log::debug!("allocation lengths, active: {}", new.get_mut().len());
    let mut allocations = new.replace(vec![]);
    allocations.shrink_to_fit();
    Allocations { allocations }
}

/// Returns a `&'static T` by leaking the existing box.
pub(super) fn bref<T>(v: Box<T>) -> &'static T {
    let p = Box::into_raw(v);
    if (p as usize) < 16 {
        panic!("bref bad ptr");
    }
    NEW_ALLOCATIONS.lock().get_mut().push(p as usize);
    Box::leak(unsafe { Box::from_raw(p) })
}

/// Returns a `&'static T` by leaking a newly created Box of `v`.
pub(super) fn sref<T>(v: T) -> &'static T {
    let p = Box::into_raw(Box::new(v));
    if (p as usize) < 16 {
        panic!("sref bad ptr");
    }
    NEW_ALLOCATIONS.lock().get_mut().push(p as usize);
    Box::leak(unsafe { Box::from_raw(p) })
}

fn bref_slice<T>(v: Box<[T]>) -> &'static [T] {
    // An empty slice has no backing allocation. `Box<[T]>` is a fat pointer so the leaked return
    // will contain a length of 0 and an invalid pointer.
    if !v.is_empty() {
        NEW_ALLOCATIONS.lock().get_mut().push(v.as_ptr() as usize);
    }
    Box::leak(v)
}

/// Returns a &'static [&'static T] from a `Vec<T>` by converting to a boxed slice and leaking it.
pub(super) fn sref_vec<T>(v: Vec<T>) -> &'static [T] {
    bref_slice(v.into_boxed_slice())
}

/// Returns a `&'static [&'static T]` by leaking a newly created box and boxed slice of `v`.
pub(super) fn sref_slice<T>(v: T) -> &'static [&'static T] {
    bref_slice(vec![sref(v)].into_boxed_slice())
}
