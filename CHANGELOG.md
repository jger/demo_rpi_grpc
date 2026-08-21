## [1.0.6](https://github.com/jger/demo_rpi_grpc/compare/v1.0.5...v1.0.6) (2026-08-21)


### Bug Fixes

* change duration type to u32 and handle Unspecified switch state ([40c888b](https://github.com/jger/demo_rpi_grpc/commit/40c888bfddfded99bdaaee38c67afea7db0c8485))
* remove software debounce logic in favor of hardware filtering in pin monitoring ([5aea8a4](https://github.com/jger/demo_rpi_grpc/commit/5aea8a4052995e3af392a816ba32396a1d3fe356))

## [1.0.5](https://github.com/jger/demo_rpi_grpc/compare/v1.0.4...v1.0.5) (2026-08-21)


### Bug Fixes

* implement robust GPIO debouncing logic with a dedicated service module and unified monitoring loop ([8676ed0](https://github.com/jger/demo_rpi_grpc/commit/8676ed08547841aa3fa4e2c8311a6f6579e82346))

## [1.0.4](https://github.com/jger/demo_rpi_grpc/compare/v1.0.3...v1.0.4) (2026-08-21)


### Bug Fixes

* **ci:** switch Linux and RPi targets to static musl to eliminate GLIBC dependency ([141eee5](https://github.com/jger/demo_rpi_grpc/commit/141eee5cd7083f6360bbaee88ba8d6b395441ef3))

## [1.0.3](https://github.com/jger/demo_rpi_grpc/compare/v1.0.2...v1.0.3) (2026-08-21)


### Bug Fixes

* **ci:** use macos-latest runner for x86_64-apple-darwin target ([601f05d](https://github.com/jger/demo_rpi_grpc/commit/601f05d4b20768a66507515d4d3fd4cc072edf34))

## [1.0.2](https://github.com/jger/demo_rpi_grpc/compare/v1.0.1...v1.0.2) (2026-08-21)


### Bug Fixes

* publish standalone binary assets to github release ([4533f9d](https://github.com/jger/demo_rpi_grpc/commit/4533f9d2c914d7f4a1f6132a0f662c218d4eae63))
* switch release preset to angular and remove unused changelog dependency ([13ffac2](https://github.com/jger/demo_rpi_grpc/commit/13ffac25a0e23710898f7c140930cd74c562f8a2))

## [1.0.1](https://github.com/jger/demo_rpi_grpc/compare/v1.0.0...v1.0.1) (2026-08-20)

### Bug Fixes

* bundle platform-prefixed binaries and generate individual checksums in release workflow ([c094711](https://github.com/jger/demo_rpi_grpc/commit/c09471191ec6c4e5dd3c330fe69175cca120f3c4))

## 1.0.0 (2026-08-20)

### Features

* add Makefile to automate build, cross-compile, and deployment tasks with updated README documentation ([28bf6fc](https://github.com/jger/demo_rpi_grpc/commit/28bf6fce32f7312283b1f744d6643ee82602d44d))

### Bug Fixes

* implement GPIO interrupt debouncing and update pin type definitions to u8 ([c350c32](https://github.com/jger/demo_rpi_grpc/commit/c350c32d8144835c8c9e66e478406c4d68451ffc))
