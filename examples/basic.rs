#![no_std]
#![no_main]

use arbitrary_int::u3;
use cortex_m_rt as _;
use defmt::unwrap;
use defmt_rtt as _;
use panic_probe as _;
use thumbv8m_mpu::{
    AccessPermissions, AttributeIndex, MemoryAttributes, Mpu, NormalMemoryAttributes, Region,
    RegionAligned, RegionConfig, RegionRange, Shareability, TransientAllocations,
};

#[cortex_m_rt::entry]
fn main() -> ! {
    static mut STATIC_DMA_MEMORY: RegionAligned<[u8; 1023], 1> = RegionAligned::new([0u8; _]);

    let dynamic_dma_memory: RegionAligned<[u8; 1022], 2> = RegionAligned::new([0u8; _]);
    let peripherals = cortex_m::Peripherals::take().unwrap();
    let mut mpu = Mpu::new(peripherals.MPU);
    let tokens: [_; 8] = mpu.tokens();

    let [mut one, mut two, ..] = tokens;

    let non_cacheable_index: AttributeIndex = unwrap!(u3::try_new(0).ok()).into();
    mpu.set_attributes(non_cacheable_index, MemoryAttributes::non_cacheable());

    let normal_index: AttributeIndex = unwrap!(u3::try_new(1).ok()).into();
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

    let mut overlapping_static_config = config_static.clone();
    let range = config_static.range.get();
    overlapping_static_config.range = unwrap!(RegionRange::new_inclusive(
        *range.start() + 32..=*range.end()
    ));

    unwrap!(mpu.set_region(&mut one, &Region::Enabled(config_static.clone())));
    // Setting the same region twice is OK
    unwrap!(mpu.set_region(&mut one, &Region::Enabled(config_static)));

    unwrap!(mpu.set_region(&mut two, &Region::Enabled(config_dynamic)));

    // Creating overlapping, enabled regions is not. It also does
    // not change the actual value of the region configuration.
    defmt::println!(
        "Result of trying to set overlapping region: {}",
        mpu.set_region(&mut two, &Region::Enabled(overlapping_static_config))
            .unwrap_err()
    );

    mpu.enable(true, false);

    defmt::println!("{}", mpu);

    loop {
        cortex_m_semihosting::debug::exit(cortex_m_semihosting::debug::EXIT_SUCCESS);
    }
}
