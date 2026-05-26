# Support for Thumbv8m MPUs

`cortex-m` provides low-level access to the MPUs on `thumbv8m-main` MCUs. In order to make working with the MPU simpler,
this crate provides more abstract access to the registers, and some helpers to make aligning data to the MPU boundaries
simpler.

Check out [the docs](https://docs.rs/thumbv8m-mpu) for more information about what is available, and [the example](./examples/basic.rs)
on how this crate can be used.

## Tested chips

This implementation has been tested on an microcontroller with a `cortex-m33` core.

# License

The code in this project is licensed under the MIT license. See [LICENSE](./LICENSE).
