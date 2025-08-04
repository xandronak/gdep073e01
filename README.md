# GDEP073E01 E-Paper Display Driver

[![Crates.io](https://img.shields.io/crates/v/gdep073e01.svg)](https://crates.io/crates/gdep073e01)
[![Documentation](https://docs.rs/gdep073e01/badge.svg)](https://docs.rs/gdep073e01)
[![Build Status](https://github.com/xandronak/gdep073e01/workflows/CI/badge.svg)](https://github.com/xandronak/gdep073e01/actions)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://github.com/xandronak/gdep073e01)

A robust, `no_std` embedded Rust driver for the **Good Display GDEP073E01** 7.5-inch, 7-color e-paper display. This driver provides full integration with the [`embedded-graphics`](https://github.com/embedded-graphics/embedded-graphics) ecosystem, enabling rich graphical applications on resource-constrained embedded systems.

## ✨ Features

- 🎨 **Full 7-color support**: Black, White, Yellow, Red, Orange, Blue, Green
- 🖼️ **Complete embedded-graphics integration**: Implements `DrawTarget` and `OriginDimensions`
- 📏 **High resolution**: 800×480 pixels (384,000 pixels total)
- ⚡ **Power management**: Deep sleep mode for ultra-low power consumption
- 🔧 **HAL agnostic**: Compatible with any `embedded-hal` 1.0 implementation
- 🚀 **Optimized performance**: Efficient buffering and SPI communication
- 🛡️ **Robust error handling**: Comprehensive error types and timeout protection
- 📚 **Well documented**: Extensive documentation and examples

## 🚀 Quick Start

### Installation

Add to your `Cargo.toml`:

```
[dependencies]
gdep073e01 = "0.1"
embedded-graphics = "0.8"
```

### Basic Usage

```
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Circle, PrimitiveStyle, Rectangle},
    text::{Baseline, Text},
};
use gdep073e01::prelude::*;

// Initialize your platform's HAL components
let spi = /* your SPI implementation */;
let cs_pin = /* chip select pin */;
let dc_pin = /* data/command pin */;
let rst_pin = /* reset pin */;
let busy_pin = /* busy pin */;
let delay = /* delay implementation */;

// Create the display driver
let mut display = Gdep073e01::new(spi, cs_pin, dc_pin, rst_pin, busy_pin, delay);

// Initialize the display
display.init().expect("Failed to initialize display");

// Clear the display with white background
display.clear(Color::White).unwrap();

// Draw a red rectangle
Rectangle::new(Point::new(50, 50), Size::new(200, 100))
    .into_styled(PrimitiveStyle::with_fill(Color::Red))
    .draw(&mut display)
    .unwrap();

// Draw a blue circle
Circle::new(Point::new(400, 200), 80)
    .into_styled(PrimitiveStyle::with_fill(Color::Blue))
    .draw(&mut display)
    .unwrap();

// Add some text
let text_style = MonoTextStyle::new(&FONT_6X10, Color::Black);
Text::with_baseline("Hello, E-Paper!", Point::new(100, 300), text_style, Baseline::Top)
    .draw(&mut display)
    .unwrap();

// Update the display (this will take ~15-20 seconds)
display.flush().expect("Failed to update display");

// Put the display to sleep to save power
display.sleep().expect("Failed to enter sleep mode");
```

## 🔌 Hardware Setup

### Pin Connections

Connect your GDEP073E01 to your microcontroller as follows:

| Display Pin | MCU Pin | Function | Description |
|------------|---------|----------|-------------|
| **VCC** | 3.3V | Power | 3.3V power supply (⚠️ **NOT** 5V tolerant) |
| **GND** | GND | Ground | Ground reference |
| **DIN** | SPI MOSI | Data In | SPI Master Out, Slave In |
| **CLK** | SPI SCK | Clock | SPI Serial Clock |
| **CS** | GPIO | Chip Select | SPI Chip Select (active low) |
| **DC** | GPIO | Data/Command | Data/Command selection |
| **RST** | GPIO | Reset | Hardware reset (active low) |
| **BUSY** | GPIO | Busy Status | Display busy indicator |

### Wiring Example (STM32)

```
// Example for STM32F4xx with stm32f4xx-hal
use stm32f4xx_hal::{
    gpio::{GpioExt, Speed},
    pac,
    prelude::*,
    spi::{NoMiso, Spi},
    timer::Timer,
};

let dp = pac::Peripherals::take().unwrap();
let cp = cortex_m::peripheral::Peripherals::take().unwrap();

let gpioa = dp.GPIOA.split();
let gpiob = dp.GPIOB.split();

// SPI1 pins: SCK=PA5, MOSI=PA7
let sck = gpioa.pa5.into_alternate();
let mosi = gpioa.pa7.into_alternate();

// Control pins
let cs = gpiob.pb0.into_push_pull_output().speed(Speed::High);
let dc = gpiob.pb1.into_push_pull_output().speed(Speed::High);
let rst = gpiob.pb2.into_push_pull_output().speed(Speed::High);
let busy = gpiob.pb3.into_pull_down_input();

// Initialize SPI
let spi = Spi::new(
    dp.SPI1,
    (sck, NoMiso, mosi),
    embedded_hal::spi::MODE_0,
    2.MHz(),
    &clocks,
);

// Create delay
let mut delay = Timer::new(dp.TIM2, &clocks).delay();

let mut display = Gdep073e01::new(spi, cs, dc, rst, busy, delay);
```

## 🎨 Color Palette

The GDEP073E01 supports 7 distinct colors:

| Color | Value | Hex | Preview |
|-------|-------|-----|---------|
| **Black** | `Color::Black` | `0x00` | ⬛ |
| **White** | `Color::White` | `0x01` | ⬜ |
| **Yellow** | `Color::Yellow` | `0x02` | 🟨 |
| **Red** | `Color::Red` | `0x03` | 🟥 |
| **Orange** | `Color::Orange` | `0x04` | 🟧 |
| **Blue** | `Color::Blue` | `0x05` | 🟦 |
| **Green** | `Color::Green` | `0x06` | 🟢 |

```
use gdep073e01::Color;

// Using colors in your code
let red_style = PrimitiveStyle::with_fill(Color::Red);
let blue_stroke = PrimitiveStyle::with_stroke(Color::Blue, 3);
```

## 🚄 Performance Characteristics

### Display Specifications
- **Resolution**: 800 × 480 pixels
- **Display Area**: 163.2 × 97.92 mm
- **Pixel Density**: ~125 PPI
- **Colors**: 7 colors (4-bit per pixel)
- **Memory Usage**: 192KB frame buffer

### Timing
- **Full Refresh**: ~15-20 seconds
- **Initialization**: ~2-3 seconds
- **Sleep Entry**: <100ms
- **Wake-up**: Requires full re-initialization

### Power Consumption
- **Active (refreshing)**: ~40-50mA @ 3.3V
- **Idle (after refresh)**: ~1-2mA @ 3.3V
- **Deep Sleep**: <1µA @ 3.3V

## 📚 Examples

### Drawing Primitives

```
use embedded_graphics::primitives::*;

// Filled rectangle
Rectangle::new(Point::new(10, 10), Size::new(100, 50))
    .into_styled(PrimitiveStyle::with_fill(Color::Red))
    .draw(&mut display)?;

// Stroked circle
Circle::new(Point::new(200, 100), 60)
    .into_styled(PrimitiveStyle::with_stroke(Color::Blue, 3))
    .draw(&mut display)?;

// Triangle
Triangle::new(
    Point::new(300, 50),
    Point::new(250, 150),
    Point::new(350, 150)
)
.into_styled(PrimitiveStyle::with_fill(Color::Green))
.draw(&mut display)?;
```

### Text Rendering

```
use embedded_graphics::{
    mono_font::{ascii::FONT_9X18_BOLD, MonoTextStyle},
    text::{Alignment, Text},
};

let large_text = MonoTextStyle::new(&FONT_9X18_BOLD, Color::Black);

Text::with_alignment(
    "E-Paper Display",
    Point::new(400, 100),
    large_text,
    Alignment::Center,
)
.draw(&mut display)?;
```

### Image Display

```
use embedded_graphics::image::Image;
use tinybmp::Bmp;

// Load BMP image from embedded data
let bmp = Bmp::from_slice(include_bytes!("../assets/logo.bmp")).unwrap();
let image = Image::new(&bmp, Point::new(50, 50));

image.draw(&mut display)?;
```

## 🛠️ Platform Support

### Requirements

Your platform must provide:
- SPI peripheral (`embedded-hal::spi::SpiDevice`)
- GPIO output pins (`embedded-hal::digital::OutputPin`)
- GPIO input pin (`embedded-hal::digital::InputPin`)
- Delay/timer (`embedded-hal::delay::DelayNs`)

## 🐛 Troubleshooting

### Common Issues

#### Display doesn't initialize
```
Error: Timeout waiting for display ready
```
**Solution**: Check BUSY pin connection and ensure proper pull-down resistor.

#### Garbled or no display output
- Verify SPI connections (CLK, MOSI, CS)
- Check power supply is stable 3.3V
- Ensure DC pin is correctly connected
- Verify SPI mode is set to MODE_0

#### Very slow updates
- This is normal! E-paper displays take 15-20 seconds for full refresh
- Ensure you're calling `flush()` only after all drawing operations

#### High power consumption
- Call `display.sleep()` after updating the display
- Verify the display enters deep sleep mode (BUSY pin should be low)

## 📖 API Documentation

### Core Methods

| Method | Description | Duration |
|--------|-------------|----------|
| `new()` | Create driver instance | Instant |
| `init()` | Initialize display | ~2-3s |
| `flush()` | Update display | ~15-20s |
| `sleep()` | Enter deep sleep | ~100ms |
| `clear()` | Clear buffer | Instant |
| `set_pixel()` | Set individual pixel | Instant |

### Error Handling

```
use gdep073e01::Error;

match display.init() {
    Ok(()) => println!("Display initialized successfully"),
    Err(Error::Spi(e)) => println!("SPI error: {:?}", e),
    Err(Error::Pin(e)) => println!("GPIO error: {:?}", e),
    Err(Error::Timeout) => println!("Display timeout - check connections"),
}
```

## 🤝 Contributing

Contributions are welcome! Here's how you can help:

1. **Report bugs** by opening an issue
2. **Suggest features** for enhancement
3. **Submit pull requests** with improvements
4. **Test on new platforms** and share results
5. **Improve documentation** and examples

### Development Setup

```
git clone https://github.com/yourusername/gdep073e01.git
cd gdep073e01
cargo test
cargo doc --open
```

### Testing

```
# Run all tests
cargo test

# Test with specific features
cargo test --features debug

# Run tests on specific target
cargo test --target thumbv7em-none-eabihf
```

## 📄 License

Licensed under either of:

- **Apache License, Version 2.0** ([apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))
- **MIT License** ([opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

---

**Made with ❤️ for the embedded Rust community**
```