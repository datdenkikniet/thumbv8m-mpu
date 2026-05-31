//! Low-level access to the MPU.
//!
//! Using the [`LlMpu`] provided by this module requires that the
//! application manages attributes and regions itself. The only validation
//! performed by this implementation is that configured regions do not overlap.

use crate::{
    AttributeIndex, MemoryAttributes, MpuImpl, Region, RegionConfig, RegionToken, regs::Control,
};

#[cfg(feature = "defmt")]
use {
    arbitrary_int::{traits::Integer, u3},
    defmt::assert,
};

#[cfg(not(feature = "defmt"))]
use core::assert;

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
pub struct LlMpu<Impl: MpuImpl> {
    pub(crate) mpu: Impl,
    took_tokens: bool,
}

impl<Impl: MpuImpl> LlMpu<Impl> {
    /// Instantiate the MPU.
    pub const fn new(mpu: Impl) -> Self {
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

    /// Get the attributes specified at `index`.
    pub fn attributes(&self, index: AttributeIndex) -> MemoryAttributes {
        self.mpu.attributes(index)
    }

    /// Set the attributes at `index` to `attributes`.
    ///
    /// This affect all regions whose `index` is set to `index`.
    pub fn set_attributes(&mut self, index: AttributeIndex, attributes: MemoryAttributes) {
        self.mpu.set_attributes(index, attributes);
    }

    /// Get the configuration of the region associated with `token`.
    pub fn region(&self, token: &RegionToken) -> Region {
        self.mpu.region(token)
    }

    /// Set the configuration of the region associated with `token` to `region`.
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
                    self.region(&RegionToken(other_region_num))
                } else {
                    continue;
                };

                let Region::Enabled(other_region_config) = other_region else {
                    continue;
                };

                // Manual implementation of currently-unstable `RangeInclusive::is_overlapping`
                if region.range.overlaps(&other_region_config.range) {
                    return Err(OverlappingRanges {
                        region: other_region_num,
                        config: other_region_config,
                    });
                }
            }
        }

        self.mpu.set_region(token, region);

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
            .with_privdefena(privileged_sw_may_access_default_map);

        self.mpu.set_control(ctrl.build());
    }

    /// Disable the MPU
    pub fn disable(&mut self) {
        self.mpu.set_control(Control::default());
    }

    /// Whether the MPU is enabled.
    pub fn enabled(&self) -> bool {
        self.mpu.control().enable()
    }

    /// Whether the PRIVDEFENA bit is set.
    pub fn privdefena(&self) -> bool {
        self.mpu.control().privdefena()
    }

    /// Whether the HFNMIENA bit is set.
    pub fn hfnmiena(&self) -> bool {
        self.mpu.control().hfnmiena()
    }

    /// Get the number of MPU regions supported by
    /// this MPU.
    pub fn regions(&self) -> u8 {
        self.mpu.regions()
    }
}

#[cfg(feature = "defmt")]
impl<Impl: MpuImpl> defmt::Format for LlMpu<Impl> {
    fn format(&self, fmt: defmt::Formatter) {
        let ctrl = self.mpu.control();

        defmt::write!(fmt, "Mpu {{\n");
        defmt::write!(fmt, "  enabled: {},\n", ctrl.enable());
        defmt::write!(fmt, "  privdefena: {},\n", ctrl.privdefena());
        defmt::write!(fmt, "  hfnmiena: {},\n", ctrl.hfnmiena());
        defmt::write!(fmt, "  Attributes(8): [\n");
        for idx_num in 0..u3::MAX.value() {
            let idx = AttributeIndex(unsafe { u3::new_unchecked(idx_num) });
            let attr = self.attributes(idx);
            defmt::write!(fmt, "    {},\n", attr);
        }
        defmt::write!(fmt, "  ],\n");

        let regions = self.regions();
        defmt::write!(fmt, "  Regions({}): [\n", regions);
        for region_idx in 0..regions {
            let region = RegionToken(region_idx);
            let region = self.region(&region);
            defmt::write!(fmt, "    {},\n", region);
        }
        defmt::write!(fmt, "  ]\n}}");
    }
}
