# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] - 2025-04-16

### Fixed

*   Resolved stack overflow panic in `flush()` by sending the display buffer in chunks instead of using a large temporary stack allocation (`mem::swap`). This prevents `IllegalInstruction` errors on constrained devices.

## [0.1.0] - 2025-04-16

### Added

*   Initial release.
*   Support for GDEP073E01 7-color e-paper display (800x480).
*   Implementation based on the GxEPD2 C++ library.
*   `embedded-graphics` `DrawTarget` and `OriginDimensions` implementations.
*   Basic display control functions: `init`, `flush`, `sleep`, `clear_buffer`, `set_pixel`.
*   HAL-agnostic design using `embedded-hal` version 1.0 traits.
*   Internal frame buffer (2 pixels per byte).
*   Mock-based unit tests for core functionality (`init`, buffer manipulation).
*   Crate documentation and README.

[0.1.1]: https://github.com/xandronak/gdep073e01/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/xandronak/gdep073e01/releases/tag/v0.1.0 