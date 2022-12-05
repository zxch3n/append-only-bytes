use std::{
    cell::UnsafeCell,
    fmt::Debug,
    mem::ManuallyDrop,
    ops::{Deref, Index, RangeBounds},
    sync::Arc,
};

struct Shared {
    ptr: *mut u8,
    capacity: usize,
}

impl Debug for Shared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unsafe {
            let slice = std::slice::from_raw_parts(self.ptr, self.capacity);
            f.debug_tuple("Shared").field(&slice).finish()
        }
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        unsafe {
            Vec::from_raw_parts(self.ptr, 0, self.capacity);
        }
    }
}

impl Shared {
    fn slice(&self, range: impl RangeBounds<usize>) -> &[u8] {
        let (start, end) = get_range(range, self.capacity);
        unsafe { std::slice::from_raw_parts(self.ptr.add(start), end - start) }
    }
}

impl From<Vec<u8>> for Shared {
    fn from(vec: Vec<u8>) -> Self {
        let mut vec = ManuallyDrop::new(vec);
        Self {
            ptr: vec.as_mut_ptr(),
            capacity: vec.capacity(),
        }
    }
}

#[derive(Debug)]
pub struct AppendOnlyBytes {
    raw: Arc<Shared>,
    len: usize,
}

#[derive(Debug)]
pub struct BytesSlice {
    raw: Arc<Shared>,
    start: usize,
    end: usize,
}

unsafe impl Send for AppendOnlyBytes {}

impl AppendOnlyBytes {
    #[inline(always)]
    pub fn new() -> Self {
        Self::with_capacity(8)
    }

    #[inline(always)]
    fn raw(&self) -> &[u8] {
        self.raw.slice(..)
    }

    #[inline(always)]
    pub fn with_capacity(capacity: usize) -> Self {
        let mut vec = Vec::with_capacity(capacity);
        #[allow(clippy::uninit_vec)]
        unsafe {
            vec.set_len(capacity)
        };
        let raw = Arc::new(vec.into());
        Self { raw, len: 0 }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.raw().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline(always)]
    pub fn push_slice(&mut self, slice: &[u8]) {
        self.reserve(slice.len());
        unsafe {
            std::ptr::copy_nonoverlapping(slice.as_ptr(), self.raw.ptr.add(self.len), slice.len());
            self.len += slice.len();
        }
    }

    #[inline(always)]
    pub fn push_str(&mut self, slice: &str) {
        self.push_slice(slice.as_bytes());
    }

    #[inline(always)]
    pub fn push(&mut self, byte: u8) {
        self.reserve(1);
        unsafe {
            std::ptr::write(self.raw.ptr.add(self.len), byte);
            self.len += 1;
        }
    }

    #[inline]
    fn reserve(&mut self, size: usize) {
        if self.len() + size > self.capacity() {
            let target_capacity = self.len() + size;
            let mut new_capacity = self.capacity() * 2;
            while new_capacity < target_capacity {
                new_capacity *= 2;
            }

            let src = std::mem::replace(self, Self::with_capacity(new_capacity));
            // SAFETY: copy from src to dst, both have at least the capacity of src.len()
            unsafe {
                std::ptr::copy_nonoverlapping(src.raw.ptr, self.raw.ptr, src.len());
                self.len = src.len();
            }
        }
    }

    #[inline]
    pub fn slice_str(&self, range: impl RangeBounds<usize>) -> Result<&str, std::str::Utf8Error> {
        let (start, end) = get_range(range, self.len());
        std::str::from_utf8(&self.raw()[start..end])
    }

    #[inline]
    pub fn slice(&self, range: impl RangeBounds<usize>) -> BytesSlice {
        let (start, end) = get_range(range, self.len());
        BytesSlice {
            raw: self.raw.clone(),
            start,
            end,
        }
    }

    #[inline(always)]
    pub fn to_slice(self) -> BytesSlice {
        BytesSlice {
            end: self.len(),
            raw: self.raw,
            start: 0,
        }
    }
}

#[inline(always)]
fn get_range(range: impl RangeBounds<usize>, max_len: usize) -> (usize, usize) {
    let start = match range.start_bound() {
        std::ops::Bound::Included(&v) => v,
        std::ops::Bound::Excluded(&v) => v + 1,
        std::ops::Bound::Unbounded => 0,
    };
    let end = match range.end_bound() {
        std::ops::Bound::Included(&v) => v + 1,
        std::ops::Bound::Excluded(&v) => v,
        std::ops::Bound::Unbounded => max_len,
    };
    assert!(start <= end);
    assert!(end <= max_len);
    (start, end)
}

// impl<I: SliceIndex<[u8]>> Index<I> for AppendOnlyBytes {
//     type Output = I::Output;

//     #[inline]
//     fn index(&self, index: I) -> &Self::Output {
//         Index::index(self.raw(), index)
//     }
// }

impl Deref for AppendOnlyBytes {
    type Target = [u8];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.raw()
    }
}

unsafe impl Send for BytesSlice {}
unsafe impl Sync for BytesSlice {}

impl BytesSlice {
    #[inline(always)]
    fn bytes(&self) -> &[u8] {
        self.raw.slice(self.start..self.end)
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    #[inline]
    pub fn slice_clone(&self, range: impl std::ops::RangeBounds<usize>) -> Self {
        let (start, end) = get_range(range, self.end - self.start);
        Self {
            raw: self.raw.clone(),
            start: self.start + start,
            end: self.start + end,
        }
    }

    #[inline(always)]
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.raw, &other.raw)
    }

    #[inline(always)]
    pub fn can_merge(&self, other: &Self) -> bool {
        self.ptr_eq(other) && self.end == other.start
    }

    #[inline(always)]
    pub fn try_merge(&mut self, other: &Self) -> Result<(), MergeFailed> {
        if self.can_merge(other) {
            self.end = other.end;
            Ok(())
        } else {
            Err(MergeFailed)
        }
    }

    #[inline]
    pub fn slice_str(&self, range: impl RangeBounds<usize>) -> Result<&str, std::str::Utf8Error> {
        let (start, end) = get_range(range, self.len());
        std::str::from_utf8(&self.deref()[start..end])
    }
}

#[derive(Debug)]
pub struct MergeFailed;

impl Deref for BytesSlice {
    type Target = [u8];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.bytes()
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;
    #[test]
    fn test() {
        let mut a = AppendOnlyBytes::new();
        let mut count = 0;
        for _ in 0..100 {
            a.push(8);
            count += 1;
            assert_eq!(a.len(), count);
        }

        for _ in 0..100 {
            a.push_slice(&[1, 2]);
            count += 2;
            assert_eq!(a.len(), count);
        }
    }

    #[test]
    fn it_works() {
        let mut a = AppendOnlyBytes::new();
        a.push_str("123");
        assert_eq!(a.slice_str(0..1).unwrap(), "1");
        let b = a.slice(..);
        for _ in 0..10 {
            a.push_str("456");
            dbg!(a.slice_str(..).unwrap());
        }
        let c = a.slice(..);
        drop(a);
        dbg!(c.slice_str(..).unwrap());
        assert_eq!(c.len(), 33);
        assert_eq!(c.slice_str(..6).unwrap(), "123456");

        assert_eq!(b.deref(), "123".as_bytes());
    }

    #[test]
    fn threads() {
        let mut a = AppendOnlyBytes::new();
        a.push_str("123");
        assert_eq!(a.slice_str(0..1).unwrap(), "1");
        let b = a.slice(..);
        let t = thread::spawn(move || {
            for _ in 0..10 {
                a.push_str("456");
                dbg!(a.slice_str(..).unwrap());
            }
            let c = a.slice(..);
            drop(a);
            dbg!(c.slice_str(..).unwrap());
            assert_eq!(c.len(), 33);
            assert_eq!(c.slice_str(..6).unwrap(), "123456");
        });

        assert_eq!(b.deref(), "123".as_bytes());
        t.join();
    }
}
