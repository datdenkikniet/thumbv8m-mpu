#![cfg_attr(not(test), no_std)]

pub(crate) mod regs;

#[cfg(not(test))]
mod arch;
use arbitrary_int::u3;
#[cfg(not(test))]
pub use arch::*;

#[cfg(test)]
mod test;

mod region_aligned;
pub use region_aligned::RegionAligned;

use bitbybit::bitenum;
use core::ops::RangeInclusive;

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

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug)]
#[bitenum(u2, exhaustive = true)]
pub enum AccessPermissions {
    PrivilegedReadWrite = 0b00,
    AnyReadWrite = 0b01,
    PrivilegedReadOnly = 0b10,
    AnyReadOnly = 0b11,
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy)]
pub enum MemoryAttributes {
    Device(DeviceMemoryAttributes),
    Normal {
        outer: NormalMemoryAttributes,
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

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy)]
pub enum NormalMemoryAttributes {
    WriteThroughTransient(TransientAllocations),
    NonCacheable,
    WriteBackTransient(TransientAllocations),
    WriteThroughNonTransient {
        allocate_reads: bool,
        allocate_writes: bool,
    },
    WriteBackNonTransient {
        allocate_reads: bool,
        allocate_writes: bool,
    },
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy)]
pub enum TransientAllocations {
    AllocateWrites,
    AllocateReads,
    /// Allocate both reads and writes.
    AllocateBoth,
}

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

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone)]
pub struct RegionRange {
    /// The range of addresses in this region.
    range: RangeInclusive<u32>,
}

impl RegionRange {
    /// Addresses must be 32-byte aligned.
    pub const REQUIRED_ALIGNMENT: u32 = 32;

    /// A bitmask that can be used to determine if
    /// an address is aligned, or to create an aligned address
    /// from a potentially-unaligned address.
    pub const ALIGNMENT_MASK: u32 = 0xFFFF_FFFF << (Self::REQUIRED_ALIGNMENT / 8);

    /// NOTE: not `unsafe`, as invalid configurations
    /// will raise a fault. However, don't use this function
    /// with values not directly read from the MPU!.
    fn new_unchecked(range: RangeInclusive<u32>) -> Self {
        Self { range }
    }

    /// Create a new region from the provided raw data.
    ///
    /// `address` is the start address of the region, and
    /// `size` is the size of the region in bytes.
    ///
    /// This function returns an error if `address` is not a multiple
    /// of 32, or if `size + 1` is not a multiple of 32.
    pub const fn new_raw(address: u32, size: u32) -> Result<Self, ()> {
        let range = address..=address + size;

        if range.start().is_multiple_of(32) && *range.end() % 32 == 31 {
            Ok(Self { range })
        } else {
            Err(())
        }
    }

    pub fn get(&self) -> RangeInclusive<u32> {
        self.range.clone()
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy)]
pub struct AttributeIndex(u3);

impl AttributeIndex {
    pub const fn new(value: u8) -> Option<Self> {
        if let Ok(v) = u3::try_new(value) {
            Some(Self(v))
        } else {
            None
        }
    }

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

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone)]
pub struct Region {
    pub enabled: bool,
    /// The range of addresses in this region.
    pub range: RegionRange,
    /// This value is ignored for address ranges
    /// that have any of the [`MemoryAttributes::Device`]
    /// attributes. Regions that have [`MemoryAttributes::Device`]
    /// are always considered to be fully shared.
    pub shareability: Shareability,
    pub attribute_index: AttributeIndex,
    pub access_permissions: AccessPermissions,
    pub execute_never: bool,
}
