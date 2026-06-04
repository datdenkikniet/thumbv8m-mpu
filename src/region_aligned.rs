use crate::RegionRange;
use core::num::NonZeroU32;

/// A region-aligned value.
///
/// `PADDING` should be an value such that
/// `core::mem::size_of::<T>() + PADDING % 32 == 0`.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy)]
#[repr(align(32))]
pub struct RegionAligned<T> {
    inner: T,
}

impl<T> RegionAligned<T> {
    const SIZE: NonZeroU32 = const {
        let size = core::mem::size_of::<Self>();
        if size > u32::MAX as usize {
            panic!("Cannot construct a `RegionAligned` whose size is bigger than u32::MAX");
        };

        let size = size as u32;

        assert!(
            size.is_multiple_of(RegionRange::REQUIRED_ALIGNMENT as _),
            "Being able to construct a `RegionAligned` whose size not a multiple of Region::REQUIRED_ALIGNMENT is a bug. Please report it."
        );

        let Some(size) = NonZeroU32::new(size) else {
            panic!("A `RegionAligned` ZST is illogical and cannot be used correctly.")
        };

        size
    };

    /// Create a new [`RegionAligned`] with the provided inner
    /// value.
    pub const fn new(value: T) -> Self {
        let _assert_size = Self::SIZE;
        Self { inner: value }
    }

    /// Get the region range that this [`RegionAligned`] value
    /// occupies.
    pub fn as_range(&self) -> RegionRange {
        let start_address = self as *const _ as usize as u32;
        let last_address = start_address + Self::SIZE.get() - 1;
        RegionRange::new_unchecked(start_address..=last_address)
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

impl<T: Default> Default for RegionAligned<T> {
    fn default() -> Self {
        RegionAligned {
            inner: T::default(),
        }
    }
}

impl<T> From<&RegionAligned<T>> for RegionRange {
    fn from(val: &RegionAligned<T>) -> Self {
        val.as_range()
    }
}

impl<T> From<&mut RegionAligned<T>> for RegionRange {
    fn from(val: &mut RegionAligned<T>) -> Self {
        val.as_range()
    }
}

impl<T> AsRef<T> for RegionAligned<T> {
    fn as_ref(&self) -> &T {
        &self.inner
    }
}

impl<T> AsMut<T> for RegionAligned<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}
