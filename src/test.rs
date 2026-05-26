use crate::*;

#[test]
fn aligned_valid_len_region() {
    assert!(RegionRange::new_inclusive(0..=31).is_ok());
}

#[test]
fn aligned_valid_len_region_exclusive() {
    assert!(RegionRange::new(0..32).is_ok());
}

#[test]
fn aligned_invalid_len_region() {
    assert_eq!(
        RegionRange::new_inclusive(0..=32),
        Err(RegionRangeError::EndMisaligned)
    );
}

#[test]
fn aligned_invalid_len_region_exclusive() {
    assert_eq!(
        RegionRange::new(0..33),
        Err(RegionRangeError::EndMisaligned)
    );
}

#[test]
fn misaligned_invalid_len_region() {
    assert_eq!(
        RegionRange::new_inclusive(1..=32),
        Err(RegionRangeError::StartMisaligned)
    );
}

#[test]
fn misaligned_invalid_len_exclusive() {
    assert_eq!(
        RegionRange::new(1..33),
        Err(RegionRangeError::StartMisaligned)
    );
}

#[test]
fn start_greater_than_end() {
    assert_eq!(
        RegionRange::new_inclusive(32..=31),
        Err(RegionRangeError::EndBeforeStart)
    );
}

#[test]
fn start_greater_than_end_exclusive() {
    assert_eq!(
        RegionRange::new(32..32),
        Err(RegionRangeError::EndBeforeStart)
    );
}

#[test]
fn region_from_aligned_struct() {
    let aligned = RegionAligned::<_, 0>::new([0u8; 32]);
    let region = aligned.as_range();

    assert!(
        region
            .range
            .start()
            .is_multiple_of(RegionRange::REQUIRED_ALIGNMENT)
    );

    let diff = region.range.end() - region.range.start();
    assert_eq!(diff, 31)
}
