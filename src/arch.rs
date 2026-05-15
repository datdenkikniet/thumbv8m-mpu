//! Architecture-specific implementations.
//!
//! To allow for normal testing, we put ARM-specific logic
//! in a module.

use crate::{
    AttributeIndex, MemoryAttributes, Region, RegionRange, Shareability,
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

    pub fn get_region(&self, num: u8) -> Region {
        assert!(num < REGIONS);

        let limit = LimitAddress::new_with_raw_value(self.mpu.rlar.read());
        let base = BaseAddress::new_with_raw_value(self.mpu.rbar.read());
        let start = u32::from(base.base()) << 5;
        let end = u32::from(limit.limit()) << 5;
        let attributes =
            self.get_attributes(limit.attr_index().into(), base.shareability().unwrap());

        Region {
            range: RegionRange::new_unchecked(start..=end),
            attributes,
            access_permissions: base.access_permissions(),
            execute_never: base.execute_never(),
            enabled: limit.enable(),
        }
    }

    pub fn get_attributes(
        &self,
        num: AttributeIndex,
        shareability: Shareability,
    ) -> MemoryAttributes {
        let value = self.read_attribute_for(num);
        MemoryAttributes::decode(value, shareability)
    }

    pub fn set_attributes(&self, num: AttributeIndex, attributes: MemoryAttributes) {
        let value = attributes.encode();
        self.write_attribute_for(num, value);
    }

    pub fn set_region(&mut self, num: u8, region: Region) -> Result<(), ()> {
        assert!(num < REGIONS);

        unsafe { self.mpu.rnr.write(num as _) };

        // Check that the requested range does not overlap with
        // any other regions.
        for other_region in (0..self.regions())
            .filter_map(|other_region| (other_region != num).then(|| self.get_region(other_region)))
        {
            let region = region.range.get();
            let other_region = other_region.range.get();

            // Manual implementation of currently-unstable `RangeInclusive::is_overlapping`
            if (region.start() <= other_region.end()) & (other_region.start() <= region.end()) {
                return Err(());
            }
        }

        let attribute_index = u3::new(num).into();
        self.write_attribute_for(attribute_index, region.attributes.encode());

        // The shareability bits are ignored for
        // any type of device memory, so we can
        // set them to some default.
        let shareability = match region.attributes {
            MemoryAttributes::Device(_) => Shareability::OuterShareable,
            MemoryAttributes::Normal { shareability, .. } => shareability,
        };

        // No overlapping ranges, so we can set up the region.
        let start = *region.range.get().start() >> 5;
        let base = BaseAddress::builder()
            .with_base(u27::new(start))
            .with_shareability(shareability)
            .with_access_permissions(region.access_permissions)
            .with_execute_never(region.execute_never)
            .build();

        let end = *region.range.get().end() >> 5;
        let limit = LimitAddress::builder()
            .with_enable(region.enabled)
            .with_attr_index(attribute_index.into())
            .with_limit(u27::new(end))
            .with_reserved(false)
            .build();

        unsafe { self.mpu.rbar.write(base.raw_value()) };
        unsafe { self.mpu.rlar.write(limit.raw_value()) };

        cortex_m::asm::dsb();
        cortex_m::asm::isb();

        Ok(())
    }

    /// Enable the MPU
    ///
    /// `privileged_sw_may_access_default_map` controls whether
    /// accesses to memory not belonging to any enabled MPU region
    /// by privileged software is allowed according to the default
    /// map (`true`), or if it will cause a fault (`false`). The
    /// bit corresponds to the `PRIVDEFENA` bit in the `MPU_CTRL`
    /// register in ARM documentation.
    ///
    /// Setting the `enable_mpu_in_nmi_and_hardfault` will enable the MPU
    /// in the Non-Maskable Interrupt and HardFault handlers. It corresponds
    /// to the `HFNMIENA` bit in the `MPU_CTRL` register in ARM documentation.
    pub fn enable(
        &mut self,
        privileged_sw_may_access_default_map: bool,
        enable_mpu_in_nmi_and_hardfault: bool,
    ) {
        let privdefena = (privileged_sw_may_access_default_map as u32) << 2;
        let hfnmiena = (enable_mpu_in_nmi_and_hardfault as u32) << 1;
        unsafe { self.mpu.ctrl.write(privdefena | hfnmiena | 1) };

        cortex_m::asm::dsb();
        cortex_m::asm::isb();
    }

    /// Disable the MPU
    pub fn disable(&mut self) {
        unsafe { self.mpu.ctrl.write(0) };
    }

    /// Get the number of MPU regions supported by
    /// this MPU.
    pub fn regions(&self) -> u8 {
        self.read_type().dregion()
    }
}

// TODO: build a nice `Debug` and `defmt::Format` implementation
