//! High-level support for the Memory Protection Unit (MPU)
//! on ARM thumbv8-m based microcontrollers.
//!
//! The MPU supports up to 8 distinct memory attribute configurations,
//! and 8 or 16 region configurations.
#![warn(missing_docs)]
#![cfg_attr(not(test), no_std)]

pub(crate) mod regs;

pub mod alloc;
#[cfg(not(test))]
mod arch;
mod ll;
mod region_aligned;
#[cfg(test)]
mod test;

pub use crate::regs::Control;
pub use alloc::{AllocMpu, DeviceRegion, RegionGroup, RegionGroupError};
use arbitrary_int::u3;
use bitbybit::bitenum;
use core::ops::{Range, RangeInclusive};
pub use cortex_m::peripheral::MPU;
pub use ll::{LlMpu, OverlappingRanges};
pub use region_aligned::RegionAligned;

/// The default, low-level MPU.
pub type Mpu = LlMpu<cortex_m::peripheral::MPU>;

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
#[derive(Debug, Clone, Copy, PartialEq)]
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
#[derive(Debug, Clone, Copy, PartialEq)]
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
#[derive(Debug, Clone, Copy, PartialEq)]
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
#[derive(Debug, PartialEq)]
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

/// The reason that a range was not a valid region range.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RegionRangeError {
    /// The start address of the range does not have
    /// an alignment of [`RegionRange::REQUIRED_ALIGNMENT`].
    StartMisaligned,
    /// The end address of the range does not have the
    /// correct alignment.
    EndMisaligned,
    /// The end of the range is before the start of the range.
    EndBeforeStart,
}

/// The range of a region.
///
/// This is, under the hood, a `RangeInclusive<u32>`
/// for which `start % 32 == 0` and `end % 32 == 31`
/// hold.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, PartialEq)]
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
    /// `Ok()` if constructed using `new()`, or if it is used
    /// for a disabled region.
    const fn new_unchecked(range: RangeInclusive<u32>) -> Self {
        Self { range }
    }

    /// Create a new region from the provided address range.
    ///
    /// This function returns an error if `range.start` is not a multiple
    /// of 32, or if `range.end` is not a multiple of 32, or if `range.end < range.start`.
    pub const fn new(range: Range<u32>) -> Result<Self, RegionRangeError> {
        Self::new_inclusive(range.start..=range.end.saturating_sub(1))
    }

    /// Create a new region from the provided address range.
    ///
    /// This function returns an error if `range.start()` is not a multiple
    /// of 32, or if `range.end().wrapping_add(1)` is not a multiple of 32,
    /// or if `range.end() < range.start()`.
    pub const fn new_inclusive(range: RangeInclusive<u32>) -> Result<Self, RegionRangeError> {
        // If `range.end != u32::MAX`, this will not overflow.
        // If `range.end == u32::MAX` (a correct ending), this will wrap to `0`,
        // which is a multiple of 32.
        let range_end_aligned = range.end().wrapping_add(1).is_multiple_of(32);

        if !range.start().is_multiple_of(Self::REQUIRED_ALIGNMENT) {
            return Err(RegionRangeError::StartMisaligned);
        } else if !range_end_aligned {
            return Err(RegionRangeError::EndMisaligned);
        } else if *range.end() < *range.start() {
            return Err(RegionRangeError::EndBeforeStart);
        }

        Ok(Self { range })
    }

    /// Get the raw range underpinning this region range.
    pub fn get(&self) -> RangeInclusive<u32> {
        self.range.clone()
    }

    /// Check whether this region overlaps with another.
    pub fn overlaps(&self, other: &Self) -> bool {
        // Manual implementation of currently-unstable `RangeInclusive::is_overlapping`
        (self.range.start() <= other.range.end()) & (other.range.start() <= self.range.end())
    }
}

/// The index of a specific [`MemoryAttributes`] configuration.
///
/// This index refers to an offset in one of the Memory Attribute Indirection
/// registers in the MPU. This offset is then used to configure the Memory
/// Attributes for all enabled memory regions whose `index` field is set to
/// the corresponding [`AttributeIndex`].
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
    pub const fn get(&self) -> u8 {
        self.0.value()
    }
}

impl From<arbitrary_int::u3> for AttributeIndex {
    fn from(value: arbitrary_int::u3) -> Self {
        Self(value)
    }
}

impl From<AttributeIndex> for arbitrary_int::u3 {
    fn from(value: AttributeIndex) -> Self {
        value.0
    }
}

/// An MPU region configuration.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone)]
pub struct RegionConfig {
    /// The range of addresses in this region.
    pub range: RegionRange,
    /// The ways in which a region is shared.
    ///
    /// For a config whose [`AttributeIndex`] configuration points
    /// to a [`MemoryAttributes::Device`], this value is ignored.
    /// Regions with that attribute are always fully shared.
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

/// An MPU-configurable region.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Default)]
pub enum Region {
    #[default]
    /// The region is disabled.
    Disabled,
    /// The region is enabled, and has the specified
    /// configuration.
    Enabled(RegionConfig),
}

impl Region {
    /// Get whether this region is enabled.
    pub const fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled(_))
    }
}

/// A token providing access to configure a specific
/// region.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, PartialEq)]
pub struct RegionToken(pub(crate) u8);

impl RegionToken {
    /// Get the raw index of this token.
    pub fn get(&self) -> u8 {
        self.0
    }
}

/// An implementation of the ARMv8-M MPU.
pub trait MpuImpl {
    /// The amount of regions supported by this MPU.
    fn regions(&self) -> u8;

    /// Read the memory attributes configuration for the item at
    /// index `index`.
    fn attributes(&self, index: AttributeIndex) -> MemoryAttributes;
    /// Set the memory attributes configuration for the item at
    /// index `index` to `attrs`.
    fn set_attributes(&mut self, index: AttributeIndex, attrs: MemoryAttributes);

    /// Read the control register.
    fn control(&self) -> Control;
    /// Set the control register to `control`.
    fn set_control(&mut self, control: Control);

    /// Get the configuration of the region associated with `token`.
    fn region(&self, token: &RegionToken) -> Region;
    /// Set the configuration of the region associated with `token` to
    /// `region`.
    fn set_region(&mut self, token: &mut RegionToken, region: &Region);
}

#[cfg(feature = "defmt")]
fn mpu_defmt(mpu: &impl MpuImpl, fmt: defmt::Formatter) {
    use arbitrary_int::traits::Integer;

    let ctrl = mpu.control();

    defmt::write!(fmt, "Mpu {{\n");
    defmt::write!(fmt, "  enabled: {},\n", ctrl.enable());
    defmt::write!(fmt, "  privdefena: {},\n", ctrl.privdefena());
    defmt::write!(fmt, "  hfnmiena: {},\n", ctrl.hfnmiena());
    defmt::write!(fmt, "  Attributes(8): [\n");
    for idx_num in 0..u3::MAX.value() {
        let idx = AttributeIndex(unsafe { u3::new_unchecked(idx_num) });
        let attr = mpu.attributes(idx);
        defmt::write!(fmt, "    {},\n", attr);
    }
    defmt::write!(fmt, "  ],\n");

    let regions = mpu.regions();
    defmt::write!(fmt, "  Regions({}): [\n", regions);
    for region_idx in 0..regions {
        let region = RegionToken(region_idx);
        let region = mpu.region(&region);
        defmt::write!(fmt, "    {},\n", region);
    }
    defmt::write!(fmt, "  ]\n}}");
}
