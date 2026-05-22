use core::ops::{Deref, DerefMut};

use crate::RegionRange;

/// A region-aligned value.
///
/// `PADDING` should be an value such that
/// `core::mem::size_of::<T>() + PADDING % 32 == 0`.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy)]
#[repr(C, align(32))]
pub struct RegionAligned<T, const PADDING: usize> {
    inner: T,
    _padding: [u8; PADDING],
}

impl<T, const PADDING: usize> RegionAligned<T, PADDING> {
    /// Create a new [`RegionAligned`] with the provided inner
    /// value.
    pub const fn new(value: T) -> Self {
        const {
            assert!(
                core::mem::size_of::<Self>().is_multiple_of(RegionRange::REQUIRED_ALIGNMENT as _),
                "Tried to construct a `RegionAligned` whose size not a multiple of Region::REQUIRED_ALIGNMENT bytes. Adjust its padding to fix the problem"
            )
        }

        Self {
            inner: value,
            _padding: [0u8; _],
        }
    }

    /// Get the region range that this [`RegionAligned`] value
    /// occupies.
    pub fn as_range(&self) -> RegionRange {
        let start = self as *const _ as usize as u32;
        let end = start + core::mem::size_of::<RegionAligned<T, PADDING>>() as u32 - 1;
        RegionRange::new_unchecked(start..=end)
    }

    /// Take the inner value out of this aligned struct.
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T: Default, const PADDING: usize> Default for RegionAligned<T, PADDING> {
    fn default() -> Self {
        RegionAligned {
            inner: T::default(),
            _padding: [0u8; _],
        }
    }
}

impl<T, const PADDING: usize> Into<RegionRange> for &RegionAligned<T, PADDING> {
    fn into(self) -> RegionRange {
        self.as_range()
    }
}

impl<T, const PADDING: usize> Into<RegionRange> for &mut RegionAligned<T, PADDING> {
    fn into(self) -> RegionRange {
        self.as_range()
    }
}

impl<T, const PADDING: usize> Deref for RegionAligned<T, PADDING> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T, const PADDING: usize> DerefMut for RegionAligned<T, PADDING> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
