//! DitherDrawTarget adapter: converts Rgb888 to panel Color using a strategy.

use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::*,
    primitives::Rectangle,
};

use crate::dither::DitherStrategy;
use crate::palette::{map_rgb_to_spectra6_nearest, Spectra6};

/// Wrap an embedded-graphics DrawTarget to apply palette+dither at draw time.
pub struct DitherDrawTarget<T, S> {
    inner: T,
    strat: S,
}

impl<T, S> DitherDrawTarget<T, S> {
    pub fn new(inner: T, strat: S) -> Self { Self { inner, strat } }
    pub fn into_inner(self) -> T { self.inner }
    pub fn inner_mut(&mut self) -> &mut T { &mut self.inner }
    pub fn strategy_mut(&mut self) -> &mut S { &mut self.strat }
}

impl<T, S, E> DrawTarget for DitherDrawTarget<T, S>
where
    T: DrawTarget<Color = crate::Color, Error = E> + OriginDimensions,
    S: DitherStrategy,
{
    type Color = Rgb888;
    type Error = E;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        for Pixel(coord, rgb) in pixels.into_iter() {
            let (x, y) = (coord.x as u32, coord.y as u32);
            let c6 = self
                .strat
                .map(x, y, [rgb.r(), rgb.g(), rgb.b()])
                .to_driver_color();
            // Forward as single pixel
            self.inner.draw_iter(core::iter::once(Pixel(coord, c6)))?;
        }
        Ok(())
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        // Use a simple iterator over the area mapping each pixel.
        let rb = area.bounding_box();
        let tl = rb.top_left;
        let w = rb.size.width as i32;
        let h = rb.size.height as i32;
        for y in tl.y..(tl.y + h) {
            for x in tl.x..(tl.x + w) {
                let sx = x as u32;
                let sy = y as u32;
                let c6 = self
                    .strat
                    .map(sx, sy, [color.r(), color.g(), color.b()])
                    .to_driver_color();
                self.inner.draw_iter(core::iter::once(Pixel(Point::new(x, y), c6)))?;
            }
        }
        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        let c6 = map_rgb_to_spectra6_nearest([color.r(), color.g(), color.b()]).to_driver_color();
        self.inner.clear(c6)
    }
}

impl<T, S> OriginDimensions for DitherDrawTarget<T, S>
where
    T: OriginDimensions,
{
    fn size(&self) -> Size { self.inner.size() }
}
