use crate::{
    AccessPermissions, AllocMpu, DeviceMemoryAttributes, DeviceRegion, MpuImpl,
    NormalMemoryAttributes, RegionGroup, RegionGroupError, RegionRange, RegionToken, Shareability,
    alloc::NormalRegion,
    test::{MockMpu, REGIONS},
};

fn nonoverlapping_ranges<const N: usize>() -> [RegionRange; N] {
    super::nonoverlapping_ranges()
        .take(N)
        .collect::<Vec<_>>()
        .try_into()
        .unwrap()
}

#[test]
fn alloc_all_regions() {
    let mut mpu = AllocMpu::new(MockMpu::<REGIONS>::new());

    let ranges: [_; REGIONS] = nonoverlapping_ranges();

    let group = RegionGroup::Normal {
        outer: NormalMemoryAttributes::NonCacheable,
        inner: NormalMemoryAttributes::NonCacheable,
        regions: ranges.map(|range| NormalRegion {
            range,
            access_permissions: AccessPermissions::AnyReadWrite,
            execute_never: false,
            shareability: Shareability::InnerShareable,
        }),
    };

    mpu.enable_group(group).unwrap();
}

#[test]
fn overalloc_regions_fails_and_does_not_write() {
    let mut mpu = AllocMpu::new(MockMpu::<REGIONS>::new());

    let ranges: [_; REGIONS + 1] = nonoverlapping_ranges();

    let group = RegionGroup::Normal {
        outer: NormalMemoryAttributes::NonCacheable,
        inner: NormalMemoryAttributes::NonCacheable,
        regions: ranges.map(|range| NormalRegion {
            range,
            access_permissions: AccessPermissions::AnyReadWrite,
            execute_never: false,
            shareability: Shareability::InnerShareable,
        }),
    };

    assert_eq!(
        mpu.enable_group(group),
        Err(RegionGroupError::RegionsExhausted)
    );

    for token in 0..REGIONS as u8 {
        assert!(!mpu.inner.region(&RegionToken(token)).is_enabled());
        assert!(mpu.inner.region_writes[token as usize] == 0);
    }
}

fn fully_allocated_attrs() -> (AllocMpu<MockMpu<16>>, [RegionRange; 16], RegionToken) {
    // This can only happen if the MPU supports more than 8 regions.
    let mut mpu = AllocMpu::new(MockMpu::<16>::new());

    let ranges: [_; 16] = nonoverlapping_ranges();

    let [r0, r1, r2, r3, r4, r5, r6, tail @ ..] = ranges.clone();

    fn dev_region(range: RegionRange) -> DeviceRegion {
        DeviceRegion {
            range,
            access_permissions: AccessPermissions::AnyReadOnly,
            execute_never: true,
        }
    }

    fn nor_region(range: RegionRange) -> NormalRegion {
        NormalRegion {
            range,
            access_permissions: AccessPermissions::AnyReadOnly,
            execute_never: false,
            shareability: Shareability::NonShareable,
        }
    }

    let g0 = RegionGroup::Device {
        attributes: DeviceMemoryAttributes::None,
        regions: [dev_region(r0)],
    };

    let g1 = RegionGroup::Device {
        attributes: DeviceMemoryAttributes::NonGathering,
        regions: [dev_region(r1)],
    };

    let g2 = RegionGroup::Device {
        attributes: DeviceMemoryAttributes::NonGatheringNonReordering,
        regions: [dev_region(r2)],
    };

    let g3 = RegionGroup::Device {
        attributes: DeviceMemoryAttributes::All,
        regions: [dev_region(r3)],
    };

    let g4 = RegionGroup::Normal {
        outer: NormalMemoryAttributes::NonCacheable,
        inner: NormalMemoryAttributes::NonCacheable,
        regions: [nor_region(r4)],
    };

    let g5 = RegionGroup::Normal {
        outer: NormalMemoryAttributes::WriteBackNonTransient {
            allocate_reads: false,
            allocate_writes: false,
        },
        inner: NormalMemoryAttributes::NonCacheable,
        regions: [nor_region(r5)],
    };

    let g6 = RegionGroup::Normal {
        outer: NormalMemoryAttributes::WriteBackNonTransient {
            allocate_reads: true,
            allocate_writes: false,
        },
        inner: NormalMemoryAttributes::NonCacheable,
        regions: tail.map(nor_region),
    };

    let g7 = RegionGroup::Normal {
        outer: NormalMemoryAttributes::WriteBackNonTransient {
            allocate_reads: false,
            allocate_writes: true,
        },
        inner: NormalMemoryAttributes::NonCacheable,
        regions: [nor_region(r6)],
    };

    mpu.enable_group(g0).unwrap();
    mpu.enable_group(g1).unwrap();
    mpu.enable_group(g2).unwrap();
    mpu.enable_group(g3).unwrap();
    mpu.enable_group(g4).unwrap();
    mpu.enable_group(g5).unwrap();
    mpu.enable_group(g6).unwrap();
    let [g7_token] = mpu.enable_group(g7).unwrap();

    (mpu, ranges, g7_token)
}

#[test]
fn overalloc_attrs_fails() {
    let (mut mpu, [.., range], _) = fully_allocated_attrs();

    let g8 = RegionGroup::Normal {
        outer: NormalMemoryAttributes::WriteBackNonTransient {
            allocate_reads: true,
            allocate_writes: true,
        },
        inner: NormalMemoryAttributes::NonCacheable,
        regions: [NormalRegion {
            range,
            access_permissions: AccessPermissions::AnyReadOnly,
            execute_never: false,
            shareability: Shareability::InnerShareable,
        }],
    };

    assert_eq!(
        mpu.enable_group(g8),
        Err(RegionGroupError::AttributesExhausted)
    );

    for token in 0..16 as u8 {
        assert!(mpu.inner.region(&RegionToken(token)).is_enabled());
        assert!(mpu.inner.region_writes[token as usize] == 1);
    }
}

#[test]
fn freeing_attr_works() {
    let (mut mpu, _, g7) = fully_allocated_attrs();
    mpu.disable_region(g7);

    let range = RegionRange::new(32 * 100..32 * 101).unwrap();
    let g8 = RegionGroup::Normal {
        outer: NormalMemoryAttributes::WriteBackNonTransient {
            allocate_reads: true,
            allocate_writes: true,
        },
        inner: NormalMemoryAttributes::NonCacheable,
        regions: [NormalRegion {
            range,
            access_permissions: AccessPermissions::AnyReadOnly,
            execute_never: false,
            shareability: Shareability::InnerShareable,
        }],
    };

    assert_eq!(mpu.enable_group(g8), Ok([RegionToken(15)]));

    for token in 0..15 as u8 {
        assert!(mpu.inner.region(&RegionToken(token)).is_enabled());
        assert!(mpu.inner.region_writes[token as usize] == 1);
    }

    assert!(mpu.inner.region(&RegionToken(15)).is_enabled());
    assert!(mpu.inner.region_writes[15] == 3);
}

#[test]
fn inter_group_overlaps_not_allowed() {
    let mut mpu = AllocMpu::new(MockMpu::<REGIONS>::new());

    let [range] = nonoverlapping_ranges();

    let region = DeviceRegion {
        range: range.clone(),
        access_permissions: AccessPermissions::AnyReadOnly,
        execute_never: true,
    };

    let g0 = RegionGroup::Device {
        regions: [region.clone()],
        attributes: DeviceMemoryAttributes::None,
    };

    let g1 = RegionGroup::Device {
        regions: [region.clone()],
        attributes: DeviceMemoryAttributes::None,
    };

    assert!(mpu.enable_group(g0).is_ok());
    assert_eq!(
        mpu.enable_group(g1),
        Err(RegionGroupError::OverlappingRanges {
            requested: range.clone(),
            overlapping: range.clone()
        })
    )
}

#[test]
fn intra_group_overlaps_not_allowed() {
    let mut mpu = AllocMpu::new(MockMpu::<REGIONS>::new());

    let [range] = nonoverlapping_ranges();

    let region = DeviceRegion {
        range: range.clone(),
        access_permissions: AccessPermissions::AnyReadOnly,
        execute_never: true,
    };

    let group = RegionGroup::Device {
        regions: [region.clone(), region.clone()],
        attributes: DeviceMemoryAttributes::None,
    };

    assert_eq!(
        mpu.enable_group(group),
        Err(RegionGroupError::OverlappingRanges {
            requested: range.clone(),
            overlapping: range.clone(),
        })
    )
}
