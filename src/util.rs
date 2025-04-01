//! HAL utilities used across modules.
#![allow(dead_code)]

use core::{
    marker::PhantomData,
    sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
};

/// Marker trait to denote that AtomicSlice can be used for mutable slices.
pub(crate) struct Mutable;

/// Marker trait to denote that AtomicSlice can be used for immutable slices.
pub(crate) struct Immutable;

/// Implementation of a slice for which the ownership is erased.
///
/// Intended to access slices from the application in ISR contexts.
///
pub(crate) struct AtomicSlice<T, M> {
    /// Pointer to the first element in the slice.
    ///
    /// Unsafe: can be NULL to indicate that the slice is not set.
    /// Unsafe: user is responsible that it points to valid memory when used.
    ptr: AtomicPtr<T>,

    /// Number of elements in slice.
    len: AtomicUsize,

    /// Marker to denote the mutability requirement of the slice.
    _mode: PhantomData<M>,
}

impl<T, M> AtomicSlice<T, M> {
    pub const fn new() -> Self {
        Self {
            ptr: AtomicPtr::new(core::ptr::null_mut()),
            len: AtomicUsize::new(0),
            _mode: PhantomData,
        }
    }

    fn inner_store(&self, slice: &[T]) {
        // Set length to zero just in case the slice is loaded after we've set the pointer,
        // but before the length is updated.
        self.len.store(0, Ordering::Release);
        self.ptr.store(slice.as_ptr() as *mut T, Ordering::Release);
        self.len.store(slice.len(), Ordering::Release);
    }

    unsafe fn inner_load(&self) -> &'static mut [T] {
        let ptr = self.ptr.load(Ordering::Acquire);
        let len = self.len.load(Ordering::Acquire);

        if !ptr.is_null() {
            core::slice::from_raw_parts_mut(ptr, len)
        } else {
            &mut []
        }
    }

    /// Store a slice that is empty.
    pub fn store_empty(&self) {
        self.ptr.store(core::ptr::null_mut(), Ordering::Release);
    }
}

impl<T> AtomicSlice<T, Immutable> {
    /// Store the slice semi-atomically.
    pub fn store(&self, slice: &[T]) {
        self.inner_store(slice);
    }

    /// Load the slice semi-atomically.
    ///
    /// This is an unsafe operation because the lifetime of the slice has been erased.
    /// Ensure that the slice still is in memory when using it.
    ///
    /// If the load occurs at the same time a store could occur, a slice may be given back with
    /// a zero length. If the length of the slice is zero, check it and try again later.
    /// This is not a consideration if the store is guaranteed to happen before load is called.
    pub unsafe fn load(&self) -> &'static [T] {
        self.inner_load()
    }
}

impl<T> AtomicSlice<T, Mutable> {
    /// Store the slice semi-atomically.
    pub fn store(&self, slice: &mut [T]) {
        self.inner_store(slice);
    }

    /// Load the slice semi-atomically.
    ///
    /// This is an unsafe operation because the lifetime of the slice has been erased.
    /// Ensure that the slice still is in memory when using it.
    ///
    /// Also unsafe because it allows for multiple mutable references to be generated,
    /// so ensure that only a single mutable reference is used at the same time.
    ///
    /// If the load occurs at the same time a store could occur, a slice may be given back with
    /// a zero length. If the length of the slice is zero, check it and try again later.
    /// This is not a consideration if the store is guaranteed to happen before load is called.
    pub unsafe fn load(&self) -> &'static mut [T] {
        self.inner_load()
    }
}
