//! High-level support for the Memory Protection Unit
//! on ARM thumbv8-m based microcontrollers.
#![deny(missing_docs)]
#![cfg_attr(not(test), no_std)]

pub(crate) mod regs;

#[cfg(not(test))]
mod arch;
#[cfg(not(test))]
pub use arch::*;

#[cfg(test)]
mod test;

mod region_aligned;

pub use arbitrary_int::u3;
use bitbybit::bitenum;
use core::ops::{Range, RangeInclusive};
pub use region_aligned::RegionAligned;

/// The shareability of a memory region.
///
/// This enum does not have a fully-shared variant:
/// to configure a region to be fully shared, configure
/// its [`MemoryAttributes`] to one of the
/// [`MemoryAttributes::Device`] variants.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug)]
#[bitenum(u2, exhaustive = false)]
pub enum Shareability {
    /// Non-shareable regions are only accessed by
    /// the core itself, and do not require any sort
    /// of bus-master synchronization.
    NonShareable = 0b00,
    /// Inner shareable regions are accessed by the
    /// core itself, and other bus-masters
    /// that are part of the same inner domain.
    InnerShareable = 0b11,
    /// Outer shareable regions are accessed by the
    /// core itself, other bus-masters that are
    /// part of the same inner domain, and other
    /// bus-masters that are part of other inner
    /// domains.
    OuterShareable = 0b10,
}

/// Access permissions for a region.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug)]
#[bitenum(u2, exhaustive = true)]
pub enum AccessPermissions {
    /// Only privileged code may perform read and
    /// write accesses to the region.
    PrivilegedReadWrite = 0b00,
    /// Any code may perform read and write accesses
    /// to the region.
    AnyReadWrite = 0b01,
    /// Only privileged code may perform read accesses
    /// to the region.
    PrivilegedReadOnly = 0b10,
    /// Any code may perform read accesses to the region.
    AnyReadOnly = 0b11,
}

/// The memory attributes of a region.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy)]
pub enum MemoryAttributes {
    /// The memory in this region is a device memory.
    Device(DeviceMemoryAttributes),
    /// The memory in this region is a normal memory.
    Normal {
        /// The attributes of the outer (inter-bus-master
        /// caching) memory.
        outer: NormalMemoryAttributes,
        /// The attributes of the outer (single-core
        /// caching) memory.
        inner: NormalMemoryAttributes,
    },
}

impl MemoryAttributes {
    /// Memory attributes describing fully non-cacheable memory,
    /// i.e. memory that is shared with a DMA peripheral.
    pub const fn non_cacheable() -> Self {
        Self::Device(DeviceMemoryAttributes::None)
    }
}

/// Attributes for a normal region.
///
/// # Allocating
/// Allocating means that a certain operation (read or write)
/// to the region will cause a cache-line to be allocated
/// for the addresses that this operation is performed on.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy)]
pub enum NormalMemoryAttributes {
    /// Transient (i.e. little-used) write-through
    /// memory.
    WriteThroughTransient(TransientAllocations),
    /// Non-cacheable normal memory.
    NonCacheable,
    /// Transient (i.e. little-used) write-back
    /// memory.
    WriteBackTransient(TransientAllocations),
    /// Non-transient (i.e. often-used) write-through
    /// memory.
    WriteThroughNonTransient {
        /// Whether reads allocate cache lines.
        allocate_reads: bool,
        /// Whether writes allocate cache lines.
        allocate_writes: bool,
    },
    /// Non-transient (i.e. often-used) write-back
    /// memory.
    WriteBackNonTransient {
        /// Whether reads allocate cache lines.
        allocate_reads: bool,
        /// Whether writes allocate cache lines.
        allocate_writes: bool,
    },
}

/// Cache allocation behaviour for transient memory
/// attributes.
///
/// For more information on what allocations mean, see
/// [NormalMemoryAttributes](./enum.NormalMemoryAttributes.html#allocating).
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy)]
pub enum TransientAllocations {
    /// Writes allocate cache lines.
    AllocateWrites,
    /// Reads allocate cache lines.
    AllocateReads,
    /// Both reads and writes allocate cache lines.
    AllocateBoth,
}

/// Memory attributes for regions of device memory.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug)]
#[bitenum(u2, exhaustive = true)]
pub enum DeviceMemoryAttributes {
    /// The device memory has the Non Gathering, Non Reordering,
    /// and Non Early Write Acknowledgement attributes.
    None = 0b00,
    /// The dvice memory has the NonGathering, Non Reordering,
    /// and Early Write Acknowledgement Attributes.
    NonGatheringNonReordering = 0b01,
    /// The device memory has the Non Gathering, Reordering and
    /// Early Write Acknowledgement attributes.
    NonGathering = 0b10,
    /// The device memory has the Gathering, Reordering, and
    /// Early Write Acknowledgement attributes.
    All = 0b11,
}

/// The range of a region.
///
/// This is, under the hood, a `RangeInclusive<u32>`
/// for which `start % 32 == 0` and `end % 32 == 31`
/// hold.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone)]
pub struct RegionRange {
    /// The range of addresses in this region.
    range: RangeInclusive<u32>,
}

impl RegionRange {
    /// The required alignment for an address.
    pub const REQUIRED_ALIGNMENT: u32 = 32;

    /// A bitmask that can be used to determine if
    /// an address is aligned, or to create an aligned address
    /// from a potentially-unaligned address.
    pub const ALIGNMENT_MASK: u32 = 0xFFFF_FFFF << (Self::REQUIRED_ALIGNMENT / 8);

    /// Only use this function if the provided range would return
    /// `Ok()` if constructed using `new()`
    fn new_unchecked(range: RangeInclusive<u32>) -> Self {
        Self { range }
    }

    /// Create a new region from the provided address range.
    ///
    /// This function returns an error if `range.start` is not a multiple
    /// of 32, or if `range.end` is not a multiple of 32.
    pub const fn new(range: Range<u32>) -> Result<Self, ()> {
        Self::new_inclusive(range.start..=range.end.saturating_sub(1))
    }

    /// Create a new region from the provided address range.
    ///
    /// This function returns an error if `range.start()` is not a multiple
    /// of 32, or if `range.end().wrapping_add(1)` is not a multiple of 32.
    pub const fn new_inclusive(range: RangeInclusive<u32>) -> Result<Self, ()> {
        // If `range.end != u32::MAX`, this will not overflow.
        // If `range.end == u32::MAX` (a correct ending), this will wrap to `0`,
        // which is a multiple of 32.
        let range_end_aligned = range.end().wrapping_add(1).is_multiple_of(32);

        if !range.start().is_multiple_of(32) || !range_end_aligned {
            return Err(());
        }

        Ok(Self { range })
    }

    /// Get the raw range underpinning this region range.
    pub fn get(&self) -> RangeInclusive<u32> {
        self.range.clone()
    }
}

/// The index of a specific [`MemoryAttributes`] configuration.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy)]
pub struct AttributeIndex(u3);

impl AttributeIndex {
    /// Create a new [`AttributeIndex`] from the provided
    /// `u8`. This will return `None` if `value > 7`.
    pub const fn new(value: u8) -> Option<Self> {
        if let Ok(v) = u3::try_new(value) {
            Some(Self(v))
        } else {
            None
        }
    }

    /// Get the value of this [`AttributeIndex`].
    pub fn get(&self) -> u8 {
        self.0.value()
    }
}

impl From<arbitrary_int::u3> for AttributeIndex {
    fn from(value: arbitrary_int::u3) -> Self {
        Self(value)
    }
}

impl Into<arbitrary_int::u3> for AttributeIndex {
    fn into(self) -> arbitrary_int::u3 {
        self.0
    }
}

/// An MPU region configuration.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy)]
pub struct RegionConfig {
    /// Whether this region is enabled.
    pub enabled: bool,
    /// This value is ignored for address ranges
    /// that have any of the [`MemoryAttributes::Device`]
    /// attributes. Regions that have [`MemoryAttributes::Device`]
    /// are always considered to be fully shared.
    pub shareability: Shareability,
    /// The index of the [`MemoryAttributes`] that should be
    /// assigned to this region.
    pub attribute_index: AttributeIndex,
    /// The access permissions for this region.
    pub access_permissions: AccessPermissions,
    /// Whether it should be possible to execute memory from
    /// this region, provided that is readable.
    pub execute_never: bool,
}

impl Default for RegionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            shareability: Shareability::OuterShareable,
            attribute_index: const { AttributeIndex::new(0).unwrap() },
            access_permissions: AccessPermissions::AnyReadWrite,
            execute_never: false,
        }
    }
}

/// An MPU-configurable region.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone)]
pub struct Region {
    /// The configuration of this Region.
    pub config: RegionConfig,
    /// The range of addresses in this region.
    pub range: RegionRange,
}
