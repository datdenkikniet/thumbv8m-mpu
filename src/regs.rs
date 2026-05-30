#![cfg_attr(test, allow(unused, reason = "allowed when compiling for host target"))]
use arbitrary_int::*;
use bitbybit::bitfield;

use crate::{
    AccessPermissions, DeviceMemoryAttributes, MemoryAttributes, NormalMemoryAttributes,
    Shareability, TransientAllocations,
};

#[bitfield(
    u32,
    defmt_bitfields(feature = "defmt"),
    debug,
    forbid_overlaps,
    default = 0
)]
pub struct Control {
    #[bit(2, rw)]
    privdefena: bool,
    #[bit(1, rw)]
    hfnmiena: bool,
    #[bit(0, rw)]
    enable: bool,
}

#[bitfield(u32, defmt_bitfields(feature = "defmt"), debug, forbid_overlaps)]
pub struct Type {
    #[bits(8..=15, r)]
    dregion: u8,
    #[bit(0, r)]
    separate: bool,
}

#[bitfield(u32, defmt_bitfields(feature = "defmt"), debug, forbid_overlaps)]
pub struct BaseAddress {
    #[bits(5..=31, rw)]
    base: u27,
    #[bits(3..=4, rw)]
    shareability: Option<Shareability>,
    #[bits(1..=2, rw)]
    access_permissions: AccessPermissions,
    #[bit(0, rw)]
    execute_never: bool,
}

#[bitfield(u32, defmt_bitfields(feature = "defmt"), debug, forbid_overlaps)]
pub struct LimitAddress {
    #[bits(5..=31, rw)]
    limit: u27,
    #[bit(4, rw)]
    reserved: bool,
    #[bits(1..=3, rw)]
    attr_index: u3,
    #[bit(0, rw)]
    enable: bool,
}

#[bitfield(u8, defmt_bitfields(feature = "defmt"), debug, forbid_overlaps)]
pub struct Attribute {
    #[bits(4..=7)]
    outer: u4,
    #[bits(0..=3)]
    inner: u4,
}

impl MemoryAttributes {
    pub(crate) const fn encode(&self) -> u8 {
        let (upper, lower) = match self {
            MemoryAttributes::Device(attr) => (0b0000, attr.raw_value().value()),
            MemoryAttributes::Normal { outer, inner, .. } => (outer.encode(), inner.encode()),
        };

        (upper << 4) | lower
    }

    pub(crate) const fn decode(value: u8) -> Self {
        let (upper, lower) = ((value >> 4) & 0xF, value & 0xF);

        match (upper, lower) {
            (0b0000, device) => Self::Device(DeviceMemoryAttributes::new_with_raw_value(
                u2::from_u8(device),
            )),
            (outer, inner) => Self::Normal {
                outer: NormalMemoryAttributes::decode(outer),
                inner: NormalMemoryAttributes::decode(inner),
            },
        }
    }
}

impl NormalMemoryAttributes {
    pub(crate) const fn encode(&self) -> u8 {
        let (lower, upper) = match *self {
            NormalMemoryAttributes::WriteThroughTransient(transient) => (0b00, transient.encode()),
            NormalMemoryAttributes::NonCacheable => (0b01, 0b00),
            NormalMemoryAttributes::WriteBackTransient(transient) => (0b01, transient.encode()),
            NormalMemoryAttributes::WriteThroughNonTransient {
                allocate_reads,
                allocate_writes,
            } => (0b10, ((allocate_reads as u8) << 1) | allocate_writes as u8),
            NormalMemoryAttributes::WriteBackNonTransient {
                allocate_reads,
                allocate_writes,
            } => (0b11, ((allocate_reads as u8) << 1) | allocate_writes as u8),
        };

        (lower << 2) | upper
    }

    pub(crate) const fn decode(value: u8) -> Self {
        let (upper, lower) = ((value >> 2) & 0b11, value & 0b11);

        match (upper, lower) {
            (0b00, allocations) => {
                Self::WriteThroughTransient(TransientAllocations::decode(allocations))
            }
            (0b01, 0b00) => Self::NonCacheable,
            (0b01, allocations) => {
                Self::WriteBackTransient(TransientAllocations::decode(allocations))
            }
            (0b10, allocations) => Self::WriteThroughNonTransient {
                allocate_reads: (allocations & 0b10) == 0b10,
                allocate_writes: (allocations & 0b01) == 0b01,
            },
            (0b11, allocations) => Self::WriteBackNonTransient {
                allocate_reads: (allocations & 0b10) == 0b10,
                allocate_writes: (allocations & 0b01) == 0b01,
            },
            _ => unreachable!(),
        }
    }
}

impl TransientAllocations {
    pub(crate) const fn encode(&self) -> u8 {
        match self {
            TransientAllocations::AllocateWrites => 0b01,
            TransientAllocations::AllocateReads => 0b10,
            TransientAllocations::AllocateBoth => 0b11,
        }
    }

    pub(crate) const fn decode(value: u8) -> Self {
        match value {
            0b01 => TransientAllocations::AllocateWrites,
            0b10 => TransientAllocations::AllocateReads,
            0b11 => TransientAllocations::AllocateBoth,
            _ => panic!("Unknown/invalid transient allocations value"),
        }
    }
}
