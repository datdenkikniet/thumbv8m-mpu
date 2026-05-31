//! Architecture-specific implementations.
//!
//! To allow for testing on non-ARM platforms, we put ARM-specific logic
//! in a module.

use crate::{
    AttributeIndex, Control, MemoryAttributes, MpuImpl, Region, RegionConfig, RegionRange,
    RegionToken,
    regs::{BaseAddress, LimitAddress, Type},
};

impl MpuImpl for cortex_m::peripheral::MPU {
    fn regions(&self) -> u8 {
        Type::new_with_raw_value(self._type.read()).dregion()
    }

    fn attributes(&self, index: AttributeIndex) -> MemoryAttributes {
        let num = index.get();
        let index = (num / 4) as usize;
        let shift = (num % 4) * 8;
        let reg = self.mair[index].read();
        let value = (reg >> shift) as u8;
        MemoryAttributes::decode(value)
    }

    fn set_attributes(&self, index: AttributeIndex, attrs: MemoryAttributes) {
        let num = index.get();
        let index = (num / 4) as usize;
        let shift = (num % 4) * 8;
        let mask = 0xFF << shift;

        unsafe { self.mair[index].modify(|w| w & !mask | ((attrs.encode() as u32) << shift)) };
    }

    fn region(&self, token: &RegionToken) -> Region {
        // SAFETY: the side-effect of writing a different set
        // of region registers is accounted for.
        unsafe { self.rnr.write(token.0 as _) };
        let limit = LimitAddress::new_with_raw_value(self.rlar.read());
        let base = BaseAddress::new_with_raw_value(self.rbar.read());
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

    fn set_region(&mut self, token: &mut RegionToken, region: &Region) {
        let num = token.0;

        unsafe { self.rnr.write(num as _) };

        if let Region::Enabled(region) = region {
            // No overlapping ranges, so we can set up the region.
            let start = *region.range.get().start() >> 5;
            let base = BaseAddress::builder()
                .with_base(arbitrary_int::u27::new(start))
                .with_shareability(region.shareability)
                .with_access_permissions(region.access_permissions)
                .with_execute_never(region.execute_never)
                .build();

            let end = *region.range.get().end() >> 5;
            let limit = LimitAddress::builder()
                .with_enable(true)
                .with_attr_index(region.attribute_index.into())
                .with_limit(arbitrary_int::u27::new(end))
                .with_reserved(false)
                .build();

            unsafe { self.rbar.write(base.raw_value()) };
            unsafe { self.rlar.write(limit.raw_value()) };
        } else {
            unsafe {
                self.rlar
                    .write(LimitAddress::ZERO.with_enable(false).raw_value())
            };
        }

        cortex_m::asm::dsb();
        cortex_m::asm::isb();
    }

    fn set_control(&mut self, control: Control) {
        unsafe { self.ctrl.write(control.raw_value()) };

        cortex_m::asm::dsb();
        cortex_m::asm::isb();
    }

    fn control(&self) -> Control {
        Control::new_with_raw_value(self.ctrl.read())
    }
}
