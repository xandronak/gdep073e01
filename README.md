# gdep073e01 Driver Crate

[![Crates.io](https://img.shields.io/crates/v/gdep073e01.svg)](https://crates.io/crates/gdep073e01)
[![Documentation](https://docs.rs/gdep073e01/badge.svg)](https://docs.rs/gdep073e01)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Embedded-graphics driver for the Good Display GDEP073E01 7-color e-paper display (800x480 pixels).

This driver provides an interface to the display, implementing the `embedded-graphics` traits `DrawTarget` and `OriginDimensions`. It is based on the C++ implementation found in the [GxEPD2 library by ZinggJM](https://github.com/ZinggJM/GxEPD2).

## Features

*   7-color display support (Black, White, Yellow, Red, Orange, Blue, Green).
*   `DrawTarget` implementation for drawing using `embedded-graphics` primitives and text.
*   `OriginDimensions` implementation for querying display size (800x480).
*   Power control methods (`init`, `sleep`).
*   Internal frame buffer enabling arbitrary drawing operations before flushing to the display.
*   `no_std` compatible, suitable for bare-metal embedded systems.

## Hardware Requirements

*   A GDEP073E01 display module.
*   A microcontroller with an SPI peripheral and sufficient GPIO pins.
*   Connections:
    *   SPI (MOSI, SCK)
    *   Chip Select (CS) - Output Pin
    *   Data/Command (DC) - Output Pin
    *   Reset (RST) - Output Pin
    *   Busy (BUSY) - Input Pin
*   **Important:** The display requires a 3.3V supply and 3.3V logic levels for SPI and GPIO communication.

## Usage

1.  Instantiate your platform's HAL implementations for `SpiDevice`, `OutputPin` (CS, DC, RST), `InputPin` (BUSY), and a `DelayNs` provider.
2.  Create the driver instance using `Gdep073e01::new`.
3.  Initialize the display with `display.init()`.
4.  Use `embedded-graphics` drawing functions (e.g., `draw`, `fill_solid`, `clear`). The `Color` enum from this crate should be used.
5.  Call `display.flush()` to send the buffer contents to the display and trigger a refresh cycle. This can take significant time (tens of seconds) for e-paper displays.
6.  Optionally, call `display.sleep()` to put the display into a low-power deep sleep mode. A hardware reset via `init()` is required to wake it up.

```rust
use embedded_graphics::{
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    text::Text,
};
use embedded_hal::{
    delay::DelayNs,
    digital::{InputPin, OutputPin},
    spi::SpiDevice,
};

// Import the driver crate (replace with actual crate name if different)
use gdep073e01::{
    prelude::*,
    Color, // Import the Color enum
};

fn run_display<
    SPI: SpiDevice,
    CS: OutputPin,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin,
    DELAY: DelayNs,
>(
    spi: SPI,
    cs: CS,
    dc: DC,
    rst: RST,
    busy: BUSY,
    delay: DELAY,
) -> Result<(), Error<SPI::Error, CS::Error>>
where
    // Add Error associated type bounds if OutputPin/InputPin errors differ
    CS::Error: Copy, // Example constraint if needed by Error enum
    DC::Error: Copy,
    RST::Error: Copy,
    BUSY::Error: Copy,
{
    // 2. Create driver instance
    let mut display = Gdep073e01::new(spi, cs, dc, rst, busy, delay);

    // 3. Initialize
    display.init()?;

    // 4. Draw something
    // Clear buffer to white first (optional, default is white)
    display.clear_buffer(Color::White);

    // Draw a red rectangle
    let style_red = PrimitiveStyle::with_stroke(Color::Red, 2);
    Rectangle::new(Point::new(50, 50), Size::new(100, 80))
        .into_styled(style_red)
        .draw(&mut display)?;

    // Draw black text
    let text_style = MonoTextStyle::new(&FONT_10X20, Color::Black);
    Text::new("Hello GDEP073E01!", Point::new(200, 100), text_style)
        .draw(&mut display)?;

    // 5. Flush buffer to display (this performs the actual update)
    println!("Flushing buffer to display..."); // Add logging
    display.flush()?;
    println!("Flush complete.");

    // Wait some time before sleeping
    // display.delay.delay_ms(5000); // Requires access to delay, maybe pass it separately

    // 6. Put display to sleep
    println!("Putting display to sleep...");
    display.sleep()?;
    println!("Display asleep.");

    Ok(())
}
```

*Note: The example function signature includes generic HAL types. You would replace these with concrete types from your specific HAL crate (e.g., `stm32f4xx_hal::spi::Spi`, `stm32f4xx_hal::gpio::PA5<Output<PushPull>>`, etc.) in your application code.* 

## Testing

This crate includes unit tests based on mocking the `embedded-hal` traits. These tests verify the command sequences and buffer manipulation logic without requiring hardware.

Run tests with: `cargo test`

## Verification

The command sequences used in this driver are based on the GxEPD2 C++ library. While they are tested via mocks, verification against the official GDEP073E01 datasheet is recommended if available.

## License

Licensed under the MIT license ([LICENSE](LICENSE) or http://opensource.org/licenses/MIT).