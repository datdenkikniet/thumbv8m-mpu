use crate::RegionRange;
use core::num::NonZeroU32;

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
        let start_address = self as *const _ as usize as u32;
        if let Some(non_zero_size) = NonZeroU32::new(core::mem::size_of::<Self>() as u32) {
            let last_address = start_address + non_zero_size.get() - 1;
            RegionRange::new_unchecked(start_address..=last_address)
        } else {
            RegionRange::new_unchecked(start_address..=start_address)
        }
    }

    /// Take the inner value out of this aligned struct.
    ///
    /// # Memory region
    /// This moves the value out of its original location. It will
    /// therefore no longer be contained in the memory range that was once
    /// returned by its [`RegionAligned::as_range`], and will no longer have
    /// the memory attributes that were configured for that range.
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

impl<T, const PADDING: usize> From<&RegionAligned<T, PADDING>> for RegionRange {
    fn from(val: &RegionAligned<T, PADDING>) -> Self {
        val.as_range()
    }
}

impl<T, const PADDING: usize> From<&mut RegionAligned<T, PADDING>> for RegionRange {
    fn from(val: &mut RegionAligned<T, PADDING>) -> Self {
        val.as_range()
    }
}

impl<T, const PADDING: usize> AsRef<T> for RegionAligned<T, PADDING> {
    fn as_ref(&self) -> &T {
        &self.inner
    }
}

impl<T, const PADDING: usize> AsMut<T> for RegionAligned<T, PADDING> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}
