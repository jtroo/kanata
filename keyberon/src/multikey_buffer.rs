//! Module for `MultiKeyBuffer`.

use std::{array, slice};

use crate::action::{Action, ONE_SHOT_MAX_ACTIVE};
use crate::key_code::KeyCode;

// Presumably this should be plenty.
// ONE_SHOT_MAX_ACTIVE is already likely unreasonably large enough.
// This buffer capacity adds more onto that,
// just in case somebody finds a way to use all of the one-shot capacity.
const BUFCAP: usize = ONE_SHOT_MAX_ACTIVE + 4;

/// This is an unsafe container that enables a mutable Action::MultipleKeyCodes.
pub(crate) struct MultiKeyBuffer<'a, T> {
    buf: [KeyCode; BUFCAP],
    size: usize,
    ptr: *mut &'static [KeyCode],
    ac: *mut Action<'a, T>,
}

unsafe impl<T> Send for MultiKeyBuffer<'_, T> {}

impl<'a, T> MultiKeyBuffer<'a, T> {
    /// Create a new instance of `MultiKeyBuffer`.
    ///
    /// # Safety
    ///
    /// The program should not have any references to the inner buffer when the struct is dropped.
    pub(crate) unsafe fn new() -> Self {
        Self {
            buf: array::from_fn(|_| KeyCode::Escape),
            size: 0,
            ptr: Box::leak(Box::new(slice::from_raw_parts(
                core::ptr::NonNull::dangling().as_ptr(),
                0,
            ))),
            ac: Box::leak(Box::new(Action::NoOp)),
        }
    }

    /// Set the current size of the buffer to zero.
    ///
    /// # Safety
    ///
    /// The program should not have any references to the inner buffer.
    pub(crate) unsafe fn clear(&mut self) {
        self.size = 0;
    }

    /// Push to the end of the buffer. If the buffer is full, this silently fails.
    ///
    /// # Safety
    ///
    /// The program should not have any references to the inner buffer.
    pub(crate) unsafe fn push(&mut self, kc: KeyCode) {
        if self.size < BUFCAP {
            self.buf[self.size] = kc;
            self.size += 1;
        }
    }

    /// Get a reference to the inner buffer in the form of an `Action`.
    /// The `Action` will be the variant `MultipleKeyCodes`,
    /// containing all keys that have been pushed.
    ///
    /// # Safety
    ///
    /// The program should not have any references to the inner buffer before calling.
    /// The program should not mutate the buffer after calling this function until after the
    /// returned reference is dropped.
    pub(crate) unsafe fn get_ref(&self) -> &'a Action<'a, T> {
        *self.ac = Action::NoOp;
        *self.ptr = slice::from_raw_parts(self.buf.as_ptr(), self.size);
        *self.ac = Action::MultipleKeyCodes(&*self.ptr);
        &*self.ac
    }
}

impl<T> Drop for MultiKeyBuffer<'_, T> {
    fn drop(&mut self) {
        unsafe {
            drop(Box::from_raw(self.ac));
            drop(Box::from_raw(self.ptr));
        }
    }
}
