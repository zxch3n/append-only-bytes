use std::{fmt::Debug, mem::ManuallyDrop, ops::RangeBounds};

use crate::get_range;

pub(crate) struct RawBytes {
    ptr: *mut u8,
    capacity: usize,
}

impl Debug for RawBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Shared").field(&self.as_bytes()).finish()
    }
}

impl Drop for RawBytes {
    fn drop(&mut self) {
        // SAFETY: We are the only owner of this memory
        unsafe {
            Vec::from_raw_parts(self.ptr, 0, self.capacity);
        }
    }
}

impl RawBytes {
    #[inline(always)]
    pub fn with_capacity(capacity: usize) -> Self {
        let vec = Vec::with_capacity(capacity);
        vec.into()
    }

    #[inline(always)]
    pub fn slice(&self, range: impl RangeBounds<usize>) -> &[u8] {
        let (start, end) = get_range(range, self.capacity);
        // SAFETY: We are the only owner of this memory
        unsafe { std::slice::from_raw_parts(self.ptr.add(start), end - start) }
    }

    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8] {
        // SAFETY: We are the only owner of this memory
        unsafe { std::slice::from_raw_parts(self.ptr, self.capacity) }
    }

    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[inline(always)]
    pub fn ptr(&self) -> *mut u8 {
        self.ptr
    }
}

impl From<Vec<u8>> for RawBytes {
    fn from(vec: Vec<u8>) -> Self {
        let mut vec = ManuallyDrop::new(vec);
        Self {
            ptr: vec.as_mut_ptr(),
            capacity: vec.capacity(),
        }
    }
}
