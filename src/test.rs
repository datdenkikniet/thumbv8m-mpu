use std::u32;

use crate::{regs::BaseAddress, *};

#[test]
fn aligned_valid_len_region() {
    assert!(RegionRange::new_raw(0, 31).is_ok());
}

#[test]
fn aligned_invalid_len_region() {
    assert!(RegionRange::new_raw(0, 32).is_err());
}

#[test]
fn misaligned_invalid_len_region() {
    assert!(RegionRange::new_raw(1, 32).is_err());
}

#[test]
fn region_from_aligned_struct() {
    let aligned = RegionAligned::new([0u8; 32]);
    let region = RegionRange::from_aligned(&aligned);

    assert!(
        region
            .range
            .start()
            .is_multiple_of(RegionRange::REQUIRED_ALIGNMENT)
    );

    let diff = region.range.end() - region.range.start();
    assert_eq!(diff, 31)
}
