#![no_std]
#![no_main]

use arbitrary_int::u3;
use cortex_m_rt as _;
use defmt_rtt as _;
use thumbv8m_mpu::{
    AccessPermissions, AttributeIndex, MemoryAttributes, Mpu, NormalMemoryAttributes, Region,
    RegionAligned, RegionConfig, Shareability, TransientAllocations,
};

#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[cortex_m_rt::entry]
fn main() -> ! {
    static mut STATIC_DMA_MEMORY: RegionAligned<[u8; 1023], 1> = RegionAligned::new([0u8; _]);

    let dynamic_dma_memory: RegionAligned<[u8; 1022], 2> = RegionAligned::new([0u8; _]);
    let peripherals = cortex_m::Peripherals::take().unwrap();
    let mut mpu = Mpu::new(peripherals.MPU);
    let mut tokens: [_; 8] = mpu.tokens();

    let non_cacheable_index: AttributeIndex = u3::try_new(0).unwrap().into();
    mpu.set_attributes(non_cacheable_index, MemoryAttributes::non_cacheable());

    let normal_index: AttributeIndex = u3::try_new(1).unwrap().into();
    mpu.set_attributes(
        normal_index,
        MemoryAttributes::Normal {
            outer: NormalMemoryAttributes::WriteThroughNonTransient {
                allocate_reads: true,
                allocate_writes: false,
            },
            inner: NormalMemoryAttributes::WriteBackTransient(TransientAllocations::AllocateBoth),
        },
    );

    let config_static = RegionConfig {
        range: STATIC_DMA_MEMORY.as_range(),
        attribute_index: non_cacheable_index,
        shareability: Shareability::OuterShareable,
        access_permissions: AccessPermissions::AnyReadWrite,
        execute_never: false,
    };

    let config_dynamic = RegionConfig {
        range: dynamic_dma_memory.as_range(),
        attribute_index: normal_index,
        shareability: Shareability::InnerShareable,
        access_permissions: AccessPermissions::PrivilegedReadOnly,
        execute_never: true,
    };

    mpu.set_region(&mut tokens[0], &Region::Enabled(config_static))
        .unwrap();
    mpu.set_region(&mut tokens[1], &Region::Enabled(config_dynamic))
        .unwrap();

    mpu.enable(true, false);

    defmt::println!("{}", mpu);

    loop {}
}
