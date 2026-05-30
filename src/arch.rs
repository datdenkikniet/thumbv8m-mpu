//! Architecture-specific implementations.
//!
//! To allow for normal testing, we put ARM-specific logic
//! in a module.

use crate::{
    AttributeIndex, MemoryAttributes, Region, RegionConfig, RegionRange,
    regs::{BaseAddress, Control, LimitAddress, Type},
};
use arbitrary_int::u27;
use cortex_m::peripheral::MPU;

#[cfg(feature = "defmt")]
use {
    arbitrary_int::{traits::Integer, u3},
    defmt::assert,
};

#[cfg(not(feature = "defmt"))]
use core::assert;

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
#[derive(Debug, Clone)]
pub struct OverlappingRanges {
    /// The number of the region with which the provided range
    /// overlaps.
    pub region: u8,
    /// The configuration of the overlapping region.
    pub config: RegionConfig,
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

    fn read_ctrl(&self) -> Control {
        Control::new_with_raw_value(self.mpu.ctrl.read())
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

    /// Set the region at `token` to the configuration specified
    /// by `region`.
    fn write_region(&mut self, token: &mut RegionToken, enabled: bool, region: &RegionConfig) {
        let num = token.0;

        unsafe { self.mpu.rnr.write(num as _) };

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
            .with_enable(enabled)
            .with_attr_index(region.attribute_index.into())
            .with_limit(u27::new(end))
            .with_reserved(false)
            .build();

        unsafe { self.mpu.rbar.write(base.raw_value()) };
        unsafe { self.mpu.rlar.write(limit.raw_value()) };

        cortex_m::asm::dsb();
        cortex_m::asm::isb();
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
        const { core::assert!(NUM_REGIONS == 8 || NUM_REGIONS == 16) };

        assert!(!self.took_tokens);
        self.took_tokens = true;

        let regions = self.regions();
        assert!(
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

        if limit.enable() {
            Region::Enabled(RegionConfig {
                range,
                attribute_index: limit.attr_index().into(),
                shareability: base.shareability().unwrap(),
                access_permissions: base.access_permissions(),
                execute_never: base.execute_never(),
            })
        } else {
            Region::Disabled
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

    /// Set the configuration of `token` to `region`.
    ///
    /// [`Err(OverlappingRanges)`](OverlappingRanges) is returned if `region` is a
    /// [`Region::Enabled`], and its `range` overlaps with at least one other
    /// enabled region.
    ///
    /// If `Err` is returned, the configuration of `token` is unchanged from the
    /// configuration it had before `set_region` was called.
    pub fn set_region(
        &mut self,
        token: &mut RegionToken,
        region: &Region,
    ) -> Result<(), OverlappingRanges> {
        let num = token.0;

        if let Region::Enabled(region) = region {
            // Access to overlapping, enabled regions causes the CPU to generate
            // a fault. Check that none of them are overlapping.
            for other_region_num in 0..self.regions() {
                let other_region = if other_region_num != num {
                    self.get_region(&RegionToken(other_region_num))
                } else {
                    continue;
                };

                let Region::Enabled(other_region_config) = other_region else {
                    continue;
                };

                let region = region.range.get();
                let other_region = other_region_config.range.get();

                // Manual implementation of currently-unstable `RangeInclusive::is_overlapping`
                if (region.start() <= other_region.end()) & (other_region.start() <= region.end()) {
                    return Err(OverlappingRanges {
                        region: other_region_num,
                        config: other_region_config,
                    });
                }
            }

            self.write_region(token, true, region);
        } else {
            const DEFAULT_CONFIG: RegionConfig = RegionConfig::disabled();
            self.write_region(token, false, &DEFAULT_CONFIG);
        }

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
        let ctrl = Control::builder()
            .with_enable(true)
            .with_hfnmiena(enable_mpu_in_nmi_and_hardfault)
            .with_privdefena(privileged_sw_may_access_default_map)
            .build();

        unsafe { self.mpu.ctrl.write(ctrl.raw_value()) };

        cortex_m::asm::dsb();
        cortex_m::asm::isb();
    }

    /// Disable the MPU
    pub fn disable(&mut self) {
        unsafe { self.mpu.ctrl.write(Control::default().raw_value()) };
    }

    /// Whether the MPU is enabled.
    pub fn enabled(&self) -> bool {
        self.read_ctrl().enable()
    }

    /// Whether the PRIVDEFENA bit is set.
    pub fn privdefena(&self) -> bool {
        self.read_ctrl().privdefena()
    }

    /// Whether the HFNMIENA bit is set.
    pub fn hfnmiena(&self) -> bool {
        self.read_ctrl().hfnmiena()
    }

    /// Get the number of MPU regions supported by
    /// this MPU.
    pub fn regions(&self) -> u8 {
        self.read_type().dregion()
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for Mpu {
    fn format(&self, fmt: defmt::Formatter) {
        let ctrl = self.read_ctrl();

        defmt::write!(fmt, "Mpu {{\n");
        defmt::write!(fmt, "  enabled: {},\n", ctrl.enable());
        defmt::write!(fmt, "  privdefena: {},\n", ctrl.privdefena());
        defmt::write!(fmt, "  hfnmiena: {},\n", ctrl.hfnmiena());
        defmt::write!(fmt, "  Attributes(8): [\n");
        for idx_num in 0..u3::MAX.value() {
            let idx = AttributeIndex(unsafe { u3::new_unchecked(idx_num) });
            let attr = self.get_attributes(idx);
            defmt::write!(fmt, "    {},\n", attr);
        }
        defmt::write!(fmt, "  ],\n");

        let regions = self.regions();
        defmt::write!(fmt, "  Regions({}): [\n", regions);
        for region_idx in 0..regions {
            let region = RegionToken(region_idx);
            let region = self.get_region(&region);
            defmt::write!(fmt, "    {},\n", region);
        }
        defmt::write!(fmt, "  ]\n}}");
    }
}
