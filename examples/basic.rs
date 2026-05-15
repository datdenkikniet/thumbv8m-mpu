#![no_std]
#![no_main]

use cortex_m_rt as _;
use cortexm33_mpu::{
    AccessPermissions, MemoryAttributes, Mpu, Region, RegionAligned, Shareability,
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

    let region_static = Region {
        enabled: true,
        range: STATIC_DMA_MEMORY.as_range(),
        attributes: MemoryAttributes::non_cacheable().into(),
        access_permissions: AccessPermissions::AnyReadWrite,
        execute_never: false,
    };

    let region_dynamic = Region {
        enabled: true,
        range: dynamic_dma_memory.as_range(),
        attributes: MemoryAttributes::non_cacheable().into(),
        access_permissions: AccessPermissions::AnyReadWrite,
        execute_never: false,
    };

    let mut mpu = Mpu::new(&peripherals.CPUID, peripherals.MPU).unwrap();

    mpu.set_region(0, region_static).unwrap();
    mpu.set_region(1, region_dynamic).unwrap();

    mpu.enable(true, false);

    loop {}
}
