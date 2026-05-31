use arbitrary_int::{traits::Integer, u3};

use crate::{
    AccessPermissions, AttributeIndex, DeviceMemoryAttributes, LlMpu, MemoryAttributes, Region,
    RegionConfig, RegionToken, Shareability,
    test::{MockMpu, REGIONS, nonoverlapping_ranges},
};

#[test]
fn can_assign_all() {
    let mut mpu = LlMpu::new(MockMpu::<REGIONS>::new());
    let mut tokens = mpu.tokens::<REGIONS>();

    for (token, range) in tokens.iter_mut().zip(nonoverlapping_ranges()) {
        let region = &Region::Enabled(RegionConfig {
            range,
            shareability: Shareability::InnerShareable,
            attribute_index: AttributeIndex::new(0).unwrap(),
            access_permissions: AccessPermissions::AnyReadOnly,
            execute_never: true,
        });
        mpu.set_region(token, region).unwrap();
    }

    for token in tokens.iter() {
        assert!(mpu.region(token).is_enabled());
    }
}

#[test]
fn disable_region() {
    let mut mpu = LlMpu::new(MockMpu::<REGIONS>::new());
    let mut tokens = mpu.tokens::<REGIONS>();

    for (token, range) in tokens.iter_mut().zip(nonoverlapping_ranges()) {
        let region = &Region::Enabled(RegionConfig {
            range,
            shareability: Shareability::InnerShareable,
            attribute_index: AttributeIndex::new(0).unwrap(),
            access_permissions: AccessPermissions::AnyReadOnly,
            execute_never: true,
        });
        mpu.set_region(token, region).unwrap();
    }

    mpu.set_region(&mut tokens[4], &Region::Disabled).unwrap();

    for token in tokens.iter() {
        if token == &RegionToken(4) {
            assert!(
                !mpu.region(token).is_enabled(),
                "expected region {token:?} to be disabled",
            );
        } else {
            assert!(
                mpu.region(token).is_enabled(),
                "expected region {token:?} to be enabled",
            );
        }
    }
}

#[test]
fn cannot_assign_overlapping() {
    let mut mpu = LlMpu::new(MockMpu::<REGIONS>::new());
    let [mut t0, mut t1, ..] = mpu.tokens::<REGIONS>();

    let range = nonoverlapping_ranges().next().unwrap();

    let region = Region::Enabled(RegionConfig {
        range,
        shareability: Shareability::InnerShareable,
        attribute_index: AttributeIndex::new(0).unwrap(),
        access_permissions: AccessPermissions::AnyReadOnly,
        execute_never: true,
    });

    mpu.set_region(&mut t0, &region).unwrap();
    assert!(mpu.set_region(&mut t1, &region).is_err());
}

#[test]
#[should_panic]
fn can_take_tokens_only_once() {
    let mut mpu = LlMpu::new(MockMpu::<REGIONS>::new());

    let _tokens: [_; REGIONS] = mpu.tokens();

    let _: [_; REGIONS] = mpu.tokens();
}

#[test]
fn can_set_all_attributes() {
    let mut mpu = LlMpu::new(MockMpu::<REGIONS>::new());

    let config = MemoryAttributes::Device(DeviceMemoryAttributes::NonGatheringNonReordering);

    for index in 0..u3::MAX.value() {
        let index = AttributeIndex::new(index).unwrap();
        mpu.set_attributes(index, config.clone());
    }

    for index in 0..u3::MAX.value() {
        let index = AttributeIndex::new(index).unwrap();
        assert_eq!(mpu.attributes(index), config);
    }
}

#[test]
#[should_panic]
fn cannot_take_more_tokens_than_supported() {
    let mut mpu = LlMpu::new(MockMpu::<8>::new());

    let _tokens: [_; 16] = mpu.tokens();
}

// Slightly silly test, but why not 🤷
#[test]
fn enable() {
    let mut mpu = LlMpu::new(MockMpu::<8>::new());

    mpu.enable(true, true);

    assert!(mpu.enabled());
    assert!(mpu.hfnmiena());
    assert!(mpu.privdefena());
}
