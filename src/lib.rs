//! Embedded-graphics driver for the GDEP073E01 7-color e-paper display.
//!
//! This driver provides an interface to the Good Display GDEP073E01 e-paper display,
//! implementing the `embedded-graphics` traits `DrawTarget` and `OriginDimensions`.
//!
//! ## Features
//! - 7-color display (Black, White, Yellow, Red, Orange, Blue, Green)
//! - `DrawTarget` implementation for drawing with `embedded-graphics`.
//! - `OriginDimensions` implementation for querying display size.
//! - Power control methods (`init`, `sleep`).
//! - Internal buffer for drawing operations.
//!
//! ## Usage
//! 
//! 1. Instantiate your platform's HAL implementations for SPI, OutputPins (CS, DC, RST),
//!    InputPin (BUSY), and a Delay provider.
//! 2. Create the driver instance using [`Gdep073e01::new`].
//! 3. Initialize the display with [`Gdep073e01::init`].
//! 4. Use `embedded-graphics` drawing functions (e.g., `draw`, `fill_solid`, `clear`).
//! 5. Call [`Gdep073e01::flush`] to send the buffer contents to the display.
//! 6. Call [`Gdep073e01::sleep`] to put the display into low-power mode.
//! 
//! ```no_run
//! # #![no_std]
//! # use embedded_graphics::prelude::*;
//! # use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
//! # // Assume mock types for HAL traits exist
//! # use embedded_hal::spi::SpiDevice;
//! # use embedded_hal::digital::{OutputPin, InputPin};
//! # use embedded_hal::delay::DelayNs;
//! # type MockSpi = (); type MockPin = (); type MockDelay = ();
//! # impl SpiDevice<u8> for MockSpi { type Error = (); fn transaction(&mut self, _: &mut [embedded_hal::spi::Operation<'_, u8>]) -> Result<(), Self::Error> { Ok(()) } }
//! # impl OutputPin for MockPin { type Error = (); fn set_low(&mut self) -> Result<(), Self::Error> { Ok(()) } fn set_high(&mut self) -> Result<(), Self::Error> { Ok(()) } }
//! # impl InputPin for MockPin { type Error = (); fn is_high(&mut self) -> Result<bool, Self::Error> { Ok(false) } fn is_low(&mut self) -> Result<bool, Self::Error> { Ok(true) } }
//! # impl DelayNs for MockDelay { fn delay_ns(&mut self, _: u32) {} }
//! # let spi: MockSpi = ();
//! # let mut cs: MockPin = ();
//! # let mut dc: MockPin = ();
//! # let mut rst: MockPin = ();
//! # let mut busy: MockPin = ();
//! # let mut delay: MockDelay = ();
//! use gdep073e01::{Gdep073e01, Color};
//! 
//! // 1. Instantiate HAL components (spi, cs, dc, rst, busy, delay)
//! // ... platform-specific setup ...
//! 
//! // 2. Create driver instance
//! let mut display = Gdep073e01::new(spi, cs, dc, rst, busy, delay);
//! 
//! // 3. Initialize
//! display.init().expect("Init failed");
//! 
//! // 4. Draw something
//! let style = PrimitiveStyle::with_fill(Color::Red);
//! Rectangle::new(Point::new(10, 10), Size::new(50, 50))
//!     .into_styled(style)
//!     .draw(&mut display)
//!     .expect("Drawing failed");
//! 
//! // 5. Flush buffer to display
//! display.flush().expect("Flush failed");
//! 
//! // 6. Put display to sleep
//! display.sleep().expect("Sleep failed");
//! ```
//! 

#![no_std]

extern crate alloc;

use core::marker::PhantomData;
use core::mem; // Import mem for swap

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

// Display dimensions
pub const WIDTH: u32 = 800;
pub const HEIGHT: u32 = 480;
const BUFFER_SIZE: usize = (WIDTH * HEIGHT / 2) as usize;

// Display commands
const CMD_PANEL_SETTING: u8 = 0x00;
const CMD_POWER_SETTING: u8 = 0x01;
const CMD_POWER_OFF: u8 = 0x02;
const CMD_POFS: u8 = 0x03; // Power Off Sequence Setting?
const CMD_POWER_ON: u8 = 0x04;
const CMD_BOOSTER_SOFT_START1: u8 = 0x05;
const CMD_BOOSTER_SOFT_START2: u8 = 0x06;
const CMD_DEEP_SLEEP: u8 = 0x07;
const CMD_BOOSTER_SOFT_START3: u8 = 0x08;
const CMD_DATA_START_TRANSMISSION: u8 = 0x10;
const CMD_DISPLAY_REFRESH: u8 = 0x12;
const CMD_PLL_CONTROL: u8 = 0x30;
const CMD_CDI: u8 = 0x50; // VCOM and data interval setting
const CMD_TCON_SETTING: u8 = 0x60; // Gate/Source Start setting
const CMD_TRES: u8 = 0x61; // Resolution setting
// const CMD_REVISION: u8 = 0x70;
// const CMD_VCM_DC_SETTING: u8 = 0x82;
// const CMD_VDCS: u8 = 0x82; // VCOM_DC Setting (Used in EPD_init_fast, maybe relevant later)
const CMD_T_VDCS: u8 = 0x84; // VCOM_DC Setting for VCOM 1 generation voltage (Used in standard init)
const CMD_PWS: u8 = 0xE3; // Power saving
const CMD_CMDH: u8 = 0xAA; // Command High? (Seems to be a prefix/mode setting)

// Delays (adjust based on target platform and testing)
const RESET_DELAY_MS: u32 = 10; // C++ uses 50, 20, 10
const BUSY_WAIT_DELAY_MS: u32 = 10;
const BUSY_TIMEOUT_MS: u32 = 30_000; // 30 seconds, adjust as needed

/// GDEP073E01 color variants.
/// Each color is represented by a 4-bit value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Color {
    Black = 0x00,  // 0000
    White = 0x01,  // 0001
    Yellow = 0x02, // 0010
    Red = 0x03,    // 0011
    Orange = 0x04, // 0100
    Blue = 0x05,   // 0101
    Green = 0x06,  // 0110
    // Note: 0x07 is often treated as White in some examples, but sticking to 7 distinct colors.
}

impl PixelColor for Color {
    type Raw = RawU4;
}

/// GDEP073E01 display driver.
///
/// Requires an SPI interface, a chip select pin (`CS`), a data/command pin (`DC`),
/// a reset pin (`RST`), a busy indicator pin (`BUSY`), and a delay provider.
pub struct Gdep073e01<SPI, CS, DC, RST, BUSY, DELAY> {
    /// SPI interface.
    spi: SPI,
    /// Chip select pin (active low).
    cs: CS,
    /// Data/command pin (high for data, low for command).
    dc: DC,
    /// Reset pin (active low).
    rst: RST,
    /// Busy indicator pin (high when busy).
    busy: BUSY,
    /// Delay provider.
    delay: DELAY,
    /// Internal frame buffer (1 byte per 2 pixels).
    buffer: [u8; BUFFER_SIZE],
    /// Marker for DrawTarget compatibility
    _phantom: PhantomData<Color>,
}

// Helper macro for mapping pin errors
macro_rules! pin_try {
    ($expr:expr) => {
        $expr.map_err(Error::Pin)?
    };
}

impl<SPI, CS, DC, RST, BUSY, DELAY, SpiE, PinE>
    Gdep073e01<SPI, CS, DC, RST, BUSY, DELAY>
where
    SPI: SpiDevice<u8, Error = SpiE>,
    CS: OutputPin<Error = PinE>,
    DC: OutputPin<Error = PinE>,
    RST: OutputPin<Error = PinE>,
    BUSY: InputPin<Error = PinE>,
    DELAY: DelayNs,
{
    /// Creates a new driver instance.
    pub fn new(
        spi: SPI,
        cs: CS,
        dc: DC,
        rst: RST,
        busy: BUSY,
        delay: DELAY,
    ) -> Self {
        Self {
            spi,
            cs,
            dc,
            rst,
            busy,
            delay,
            buffer: [0x11; BUFFER_SIZE], // Initialize with white
            _phantom: PhantomData,
        }
    }

    /// Initializes the display by performing a hardware reset and sending the
    /// required initialization sequence commands.
    ///
    /// This method should be called before any drawing operations.
    pub fn init(&mut self) -> Result<(), Error<SpiE, PinE>> {
        self.reset()?;
        Self::wait_until_idle(&mut self.busy, &mut self.delay)?;

        // Initialization sequence based on C++ examples (EPD_init / GxEPD2)
        self.command_with_data(CMD_CMDH, &[0x49, 0x55, 0x20, 0x08, 0x09, 0x18])?;
        self.command_with_data(CMD_POWER_SETTING, &[0x3F])?;
        self.command_with_data(CMD_PANEL_SETTING, &[0x5F, 0x69])?;
        // Use POFS (0x03) here as per C++ examples for init
        self.command_with_data(CMD_POFS, &[0x00, 0x54, 0x00, 0x44])?;
        self.command_with_data(CMD_BOOSTER_SOFT_START1, &[0x40, 0x1F, 0x1F, 0x2C])?;
        self.command_with_data(CMD_BOOSTER_SOFT_START2, &[0x6F, 0x1F, 0x17, 0x49])?;
        self.command_with_data(CMD_BOOSTER_SOFT_START3, &[0x6F, 0x1F, 0x1F, 0x22])?;
        self.command_with_data(CMD_PLL_CONTROL, &[0x08])?;
        self.command_with_data(CMD_CDI, &[0x3F])?; // Vcom and data interval setting
        self.command_with_data(CMD_TCON_SETTING, &[0x02, 0x00])?; // Gate/Source Start setting
        self.command_with_data(CMD_TRES, &[0x03, 0x20, 0x01, 0xE0])?; // Resolution 800x480
        // Use T_VDCS (0x84) here as per C++ EPD_init example
        self.command_with_data(CMD_T_VDCS, &[0x01])?;
        self.command_with_data(CMD_PWS, &[0x2F])?; // Power saving

        self.power_on()
    }

    /// Resets the display hardware.
    fn reset(&mut self) -> Result<(), Error<SpiE, PinE>> {
        pin_try!(self.rst.set_high());
        self.delay.delay_ms(RESET_DELAY_MS);
        pin_try!(self.rst.set_low());
        self.delay.delay_ms(RESET_DELAY_MS);
        pin_try!(self.rst.set_high());
        self.delay.delay_ms(RESET_DELAY_MS);
        Ok(())
    }

    /// Sends a command to the display.
    fn write_command(&mut self, command: u8) -> Result<(), Error<SpiE, PinE>> {
        pin_try!(self.dc.set_low());
        pin_try!(self.cs.set_low());
        self.spi.write(&[command]).map_err(Error::Spi)?;
        pin_try!(self.cs.set_high());
        Ok(())
    }

    /// Sends data to the display.
    fn write_data(&mut self, data: &[u8]) -> Result<(), Error<SpiE, PinE>> {
        pin_try!(self.dc.set_high());
        pin_try!(self.cs.set_low());
        self.spi.write(data).map_err(Error::Spi)?;
        pin_try!(self.cs.set_high());
        Ok(())
    }

    /// Sends a command followed by data.
    fn command_with_data(&mut self, command: u8, data: &[u8]) -> Result<(), Error<SpiE, PinE>> {
        self.write_command(command)?;
        self.write_data(data)
    }

    /// Waits until the BUSY pin is low, with a timeout.
    /// Takes BUSY pin and DELAY provider directly to avoid borrow conflicts.
    fn wait_until_idle(busy: &mut BUSY, delay: &mut DELAY) -> Result<(), Error<SpiE, PinE>>
    where
        BUSY: InputPin<Error = PinE>,
        DELAY: DelayNs,
    {
        let mut remaining_delay = BUSY_TIMEOUT_MS;
        while busy.is_high().map_err(Error::Pin)? {
            if remaining_delay == 0 {
                return Err(Error::Timeout);
            }
            let delay_step = remaining_delay.min(BUSY_WAIT_DELAY_MS);
            delay.delay_ms(delay_step);
            remaining_delay -= delay_step;
        }
        Ok(())
    }

    /// Powers on the display panel and booster.
    fn power_on(&mut self) -> Result<(), Error<SpiE, PinE>> {
        self.write_command(CMD_POWER_ON)?;
        Self::wait_until_idle(&mut self.busy, &mut self.delay)?;
        Ok(())
    }

    /// Powers off the display panel and booster.
    fn power_off(&mut self) -> Result<(), Error<SpiE, PinE>> {
        self.command_with_data(CMD_POWER_OFF, &[0x00])?;
        Self::wait_until_idle(&mut self.busy, &mut self.delay)?;
        Ok(())
    }

    /// Puts the display into deep sleep mode to minimize power consumption.
    ///
    /// Requires a hardware reset via [`init()`] to wake up.
    pub fn sleep(&mut self) -> Result<(), Error<SpiE, PinE>> {
        self.power_off()?; // Sends 0x02 + 0x00 + waits
        self.command_with_data(CMD_DEEP_SLEEP, &[0xA5])?; // Sends 0x07 + 0xA5
        Ok(())
    }

    /// Updates the display with the content of the internal buffer.
    pub fn flush(&mut self) -> Result<(), Error<SpiE, PinE>> {
        let mut buffer_temp = [0x11; BUFFER_SIZE]; // Default value (white)
        mem::swap(&mut self.buffer, &mut buffer_temp);

        self.write_command(CMD_DATA_START_TRANSMISSION)?;
        let result_data = self.write_data(&buffer_temp);

        mem::swap(&mut self.buffer, &mut buffer_temp);

        result_data?; // Propagate error if write_data failed

        self.refresh()
    }

    /// Refreshes the display.
    fn refresh(&mut self) -> Result<(), Error<SpiE, PinE>> {
        self.command_with_data(CMD_DISPLAY_REFRESH, &[0x00])?;
        Self::wait_until_idle(&mut self.busy, &mut self.delay)?;
        Ok(())
    }

    /// Clears the internal buffer with the specified color.
    /// Call `flush()` afterwards to update the display.
    pub fn clear_buffer(&mut self, color: Color) {
        let color_val = color as u8;
        let packed_color = (color_val << 4) | color_val;
        self.buffer.fill(packed_color);
    }

    /// Sets a pixel in the internal buffer.
    /// Call `flush()` afterwards to update the display.
    pub fn set_pixel(&mut self, x: u32, y: u32, color: Color) {
        if x >= WIDTH || y >= HEIGHT {
            // Out of bounds
            return;
        }
        let index = (y * WIDTH + x) as usize / 2;
        let color_val = color as u8;

        // Read the existing byte
        let mut byte = self.buffer[index];

        // Modify the correct nibble
        if x % 2 == 0 {
            // Even column, modify high nibble
            byte = (byte & 0x0F) | (color_val << 4);
        } else {
            // Odd column, modify low nibble
            byte = (byte & 0xF0) | color_val;
        }

        // Write the modified byte back
        self.buffer[index] = byte;
    }
}

/// Error type for the driver.
#[derive(Debug)]
pub enum Error<SpiE, PinE> {
    /// SPI communication error.
    Spi(SpiE),
    /// GPIO pin error.
    Pin(PinE),
    /// Timeout waiting for BUSY pin.
    Timeout,
}

impl<SPI, CS, DC, RST, BUSY, DELAY, SpiE, PinE>
    DrawTarget for Gdep073e01<SPI, CS, DC, RST, BUSY, DELAY>
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
        for Pixel(coord, color) in pixels.into_iter() {
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
        // Note: clear() in embedded-graphics usually doesn't flush automatically.
        // The user should call flush() after drawing or clearing.
    }
}

impl<SPI, CS, DC, RST, BUSY, DELAY> OriginDimensions
    for Gdep073e01<SPI, CS, DC, RST, BUSY, DELAY>
{
    fn size(&self) -> Size {
        Size::new(WIDTH, HEIGHT)
    }
}

/// Prelude module for easy importing of common traits and types.
pub mod prelude {
    pub use embedded_graphics::prelude::*;
    pub use embedded_hal::{
        delay::DelayNs,
        digital::{InputPin, OutputPin},
        spi::SpiDevice,
    };
    pub use super::{Color, Error, Gdep073e01, HEIGHT, WIDTH};
}

// --- Tests --- 
#[cfg(test)]
mod tests {
    extern crate alloc; // Use alloc crate for Vec in no_std test context
    use alloc::vec;
    use alloc::vec::Vec;

    use super::prelude::*; // Use prelude for driver types
    use super::*;
    // use embedded_graphics::pixelcolor::RgbColor; // Unused currently
    use embedded_hal::delay::DelayNs;
    use embedded_hal::digital::{ErrorType as DigitalErrorType, InputPin, OutputPin, PinState};
    use embedded_hal::spi::{ErrorType as SpiErrorType, Operation, /* SpiBus, */ SpiDevice}; // SpiBus unused

    // --- Mock HAL Implementations --- 

    // Define simple, cloneable error types for mocks
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct MockPinError;
    impl embedded_hal::digital::Error for MockPinError {
        fn kind(&self) -> embedded_hal::digital::ErrorKind {
            embedded_hal::digital::ErrorKind::Other
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct MockSpiError;
    impl embedded_hal::spi::Error for MockSpiError {
        fn kind(&self) -> embedded_hal::spi::ErrorKind {
            embedded_hal::spi::ErrorKind::Other
        }
    }

    // Mock implementations for individual pins (more robust)
    #[derive(Debug, Default)]
    struct MockOutputPin { pub actions: Vec<PinState>, name: &'static str }
    impl DigitalErrorType for MockOutputPin {
        type Error = MockPinError;
    }
    impl OutputPin for MockOutputPin {
        fn set_low(&mut self) -> Result<(), Self::Error> { self.actions.push(PinState::Low); Ok(()) }
        fn set_high(&mut self) -> Result<(), Self::Error> { self.actions.push(PinState::High); Ok(()) }
    }

    #[derive(Debug, Default)]
    struct MockInputPin { pub states: Vec<PinState>, name: &'static str }
    impl DigitalErrorType for MockInputPin {
        type Error = MockPinError;
    }
    impl InputPin for MockInputPin { // Implement for MockInputPin directly
        fn is_high(&mut self) -> Result<bool, Self::Error> {
             let state = self.states.pop().unwrap_or(PinState::Low);
             Ok(state == PinState::High)
        }
        fn is_low(&mut self) -> Result<bool, Self::Error> { 
            Ok(self.is_high()? == false)
         }
    }

    #[derive(Debug, Clone)] // Allow cloning for recording
    enum RecordedSpiOperation {
        Write(Vec<u8>),
        Transfer(Vec<u8>), // Record only written data
        TransferInPlace(Vec<u8>), // Record written data
        Read(usize), // Record length of read buffer
        DelayNs(u32),
    }

    #[derive(Debug, Default)]
    struct MockSpiDevice {
        pub writes: Vec<Vec<u8>>, // Keep recording raw writes separately for simplicity
        pub transactions: Vec<Vec<RecordedSpiOperation>> // Record sequences of operations in a transaction
    }
    impl SpiErrorType for MockSpiDevice {
        type Error = MockSpiError;
    }
    impl SpiDevice<u8> for MockSpiDevice { 
        fn transaction(&mut self, operations: &mut [Operation<u8>]) -> Result<(), Self::Error> {
            let mut recorded_ops = Vec::new(); 
            for op in operations.iter_mut() {
                match op {
                    Operation::Write(data) => {
                        let owned_data = data.to_vec();
                        self.writes.push(owned_data.clone()); // Also record in raw writes
                        recorded_ops.push(RecordedSpiOperation::Write(owned_data));
                    }
                    Operation::Transfer(read, write) => {
                         let owned_data = write.to_vec();
                         self.writes.push(owned_data.clone()); // Also record in raw writes
                         read.fill(0xAA); // Fill read buffer
                         recorded_ops.push(RecordedSpiOperation::Transfer(owned_data));
                    }
                    Operation::TransferInPlace(data) => {
                        let owned_data = data.to_vec(); // Data before modification
                        self.writes.push(owned_data.clone()); // Also record in raw writes
                        data.fill(0xBB); // Modify buffer with dummy read data
                        recorded_ops.push(RecordedSpiOperation::TransferInPlace(owned_data));
                    }
                    Operation::Read(read) => {
                        let len = read.len();
                        read.fill(0xCC); // Fill read buffer
                        recorded_ops.push(RecordedSpiOperation::Read(len));
                    }
                    Operation::DelayNs(ns) => {
                        recorded_ops.push(RecordedSpiOperation::DelayNs(*ns));
                     }
                }
            }
            if !recorded_ops.is_empty() {
                self.transactions.push(recorded_ops);
            }
            Ok(())    
        }

        fn write(&mut self, words: &[u8]) -> Result<(), Self::Error> {
            self.writes.push(words.to_vec());
            // Optionally, record this as a single-operation transaction
            // self.transactions.push(vec![RecordedSpiOperation::Write(words.to_vec())]);
            Ok(())    
        }
    }

    #[derive(Debug, Default)]
    struct MockDelay { pub delays_ms: Vec<u32> }
    impl DelayNs for MockDelay { // Implement for MockDelay directly
        fn delay_ns(&mut self, _ns: u32) {}
        fn delay_ms(&mut self, ms: u32) { self.delays_ms.push(ms); }
    }

    // --- Test Functions --- 

    #[test]
    fn test_set_pixel() {
        let mut spi = MockSpiDevice::default();
        let mut cs = MockOutputPin{ name: "cs", ..Default::default() };
        let mut dc = MockOutputPin{ name: "dc", ..Default::default() };
        let mut rst = MockOutputPin{ name: "rst", ..Default::default() };
        let mut busy = MockInputPin{ name: "busy", ..Default::default() };
        let mut delay = MockDelay::default();

        // Pass mocks directly, not mutable references unless the mock needs internal mutation tracking beyond Vec::push
        let mut display = Gdep073e01::new(spi, cs, dc, rst, busy, delay);

        // Initial buffer is white (0x11)
        assert_eq!(display.buffer[0], 0x11);

        // Set pixel at (0, 0) to Black (0x0)
        display.set_pixel(0, 0, Color::Black);
        assert_eq!(display.buffer[0], 0x01); // High nibble changes

        // Set pixel at (1, 0) to Red (0x3)
        display.set_pixel(1, 0, Color::Red);
        assert_eq!(display.buffer[0], 0x03); // Low nibble changes

        // Set pixel at (2, 0) to Blue (0x5)
        display.set_pixel(2, 0, Color::Blue);
        assert_eq!(display.buffer[1], 0x51); // Next byte, high nibble changes

        // Out of bounds
        display.set_pixel(WIDTH, HEIGHT, Color::Black);
        // No change expected, buffer size check is implicit
    }

    #[test]
    fn test_clear_buffer() {
        let mut spi = MockSpiDevice::default();
        let mut cs = MockOutputPin{ name: "cs", ..Default::default() };
        let mut dc = MockOutputPin{ name: "dc", ..Default::default() };
        let mut rst = MockOutputPin{ name: "rst", ..Default::default() };
        let mut busy = MockInputPin{ name: "busy", ..Default::default() };
        let mut delay = MockDelay::default();

        let mut display = Gdep073e01::new(spi, cs, dc, rst, busy, delay);

        display.set_pixel(0, 0, Color::Black); // Make buffer non-uniform
        assert_ne!(display.buffer[0], 0x44); 

        display.clear_buffer(Color::Orange); // Orange = 0x4
        assert!(display.buffer.iter().all(|&byte| byte == 0x44));
    }

    #[test]
    fn test_init_sequence() {
        let mut spi = MockSpiDevice::default();
        let mut cs = MockOutputPin{ name: "cs", ..Default::default() };
        let mut dc = MockOutputPin{ name: "dc", ..Default::default() };
        let mut rst = MockOutputPin{ name: "rst", ..Default::default() };
        let mut busy = MockInputPin{ name: "busy", ..Default::default() };
        let mut delay = MockDelay::default();

        // Simulate busy pin going low after reset and commands
        busy.states = vec![
            PinState::Low, // After power on
            PinState::Low, // After reset
        ];

        let mut display = Gdep073e01::new(spi, cs, dc, rst, busy, delay);
        display.init().unwrap();

        let spi_ref = display.spi;
        let rst_ref = display.rst;
        let delay_ref = display.delay;

        // Verify Reset Sequence
        assert_eq!(rst_ref.actions, vec![PinState::High, PinState::Low, PinState::High]);
        assert!(delay_ref.delays_ms.contains(&RESET_DELAY_MS));

        // Verify Init Commands
        let commands = spi_ref.writes.iter().filter(|w| w.len() == 1).map(|w| w[0]).collect::<Vec<_>>();
        let datas = spi_ref.writes.iter().filter(|w| w.len() > 1).collect::<Vec<_>>();

        // Expected sequence based on updated init()
        let expected_cmds = vec![
            CMD_CMDH,
            CMD_POWER_SETTING,
            CMD_PANEL_SETTING,
            CMD_POFS, // Changed from CMD_POWER_OFF
            CMD_BOOSTER_SOFT_START1,
            CMD_BOOSTER_SOFT_START2,
            CMD_BOOSTER_SOFT_START3,
            CMD_PLL_CONTROL,
            CMD_CDI,
            CMD_TCON_SETTING,
            CMD_TRES,
            CMD_T_VDCS, // Changed from CMD_VDCS
            CMD_PWS,
            CMD_POWER_ON,
        ];
        assert_eq!(commands, expected_cmds);

        // Check specific data payloads
        assert!(datas.iter().any(|d| *d == &[0x49, 0x55, 0x20, 0x08, 0x09, 0x18]), "CMDH data mismatch");
        assert!(datas.iter().any(|d| *d == &[0x00, 0x54, 0x00, 0x44]), "POFS data mismatch");
        assert!(datas.iter().any(|d| *d == &[0x01]), "T_VDCS data mismatch");
    }
    
    #[test]
    fn test_flush_sequence() {
        let mut spi = MockSpiDevice::default();
        let mut cs = MockOutputPin{ name: "cs", ..Default::default() };
        let mut dc = MockOutputPin{ name: "dc", ..Default::default() };
        let mut rst = MockOutputPin{ name: "rst", ..Default::default() };
        let mut busy = MockInputPin{ name: "busy", ..Default::default() };
        let mut delay = MockDelay::default();

        // Simulate busy pin going low after refresh
        busy.states = vec![PinState::Low]; 

        let mut display = Gdep073e01::new(spi, cs, dc, rst, busy, delay);
        // Modify buffer slightly to ensure it's sent
        display.buffer[0] = 0xAB;
        
        display.flush().unwrap();

        let spi_writes = &display.spi.writes;
        
        // Check Data Transmission Start command
        assert!(spi_writes.iter().any(|w| w.len() == 1 && w[0] == CMD_DATA_START_TRANSMISSION));
        
        // Check that the buffer content was sent (assuming write() is used, check last large write)
        assert!(spi_writes.iter().any(|w| w.len() == BUFFER_SIZE && w[0] == 0xAB));

        // Check Display Refresh command
        assert!(spi_writes.iter().any(|w| w.len() == 1 && w[0] == CMD_DISPLAY_REFRESH));
        // Check data associated with refresh
        assert!(spi_writes.iter().any(|w| w.len() > 1 && w[0] == 0x00)); // Data 0x00 for refresh command

        // Check that busy was polled (MockInputPin pops states)
        assert!(display.busy.states.is_empty());
    }

    #[test]
    fn test_sleep_sequence() {
        let mut spi = MockSpiDevice::default();
        let mut cs = MockOutputPin{ name: "cs", ..Default::default() };
        let mut dc = MockOutputPin{ name: "dc", ..Default::default() };
        let mut rst = MockOutputPin{ name: "rst", ..Default::default() };
        let mut busy = MockInputPin{ name: "busy", ..Default::default() };
        let mut delay = MockDelay::default();

        // Simulate busy pin going low after power off
        busy.states = vec![PinState::Low]; 

        let mut display = Gdep073e01::new(spi, cs, dc, rst, busy, delay);
        display.sleep().unwrap();

        let spi_writes = &display.spi.writes;
        
        // Check Power Off command
        assert!(spi_writes.iter().any(|w| w.len() == 1 && w[0] == CMD_POWER_OFF));
        // Check Power Off data
        assert!(spi_writes.iter().any(|w| w.len() > 1 && w[0] == 0x00)); // Data 0x00 for power off
        
        // Check Deep Sleep command
        assert!(spi_writes.iter().any(|w| w.len() == 1 && w[0] == CMD_DEEP_SLEEP));
        // Check Deep Sleep data
        assert!(spi_writes.iter().any(|w| w.len() > 1 && w[0] == 0xA5)); // Data 0xA5 for sleep

        // Check that busy was polled for power off, but not after sleep cmd
        assert!(display.busy.states.is_empty()); // Busy state was consumed by power_off
    }
}


// --- TODO ---
// - Verify init sequence and command parameters against the datasheet if possible.
// - Verify power control logic against the datasheet if possible.
// - Perform testing on actual hardware using a specific HAL implementation. 