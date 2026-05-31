mod ll;

use crate::*;

struct MockMpu<const N: usize> {
    ctrl: Control,
    attributes: [MemoryAttributes; 8],
    regions: [Region; N],
}

impl<const N: usize> MockMpu<N> {
    pub const fn new() -> Self {
        // Testing other sizes is nonsensical
        const { core::assert!(N == 8 || N == 16) };

        Self {
            ctrl: Control::ZERO,
            attributes: [const { MemoryAttributes::non_cacheable() }; _],
            regions: [const { Region::Disabled }; _],
        }
    }
}

impl<const N: usize> crate::MpuImpl for MockMpu<N> {
    fn regions(&self) -> u8 {
        N as _
    }

    fn attributes(&self, index: AttributeIndex) -> MemoryAttributes {
        self.attributes[index.get() as usize]
    }

    fn set_attributes(&mut self, index: AttributeIndex, attrs: MemoryAttributes) {
        self.attributes[index.get() as usize] = attrs;
    }

    fn control(&self) -> Control {
        self.ctrl
    }

    fn set_control(&mut self, control: Control) {
        self.ctrl = control;
    }

    fn region(&self, token: &RegionToken) -> Region {
        self.regions[token.0 as usize].clone()
    }

    fn set_region(&mut self, token: &mut RegionToken, region: &Region) {
        self.regions[token.0 as usize] = region.clone();
    }
}

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
