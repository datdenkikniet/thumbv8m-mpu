//! MPU with quasi-allocation support.
//!
//! The [`LlMpu`](crate::ll::LlMpu) requires that the application manages
//! tokens and attributes. This implementation allocates them for you, and
//! returns errors in case it cannot accomodate the requested configuration.

use core::mem::MaybeUninit;

use arbitrary_int::{traits::Integer, u3};

use crate::{
    AccessPermissions, AttributeIndex, Control, DeviceMemoryAttributes, MemoryAttributes, MpuImpl,
    NormalMemoryAttributes, Region, RegionRange, RegionToken, Shareability,
};

/// Errors that can occur while allocating a region group.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, PartialEq)]
pub enum RegionGroupError {
    /// All attribute slots contain an attribute configuration
    /// that is used and does not match the requested attribute
    /// configuration.
    AttributesExhausted,
    /// Enabling the requested configuration would cause overlapping
    /// regions to be allocated.
    OverlappingRanges {
        /// The requested range.
        requested: RegionRange,
        /// The overlapping range.
        overlapping: RegionRange,
    },
    /// Allocating the requested group uses more regions than
    /// are available.
    RegionsExhausted,
}

/// Configuration for a device region.
#[derive(Debug, Clone)]
pub struct DeviceRegion {
    /// The range of addresses in this region.
    pub range: RegionRange,
    /// The access permissions for this region.
    pub access_permissions: AccessPermissions,
    /// Whether it should be possible to execute memory from
    /// this region, provided that is readable.
    pub execute_never: bool,
}

/// Configuration for a normal (non-device) region.
#[derive(Debug, Clone)]
pub struct NormalRegion {
    /// The range of addresses in this region.
    pub range: RegionRange,
    /// The access permissions for this region.
    pub access_permissions: AccessPermissions,
    /// Whether it should be possible to execute memory from
    /// this region, provided that is readable.
    pub execute_never: bool,
    /// The ways in which this region is shared.
    pub shareability: Shareability,
}

/// A group of regions that share memory attributes.
#[derive(Debug, Clone)]
pub enum RegionGroup<const N: usize> {
    /// A group containing normal (non-device) memory.
    Normal {
        /// The attributes of the outer (inter-bus-master
        /// caching) memory in these regions.
        outer: NormalMemoryAttributes,
        /// The attributes of the outer (single-core
        /// caching) memory in these regions.
        inner: NormalMemoryAttributes,
        /// The regions that make up this group.
        regions: [NormalRegion; N],
    },
    /// A group cnotaining device memory.
    Device {
        /// The attributes of the memory in these regions.
        attributes: DeviceMemoryAttributes,
        /// The regions that make up this group.
        regions: [DeviceRegion; N],
    },
}

impl<const N: usize> RegionGroup<N> {
    /// Create a new region group, whose memory should be marked
    /// as non-cacheable.
    pub const fn non_cacheable(regions: [DeviceRegion; N]) -> Self {
        Self::Device {
            attributes: DeviceMemoryAttributes::None,
            regions,
        }
    }
}

/// An MPU implementation that handles allocation of
/// attributes and regions internally.
pub struct AllocMpu<Impl: MpuImpl> {
    pub(crate) inner: Impl,
}

impl<Impl: MpuImpl> AllocMpu<Impl> {
    fn setup_attribute(
        &mut self,
        attributes: MemoryAttributes,
    ) -> Result<AttributeIndex, RegionGroupError> {
        for idx in 0..u3::MAX.value() {
            let attr_index = AttributeIndex(u3::new(idx));
            let attr = self.inner.attributes(attr_index);

            if attr == attributes {
                return Ok(attr_index);
            }
        }

        let mut used_attributes = [false; 8];

        for region in 0..self.inner.regions() {
            let token = RegionToken(region);
            let Region::Enabled(config) = self.inner.region(&token) else {
                continue;
            };

            used_attributes[config.attribute_index.get() as usize] = true;
        }

        let unused_attribute = used_attributes
            .into_iter()
            .enumerate()
            .find_map(|(idx, v)| (!v).then_some(AttributeIndex::new(idx as _).unwrap()));

        unused_attribute.ok_or(RegionGroupError::AttributesExhausted)
    }

    fn find_disabled_regions<const N: usize>(
        &mut self,
    ) -> Result<[RegionToken; N], RegionGroupError> {
        let mut regions = [const { MaybeUninit::uninit() }; N];

        let mut index = 0;

        for region in 0..self.inner.regions() {
            let token = RegionToken(region);
            let region = self.inner.region(&token);

            if !region.is_enabled() {
                regions[index].write(token);
                index += 1;

                if index == N {
                    break;
                }
            }
        }

        if index == N {
            let regions = regions.map(|v| {
                // SAFETY: `index == N`, so all tokens have been initialized.
                unsafe { v.assume_init() }
            });
            Ok(regions)
        } else {
            Err(RegionGroupError::RegionsExhausted)
        }
    }

    /// Initialize an MPU with the provided implementation.
    pub const fn new(inner: Impl) -> Self {
        Self { inner }
    }

    /// Disable the region associated with `token`.
    pub fn disable_region(&mut self, mut token: RegionToken) {
        self.inner.set_region(&mut token, &Region::Disabled);
    }

    /// Enable a group of regions.
    ///
    /// If an error is returned, the MPU state remains unchanged.
    ///
    /// Note that this change does not take effect unless
    /// [`AllocMpu::enable`] is called.
    pub fn enable_group<const N: usize>(
        &mut self,
        group: RegionGroup<N>,
    ) -> Result<[RegionToken; N], RegionGroupError> {
        // Check for inter-group overlaps
        let ranges = match &group {
            RegionGroup::Normal { regions, .. } => regions.clone().map(|r| r.range),
            RegionGroup::Device { regions, .. } => regions.clone().map(|r| r.range),
        };

        for (idx, range0) in ranges.iter().enumerate() {
            for range1 in ranges.iter().skip(idx + 1) {
                if range0.overlaps(range1) {
                    return Err(RegionGroupError::OverlappingRanges {
                        requested: range0.clone(),
                        overlapping: range1.clone(),
                    });
                }
            }
        }

        let attributes = match group {
            RegionGroup::Normal { outer, inner, .. } => MemoryAttributes::Normal { outer, inner },
            RegionGroup::Device { attributes, .. } => MemoryAttributes::Device(attributes),
        };

        let attribute_index = self.setup_attribute(attributes)?;
        self.inner.set_attributes(attribute_index, attributes);

        let configs = match group {
            RegionGroup::Normal {
                regions: config, ..
            } => config.map(|v| crate::RegionConfig {
                range: v.range,
                shareability: v.shareability,
                attribute_index,
                access_permissions: v.access_permissions,
                execute_never: v.execute_never,
            }),
            RegionGroup::Device {
                regions: config, ..
            } => {
                config.map(|v| crate::RegionConfig {
                    range: v.range,
                    // This value is ignored for regions with DeviceMemoryAttrributes.
                    // Be as pessimistic as we can, for good measure.
                    shareability: Shareability::OuterShareable,
                    attribute_index,
                    access_permissions: v.access_permissions,
                    execute_never: v.execute_never,
                })
            }
        };

        let mut tokens: [_; N] = self.find_disabled_regions()?;

        // Check for intra-group overlaps
        for token in 0..self.regions() {
            let token = RegionToken(token);
            let Region::Enabled(other_region_config) = self.inner.region(&token) else {
                continue;
            };

            for config in configs.iter() {
                if config.range.overlaps(&other_region_config.range) {
                    return Err(RegionGroupError::OverlappingRanges {
                        requested: config.range.clone(),
                        overlapping: other_region_config.range.clone(),
                    });
                }
            }
        }

        for (token, config) in tokens.iter_mut().zip(configs) {
            self.inner.set_region(token, &Region::Enabled(config));
        }

        Ok(tokens)
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
            .with_privdefena(privileged_sw_may_access_default_map)
            .with_hfnmiena(enable_mpu_in_nmi_and_hardfault);
        self.inner.set_control(ctrl.build());
    }

    /// Disable the MPU
    pub fn disable(&mut self) {
        self.inner.set_control(Control::ZERO);
    }

    /// Whether the MPU is enabled.
    pub fn enabled(&self) -> bool {
        self.inner.control().enable()
    }

    /// Whether the PRIVDEFENA bit is set.
    pub fn privdefena(&self) -> bool {
        self.inner.control().privdefena()
    }

    /// Whether the HFNMIENA bit is set.
    pub fn hfnmiena(&self) -> bool {
        self.inner.control().hfnmiena()
    }

    /// Get the number of MPU regions supported by
    /// this MPU.
    pub fn regions(&self) -> u8 {
        self.inner.regions()
    }
}
