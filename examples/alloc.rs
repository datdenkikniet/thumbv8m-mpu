#![no_std]
#![no_main]

use cortex_m_rt as _;
use defmt::unwrap;
use defmt_rtt as _;
use panic_probe as _;
use thumbv8m_mpu::{
    AccessPermissions, AllocMpu, NormalMemoryAttributes, RegionAligned, RegionGroup, Shareability,
    TransientAllocations, alloc::NormalRegion,
};

#[cortex_m_rt::entry]
fn main() -> ! {
    static mut STATIC_DMA_MEMORY: RegionAligned<[u8; 1023]> = RegionAligned::new([0u8; _]);

    let dynamic_dma_memory: RegionAligned<[u8; 1022]> = RegionAligned::new([0u8; _]);
    let peripherals = cortex_m::Peripherals::take().unwrap();
    let mut mpu = AllocMpu::new(peripherals.MPU);

    let group_static = RegionGroup::normal_non_cacheable([NormalRegion {
        range: STATIC_DMA_MEMORY.as_range(),
        access_permissions: AccessPermissions::AnyReadWrite,
        shareability: Shareability::OuterShareable,
        execute_never: false,
    }]);

    let group_dynamic = RegionGroup::Normal {
        outer: NormalMemoryAttributes::WriteThroughNonTransient {
            allocate_reads: true,
            allocate_writes: false,
        },
        inner: NormalMemoryAttributes::WriteBackTransient(TransientAllocations::AllocateBoth),
        regions: [NormalRegion {
            range: dynamic_dma_memory.as_range(),
            shareability: Shareability::InnerShareable,
            access_permissions: AccessPermissions::PrivilegedReadOnly,
            execute_never: true,
        }],
    };

    unwrap!(mpu.enable_group(group_static));
    unwrap!(mpu.enable_group(group_dynamic));

    mpu.enable(true, false);

    loop {
        cortex_m_semihosting::debug::exit(cortex_m_semihosting::debug::EXIT_SUCCESS);
    }
}
