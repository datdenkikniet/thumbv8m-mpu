//! Architecture-specific implementations.
//!
//! To allow for normal testing, we put ARM-specific logic
//! in a module.

use crate::{
    AttributeIndex, MemoryAttributes, Region, RegionConfig, RegionRange,
    regs::{BaseAddress, LimitAddress, Type},
};
use arbitrary_int::*;
use cortex_m::peripheral::MPU;

/// A token providing access to configure a specific
/// region.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, PartialEq)]
pub struct RegionToken(u8);

impl RegionToken {
    /// Get the raw index of this token.
    pub fn get(&self) -> u8 {
        self.0
    }
}

/// The provided region range overlaps with another
/// enabled region.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlappingRanges {
    /// The number of the region with which the provided range
    /// overlaps.
    pub region: u8,
}

/// The thumbv8m MPU.
pub struct Mpu {
    mpu: MPU,
    took_tokens: bool,
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

    /// Instantiate the MPU.
    pub const fn new(mpu: MPU) -> Self {
        Self {
            mpu,
            took_tokens: false,
        }
    }

    /// Get an array of all available region tokens.
    ///
    /// # Panics
    /// This function panics if `NUM_REGIONS` is not 8 or 16, or if
    /// `NUM_REGIONS` is greater than the amount of regions supported
    /// by the device's MPU, or if `tokens()` is called more than once.
    pub fn tokens<const NUM_REGIONS: usize>(&mut self) -> [RegionToken; NUM_REGIONS] {
        const { assert!(NUM_REGIONS == 8 || NUM_REGIONS == 16) };

        assert!(!self.took_tokens);
        self.took_tokens = true;

        let regions = self.regions();
        defmt::assert!(
            NUM_REGIONS as u8 <= regions,
            "MPU had {} regions, but we're trying to hand out {} tokens",
            regions,
            NUM_REGIONS,
        );

        let mut tokens = [const { RegionToken(0) }; _];

        let mut index = 0;
        while index < NUM_REGIONS {
            tokens[index] = RegionToken(index as u8);
            index += 1;
        }

        tokens
    }

    /// Get the configuration of the region found at `token`.
    pub fn get_region(&self, token: &RegionToken) -> Region {
        // SAFETY: the side-effect of writing a different set
        // of region registers is accounted for.
        unsafe { self.mpu.rnr.write(token.0 as _) };
        let limit = LimitAddress::new_with_raw_value(self.mpu.rlar.read());
        let base = BaseAddress::new_with_raw_value(self.mpu.rbar.read());
        let start = u32::from(base.base()) << 5;
        let end = u32::from(limit.limit()) << 5;

        let range = RegionRange::new_unchecked(start..=end);

        Region {
            range,
            config: RegionConfig {
                attribute_index: limit.attr_index().into(),
                shareability: base.shareability().unwrap(),
                access_permissions: base.access_permissions(),
                execute_never: base.execute_never(),
                enabled: limit.enable(),
            },
        }
    }

    /// Get the attributes specified at `index`.
    pub fn get_attributes(&self, index: AttributeIndex) -> MemoryAttributes {
        let value = self.read_attribute_for(index);
        MemoryAttributes::decode(value)
    }

    /// Set the attributes at `index` to `attributes`.
    ///
    /// This affect all regions whose `index` is set to `index`.
    pub fn set_attributes(&self, index: AttributeIndex, attributes: MemoryAttributes) {
        let value = attributes.encode();
        self.write_attribute_for(index, value);
    }

    /// Set the region at `token` to the configuration specified
    /// by `region`.
    pub fn set_region(
        &mut self,
        token: &mut RegionToken,
        region: Region,
    ) -> Result<(), OverlappingRanges> {
        let num = token.0;

        unsafe { self.mpu.rnr.write(num as _) };

        // Check that the requested range does not overlap with
        // any other regions.
        for other_region_num in 0..self.regions() {
            let other_region = if other_region_num != num {
                self.get_region(&RegionToken(other_region_num))
            } else {
                continue;
            };

            let region = region.range.get();
            let other_region = other_region.range.get();

            // Manual implementation of currently-unstable `RangeInclusive::is_overlapping`
            if (region.start() <= other_region.end()) & (other_region.start() <= region.end()) {
                return Err(OverlappingRanges {
                    region: other_region_num,
                });
            }
        }

        // No overlapping ranges, so we can set up the region.
        let start = *region.range.get().start() >> 5;
        let base = BaseAddress::builder()
            .with_base(u27::new(start))
            .with_shareability(region.config.shareability)
            .with_access_permissions(region.config.access_permissions)
            .with_execute_never(region.config.execute_never)
            .build();

        let end = *region.range.get().end() >> 5;
        let limit = LimitAddress::builder()
            .with_enable(region.config.enabled)
            .with_attr_index(region.config.attribute_index.into())
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
