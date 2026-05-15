//! Architecture-specific implementations.
//!
//! To allow for normal testing, we put ARM-specific logic
//! in a module.

use crate::{
    AttributeIndex, MemoryAttributes, Region, RegionRange,
    regs::{BaseAddress, LimitAddress, Type},
};
use arbitrary_int::*;
use cortex_m::peripheral::{CPUID, MPU};

/// The amount of regions that this implementation
/// supports.
pub const REGIONS: u8 = 8;

pub struct Mpu {
    mpu: MPU,
}

impl Mpu {
    fn read_type(&self) -> Type {
        Type::new_with_raw_value(self.mpu._type.read())
    }

    fn read_attribute_for(&self, num: AttributeIndex) -> u8 {
        let num = num.get();
        let index = (num / 4) as usize;
        let shift = (num % 4) * 8;
        let reg = self.mpu.mair[index].read();
        (reg >> shift) as u8
    }

    fn write_attribute_for(&self, num: AttributeIndex, attr: u8) {
        let num = num.get();
        let index = (num / 4) as usize;
        let shift = (num % 4) * 8;
        let mask = 0xFF << shift;

        unsafe { self.mpu.mair[index].modify(|w| w & !mask | ((attr as u32) << shift)) };
    }

    pub fn new(cpuid: &CPUID, mpu: MPU) -> Result<Self, ()> {
        let base = cpuid.base.read();
        let part_no = (base >> 4) & 0xFFF;

        // 0xD21 = Cortex-M33
        if part_no != 0xD21 {
            return Err(());
        }

        let me = Self { mpu };

        if me.regions() != 8 {
            return Err(());
        }

        Ok(me)
    }

    pub fn get_region(&self, num: u8) -> Option<Region> {
        assert!(num < REGIONS);

        let limit = LimitAddress::new_with_raw_value(self.mpu.rlar.read());

        if limit.enable() {
            let base = BaseAddress::new_with_raw_value(self.mpu.rbar.read());
            let start = u32::from(base.base()) << 5;
            let end = u32::from(limit.limit()) << 5;

            let region = Region {
                range: RegionRange::new_unchecked(start..=end),
                attribute_index: limit.attr_index().into(),
                shareability: base.shareability().unwrap(),
                access_permissions: base.access_permissions(),
                execute_never: base.execute_never(),
            };

            Some(region)
        } else {
            None
        }
    }

    pub fn get_attributes(&self, num: AttributeIndex) -> MemoryAttributes {
        let value = self.read_attribute_for(num);
        MemoryAttributes::decode(value)
    }

    pub fn set_attributes(&self, num: AttributeIndex, attributes: MemoryAttributes) {
        let value = attributes.encode();
        self.write_attribute_for(num, value);
    }

    pub fn set_region(&mut self, num: u8, region: Option<Region>) -> Result<(), ()> {
        assert!(num < REGIONS);

        unsafe { self.mpu.rnr.write(num as _) };

        if let Some(region) = region {
            // Check that the requested range does not overlap with
            // any other active regions.
            for other_region in (0..self.regions()).filter_map(|other_region| {
                (other_region != num)
                    .then(|| self.get_region(other_region))
                    .flatten()
            }) {
                let region = region.range.get();
                let other_region = other_region.range.get();

                // Manual implementation of currently-unstable `RangeInclusive::is_overlapping`
                if (region.start() <= other_region.end()) & (other_region.start() <= region.end()) {
                    return Err(());
                }
            }

            // No overlapping ranges, so we can set up the region.

            let start = *region.range.get().start() >> 5;
            let base = BaseAddress::builder()
                .with_base(u27::new(start))
                .with_shareability(region.shareability)
                .with_access_permissions(region.access_permissions)
                .with_execute_never(region.execute_never)
                .build();

            let end = *region.range.get().end() >> 5;
            let limit = LimitAddress::builder()
                .with_enable(true)
                .with_attr_index(u3::new(num))
                .with_limit(u27::new(end))
                .with_reserved(false)
                .build();

            unsafe { self.mpu.rbar.write(base.raw_value()) };
            unsafe { self.mpu.rlar.write(limit.raw_value()) };
        } else {
            let limit = LimitAddress::builder()
                .with_enable(false)
                .with_attr_index(Default::default())
                .with_limit(Default::default())
                .with_reserved(false)
                .build();

            unsafe { self.mpu.rlar.write(limit.raw_value()) };
        }

        Ok(())
    }

    /// Get the number of MPU regions supported by
    /// this MPU.
    pub fn regions(&self) -> u8 {
        self.read_type().dregion()
    }
}

// TODO: build a nice `Debug` and `defmt::Format` implementation
