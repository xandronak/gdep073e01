//! Embedded-graphics driver for the GDEP073E01 7-color e-paper display.
//!
//! This driver provides an interface to the Good Display GDEP073E01 e-paper display,
//! implementing the `embedded-graphics` traits `DrawTarget` and `OriginDimensions`.
//!
//! ## Features
//! - 7-color display support (Black, White, Yellow, Red, Orange, Blue, Green)
//! - Full `embedded-graphics` integration with `DrawTarget` trait
//! - Hardware abstraction layer (HAL) compatibility
//! - Power management with sleep modes
//! - Efficient internal buffering
//!
//! ## Usage
//!
//! ```
//! use embedded_graphics::prelude::*;
//! use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
//! use gdep073e01::{Gdep073e01, Color};
//! use core::convert::Infallible;
//! # use embedded_hal::spi::SpiDevice;
//! # use embedded_hal::digital::{OutputPin, InputPin};
//! # use embedded_hal::delay::DelayNs;
//! # struct MockSpi; struct MockPin; struct MockDelay;
//! # impl embedded_hal::spi::ErrorType for MockSpi { type Error = Infallible; }
//! # impl SpiDevice<u8> for MockSpi { fn transaction(&mut self, _: &mut [embedded_hal::spi::Operation<'_, u8>]) -> Result<(), Self::Error> { Ok(()) } }
//! # impl embedded_hal::digital::ErrorType for MockPin { type Error = Infallible; }
//! # impl OutputPin for MockPin { fn set_low(&mut self) -> Result<(), Self::Error> { Ok(()) } fn set_high(&mut self) -> Result<(), Self::Error> { Ok(()) } }
//! # impl InputPin for MockPin { fn is_high(&mut self) -> Result<bool, Self::Error> { Ok(false) } fn is_low(&mut self) -> Result<bool, Self::Error> { Ok(true) } }
//! # impl DelayNs for MockDelay { fn delay_ns(&mut self, _: u32) {} }
//! # let spi = MockSpi; let cs = MockPin; let dc = MockPin; let rst = MockPin; let busy = MockPin; let delay = MockDelay;
//!
//! let mut display = Gdep073e01::new(spi, cs, dc, rst, busy, delay);
//!
//! display.init().expect("Failed to initialize display");
//!
//! let style = PrimitiveStyle::with_fill(Color::Red);
//! Rectangle::new(Point::new(10, 10), Size::new(50, 50))
//!     .into_styled(style)
//!     .draw(&mut display)
//!     .expect("Failed to draw rectangle");
//!
//! display.flush().expect("Failed to update display");
//! display.sleep().expect("Failed to enter sleep mode");
//! ```

#![no_std]
#![warn(missing_docs)]

extern crate alloc;

use alloc::{boxed::Box, vec};
use core::marker::PhantomData;

use embedded_graphics::{
    pixelcolor::{raw::RawU4, PixelColor},
    prelude::*,
    primitives::Rectangle,
};
use embedded_hal::{
    delay::DelayNs,
    digital::{InputPin, OutputPin},
    spi::SpiDevice,
};

/// Display width in pixels
pub const WIDTH: u32 = 800;
/// Display height in pixels
pub const HEIGHT: u32 = 480;

const BUFFER_SIZE: usize = (WIDTH * HEIGHT / 2) as usize;

// Display command constants
const CMD_PANEL_SETTING: u8 = 0x00;
const CMD_POWER_SETTING: u8 = 0x01;
const CMD_POWER_OFF: u8 = 0x02;
const CMD_POFS: u8 = 0x03;
const CMD_POWER_ON: u8 = 0x04;
const CMD_BOOSTER_SOFT_START1: u8 = 0x05;
const CMD_BOOSTER_SOFT_START2: u8 = 0x06;
const CMD_DEEP_SLEEP: u8 = 0x07;
const CMD_BOOSTER_SOFT_START3: u8 = 0x08;
const CMD_DATA_START_TRANSMISSION: u8 = 0x10;
const CMD_DISPLAY_REFRESH: u8 = 0x12;
const CMD_PLL_CONTROL: u8 = 0x30;
const CMD_CDI: u8 = 0x50;
const CMD_TCON_SETTING: u8 = 0x60;
const CMD_TRES: u8 = 0x61;
const CMD_T_VDCS: u8 = 0x84;
const CMD_PWS: u8 = 0xE3;
const CMD_CMDH: u8 = 0xAA;

// Timing constants
const RESET_DELAY_MS: u32 = 10;
const BUSY_WAIT_DELAY_MS: u32 = 10;
const BUSY_TIMEOUT_MS: u32 = 30_000;

/// GDEP073E01 color variants.
///
/// Each color is represented by a 4-bit value that corresponds to the
/// display's internal color mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
#[derive(Default)]
pub enum Color {
    /// Black color (0x00)
    Black = 0x00,
    /// White color (0x01)
    #[default]
    White = 0x01,
    /// Yellow color (0x02)
    Yellow = 0x02,
    /// Red color (0x03)
    Red = 0x03,
    /// Orange color (0x04)
    Orange = 0x04,
    /// Blue color (0x05)
    Blue = 0x05,
    /// Green color (0x06)
    Green = 0x06,
}

impl PixelColor for Color {
    type Raw = RawU4;
}

/// GDEP073E01 display driver.
///
/// This driver manages communication with the GDEP073E01 7-color e-paper display
/// via SPI and provides embedded-graphics compatibility.
///
/// # Type Parameters
///
/// - `SPI`: SPI device implementing `SpiDevice<u8>`
/// - `CS`: Chip select pin (active low)
/// - `DC`: Data/command pin (high for data, low for command)
/// - `RST`: Reset pin (active low)
/// - `BUSY`: Busy indicator pin (high when display is busy)
/// - `DELAY`: Delay provider implementing `DelayNs`
pub struct Gdep073e01<SPI, CS, DC, RST, BUSY, DELAY> {
    spi: SPI,
    cs: CS,
    dc: DC,
    rst: RST,
    busy: BUSY,
    delay: DELAY,
    buffer: Box<[u8]>,
    _phantom: PhantomData<Color>,
}

/// Error types for the GDEP073E01 driver.
#[derive(Debug)]
pub enum Error<SpiE, PinE> {
    /// SPI communication error
    Spi(SpiE),
    /// GPIO pin operation error
    Pin(PinE),
    /// Timeout waiting for display ready
    Timeout,
}

impl<SPI, CS, DC, RST, BUSY, DELAY, SpiE, PinE> Gdep073e01<SPI, CS, DC, RST, BUSY, DELAY>
where
    SPI: SpiDevice<u8, Error = SpiE>,
    CS: OutputPin<Error = PinE>,
    DC: OutputPin<Error = PinE>,
    RST: OutputPin<Error = PinE>,
    BUSY: InputPin<Error = PinE>,
    DELAY: DelayNs,
{
    /// Creates a new GDEP073E01 driver instance.
    ///
    /// # Arguments
    ///
    /// * `spi` - SPI device for communication
    /// * `cs` - Chip select pin (active low)
    /// * `dc` - Data/command selection pin
    /// * `rst` - Reset pin (active low)
    /// * `busy` - Busy status pin
    /// * `delay` - Delay provider
    ///
    /// # Returns
    ///
    /// A new driver instance with an initialized buffer.
    pub fn new(spi: SPI, cs: CS, dc: DC, rst: RST, busy: BUSY, delay: DELAY) -> Self {
        let buffer = vec![0x11; BUFFER_SIZE].into_boxed_slice(); // Default to white

        Self {
            spi,
            cs,
            dc,
            rst,
            busy,
            delay,
            buffer,
            _phantom: PhantomData,
        }
    }

    /// Initializes the display.
    ///
    /// Performs hardware reset and sends the initialization sequence required
    /// for proper display operation. This must be called before any drawing operations.
    ///
    /// # Errors
    ///
    /// Returns `Error::Spi` for SPI communication failures, `Error::Pin` for GPIO
    /// errors, or `Error::Timeout` if the display doesn't respond within the timeout period.
    pub fn init(&mut self) -> Result<(), Error<SpiE, PinE>> {
        self.reset()?;
        self.send_init_sequence()?;
        self.power_on()
    }

    /// Puts the display into deep sleep mode.
    ///
    /// This significantly reduces power consumption. The display requires
    /// reinitialization via `init()` to wake up from deep sleep.
    ///
    /// # Errors
    ///
    /// Returns errors for communication failures or timeout.
    pub fn sleep(&mut self) -> Result<(), Error<SpiE, PinE>> {
        self.power_off()?;
        self.command_with_data(CMD_DEEP_SLEEP, &[0xA5])
    }

    /// Updates the display with the current buffer contents.
    ///
    /// Sends the internal buffer to the display and triggers a refresh.
    /// This operation may take several seconds to complete.
    ///
    /// # Errors
    ///
    /// Returns errors for communication failures or timeout.
    pub fn flush(&mut self) -> Result<(), Error<SpiE, PinE>> {
        self.write_command(CMD_DATA_START_TRANSMISSION)?;
        self.write_buffer_data()?;
        self.refresh()
    }

    /// Clears the internal buffer with the specified color.
    ///
    /// Note: This only affects the internal buffer. Call `flush()` to update the display.
    ///
    /// # Arguments
    ///
    /// * `color` - The color to fill the buffer with
    pub fn clear_buffer(&mut self, color: Color) {
        let color_val = color as u8;
        let packed_color = (color_val << 4) | color_val;
        self.buffer.fill(packed_color);
    }

    /// Sets a pixel in the internal buffer.
    ///
    /// Note: This only affects the internal buffer. Call `flush()` to update the display.
    ///
    /// # Arguments
    ///
    /// * `x` - X coordinate (0 to WIDTH-1)
    /// * `y` - Y coordinate (0 to HEIGHT-1)
    /// * `color` - Pixel color
    pub fn set_pixel(&mut self, x: u32, y: u32, color: Color) {
        if x >= WIDTH || y >= HEIGHT {
            return;
        }

        let index = (y * WIDTH + x) as usize / 2;
        let color_val = color as u8;
        let mut byte = self.buffer[index];

        if x % 2 == 0 {
            byte = (byte & 0x0F) | (color_val << 4);
        } else {
            byte = (byte & 0xF0) | color_val;
        }

        self.buffer[index] = byte;
    }

    fn reset(&mut self) -> Result<(), Error<SpiE, PinE>> {
        self.rst.set_low().map_err(Error::Pin)?;
        self.delay.delay_ms(RESET_DELAY_MS);
        self.rst.set_high().map_err(Error::Pin)?;
        self.delay.delay_ms(RESET_DELAY_MS);
        Ok(())
    }

    fn send_init_sequence(&mut self) -> Result<(), Error<SpiE, PinE>> {
        self.command_with_data(CMD_CMDH, &[0x49, 0x55, 0x20, 0x08, 0x09, 0x18])?;
        self.command_with_data(CMD_POWER_SETTING, &[0x3F])?;
        self.command_with_data(CMD_PANEL_SETTING, &[0x5F, 0x69])?;
        self.command_with_data(CMD_POFS, &[0x00, 0x54, 0x00, 0x44])?;
        self.command_with_data(CMD_BOOSTER_SOFT_START1, &[0x40, 0x1F, 0x1F, 0x2C])?;
        self.command_with_data(CMD_BOOSTER_SOFT_START2, &[0x6F, 0x1F, 0x17, 0x49])?;
        self.command_with_data(CMD_BOOSTER_SOFT_START3, &[0x6F, 0x1F, 0x1F, 0x22])?;
        self.command_with_data(CMD_PLL_CONTROL, &[0x08])?;
        self.command_with_data(CMD_CDI, &[0x3F])?;
        self.command_with_data(CMD_TCON_SETTING, &[0x02, 0x00])?;
        self.command_with_data(CMD_TRES, &[0x03, 0x20, 0x01, 0xE0])?; // 800x480
        self.command_with_data(CMD_T_VDCS, &[0x01])?;
        self.command_with_data(CMD_PWS, &[0x2F])
    }

    fn write_command(&mut self, command: u8) -> Result<(), Error<SpiE, PinE>> {
        self.dc.set_low().map_err(Error::Pin)?;
        self.cs.set_low().map_err(Error::Pin)?;
        let result = self.spi.write(&[command]).map_err(Error::Spi);
        self.cs.set_high().map_err(Error::Pin)?;
        result
    }

    fn write_data(&mut self, data: &[u8]) -> Result<(), Error<SpiE, PinE>> {
        self.dc.set_high().map_err(Error::Pin)?;
        self.cs.set_low().map_err(Error::Pin)?;
        let result = self.spi.write(data).map_err(Error::Spi);
        self.cs.set_high().map_err(Error::Pin)?;
        result
    }

    fn command_with_data(&mut self, command: u8, data: &[u8]) -> Result<(), Error<SpiE, PinE>> {
        self.write_command(command)?;
        self.write_data(data)
    }

    fn write_buffer_data(&mut self) -> Result<(), Error<SpiE, PinE>> {
        self.dc.set_high().map_err(Error::Pin)?;
        self.cs.set_low().map_err(Error::Pin)?;

        const CHUNK_SIZE: usize = 4096;
        let mut result = Ok(());

        for chunk in self.buffer.chunks(CHUNK_SIZE) {
            if let Err(e) = self.spi.write(chunk).map_err(Error::Spi) {
                result = Err(e);
                break;
            }
        }

        self.cs.set_high().map_err(Error::Pin)?;
        result
    }

    fn wait_until_idle(&mut self) -> Result<(), Error<SpiE, PinE>> {
        let mut remaining_delay = BUSY_TIMEOUT_MS;

        while self.busy.is_high().map_err(Error::Pin)? {
            if remaining_delay == 0 {
                return Err(Error::Timeout);
            }
            let delay_step = remaining_delay.min(BUSY_WAIT_DELAY_MS);
            self.delay.delay_ms(delay_step);
            remaining_delay = remaining_delay.saturating_sub(delay_step);
        }

        Ok(())
    }

    fn power_on(&mut self) -> Result<(), Error<SpiE, PinE>> {
        self.write_command(CMD_POWER_ON)?;
        self.wait_until_idle()
    }

    fn power_off(&mut self) -> Result<(), Error<SpiE, PinE>> {
        self.command_with_data(CMD_POWER_OFF, &[0x00])?;
        self.wait_until_idle()
    }

    fn refresh(&mut self) -> Result<(), Error<SpiE, PinE>> {
        self.command_with_data(CMD_DISPLAY_REFRESH, &[0x00])?;
        self.wait_until_idle()
    }
}

impl<SPI, CS, DC, RST, BUSY, DELAY, SpiE, PinE> DrawTarget
    for Gdep073e01<SPI, CS, DC, RST, BUSY, DELAY>
where
    SPI: SpiDevice<u8, Error = SpiE>,
    CS: OutputPin<Error = PinE>,
    DC: OutputPin<Error = PinE>,
    RST: OutputPin<Error = PinE>,
    BUSY: InputPin<Error = PinE>,
    DELAY: DelayNs,
{
    type Color = Color;
    type Error = Error<SpiE, PinE>;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels {
            if let Ok((x, y)) = coord.try_into() {
                self.set_pixel(x, y, color);
            }
        }
        Ok(())
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let area = area.intersection(&self.bounding_box());
        if area.is_zero_sized() {
            return Ok(());
        }

        let start_x = area.top_left.x as u32;
        let start_y = area.top_left.y as u32;
        let end_x = (area.top_left.x + area.size.width as i32) as u32;
        let end_y = (area.top_left.y + area.size.height as i32) as u32;

        for y in start_y..end_y {
            for x in start_x..end_x {
                self.set_pixel(x, y, color);
            }
        }

        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.clear_buffer(color);
        Ok(())
    }
}

impl<SPI, CS, DC, RST, BUSY, DELAY> OriginDimensions for Gdep073e01<SPI, CS, DC, RST, BUSY, DELAY> {
    fn size(&self) -> Size {
        Size::new(WIDTH, HEIGHT)
    }
}

/// Prelude module for convenient imports.
pub mod prelude {
    pub use crate::{Color, Error, Gdep073e01, HEIGHT, WIDTH};
    pub use embedded_graphics::prelude::*;
    pub use embedded_hal::{
        delay::DelayNs,
        digital::{ErrorType as DigitalErrorType, InputPin, OutputPin},
        spi::{ErrorType as SpiErrorType, SpiDevice},
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use embedded_hal::digital::{ErrorType as DigitalErrorType, PinState};
    use embedded_hal::spi::{ErrorType as SpiErrorType, Operation};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct MockError;

    impl embedded_hal::digital::Error for MockError {
        fn kind(&self) -> embedded_hal::digital::ErrorKind {
            embedded_hal::digital::ErrorKind::Other
        }
    }

    impl embedded_hal::spi::Error for MockError {
        fn kind(&self) -> embedded_hal::spi::ErrorKind {
            embedded_hal::spi::ErrorKind::Other
        }
    }

    #[derive(Debug, Default)]
    struct MockSpi {
        pub writes: Vec<Vec<u8>>,
    }

    impl SpiErrorType for MockSpi {
        type Error = MockError;
    }

    impl SpiDevice<u8> for MockSpi {
        fn transaction(&mut self, operations: &mut [Operation<u8>]) -> Result<(), Self::Error> {
            for op in operations {
                if let Operation::Write(data) = op {
                    self.writes.push(data.to_vec());
                }
            }
            Ok(())
        }

        fn write(&mut self, words: &[u8]) -> Result<(), Self::Error> {
            self.writes.push(words.to_vec());
            Ok(())
        }
    }

    #[derive(Debug, Default)]
    struct MockPin {
        pub states: Vec<PinState>,
    }

    impl DigitalErrorType for MockPin {
        type Error = MockError;
    }

    impl OutputPin for MockPin {
        fn set_low(&mut self) -> Result<(), Self::Error> {
            self.states.push(PinState::Low);
            Ok(())
        }

        fn set_high(&mut self) -> Result<(), Self::Error> {
            self.states.push(PinState::High);
            Ok(())
        }
    }

    impl InputPin for MockPin {
        fn is_high(&mut self) -> Result<bool, Self::Error> {
            Ok(false) // Always return not busy for tests
        }

        fn is_low(&mut self) -> Result<bool, Self::Error> {
            Ok(true)
        }
    }

    #[derive(Debug, Default)]
    struct MockDelay;

    impl DelayNs for MockDelay {
        fn delay_ns(&mut self, _ns: u32) {}
    }

    #[test]
    fn test_set_pixel() {
        let spi = MockSpi::default();
        let cs = MockPin::default();
        let dc = MockPin::default();
        let rst = MockPin::default();
        let busy = MockPin::default();
        let delay = MockDelay;

        let mut display = Gdep073e01::new(spi, cs, dc, rst, busy, delay);

        display.set_pixel(0, 0, Color::Black);
        assert_eq!(display.buffer[0], 0x01);

        display.set_pixel(1, 0, Color::Red);
        assert_eq!(display.buffer[0], 0x03);
    }

    #[test]
    fn test_clear_buffer() {
        let spi = MockSpi::default();
        let cs = MockPin::default();
        let dc = MockPin::default();
        let rst = MockPin::default();
        let busy = MockPin::default();
        let delay = MockDelay;

        let mut display = Gdep073e01::new(spi, cs, dc, rst, busy, delay);

        display.clear_buffer(Color::Orange);
        assert!(display.buffer.iter().all(|&byte| byte == 0x44));
    }

    #[test]
    fn test_display_dimensions() {
        let spi = MockSpi::default();
        let cs = MockPin::default();
        let dc = MockPin::default();
        let rst = MockPin::default();
        let busy = MockPin::default();
        let delay = MockDelay;

        let display = Gdep073e01::new(spi, cs, dc, rst, busy, delay);
        assert_eq!(display.size(), Size::new(WIDTH, HEIGHT));
    }
}
