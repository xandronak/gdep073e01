//! Spectra6 palette and mapping utilities.
//! Works in no_std.


/// Fixed Spectra 6 palette order used by the panel’s LUT/driver (excluding Orange).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Spectra6 {
    White,
    Black,
    Yellow,
    Red,
    Green,
    Blue,
}

/// Panel order palette RGB centers in sRGB 8-bit.
/// Order: White, Black, Yellow, Red, Green, Blue
pub const PALETTE: [[u8; 3]; 6] = [
    [255, 255, 255], // White
    [0, 0, 0],       // Black
    [255, 255, 0],   // Yellow
    [255, 0, 0],     // Red
    [0, 255, 0],     // Green
    [0, 0, 255],     // Blue
];

impl Spectra6 {
    /// Convert this Spectra6 color to the driver's 7-color `Color` variant.
    /// Note: Orange is intentionally not used by Spectra6.
    pub fn to_driver_color(self) -> crate::Color {
        match self {
            Spectra6::White => crate::Color::White,
            Spectra6::Black => crate::Color::Black,
            Spectra6::Yellow => crate::Color::Yellow,
            Spectra6::Red => crate::Color::Red,
            Spectra6::Green => crate::Color::Green,
            Spectra6::Blue => crate::Color::Blue,
        }
    }
}

/// Cheap perceptual-ish distance between two sRGB triples (0..=255).
/// Uses a weighted squared distance to approximate luminance sensitivity without floats.
#[inline]
fn dist2_weighted(a: [u8; 3], b: [u8; 3]) -> u32 {
    let dr = a[0] as i32 - b[0] as i32;
    let dg = a[1] as i32 - b[1] as i32;
    let db = a[2] as i32 - b[2] as i32;
    // Weights approx human sensitivity: G strongest, then R, then B
    // wR=3, wG=6, wB=1
    (3 * dr * dr + 6 * dg * dg + 1 * db * db) as u32
}

/// RGB -> closest Spectra6 color (no dither).
#[inline]
pub fn map_rgb_to_spectra6_nearest(rgb: [u8; 3]) -> Spectra6 {
    // Find minimum distance in PALETTE
    let mut best = 0usize;
    let mut best_d = u32::MAX;
    for (i, p) in PALETTE.iter().enumerate() {
        let d = dist2_weighted(rgb, *p);
        if d < best_d {
            best_d = d;
            best = i;
        }
    }
    match best {
        0 => Spectra6::White,
        1 => Spectra6::Black,
        2 => Spectra6::Yellow,
        3 => Spectra6::Red,
        4 => Spectra6::Green,
        _ => Spectra6::Blue,
    }
}

/// Utility: clamp i32 to 0..=255 and return u8.
#[inline]
pub fn clamp_u8(v: i32) -> u8 {
    if v < 0 { 0 } else if v > 255 { 255 } else { v as u8 }
}

/// Utility: add bias to an rgb triple with saturation.
#[inline]
pub fn add_bias(rgb: [u8;3], bias: [i16;3]) -> [u8;3] {
    [
        clamp_u8(rgb[0] as i32 + bias[0] as i32),
        clamp_u8(rgb[1] as i32 + bias[1] as i32),
        clamp_u8(rgb[2] as i32 + bias[2] as i32),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_basic() {
        assert_eq!(map_rgb_to_spectra6_nearest([250,250,250]), Spectra6::White);
        assert_eq!(map_rgb_to_spectra6_nearest([5,5,5]), Spectra6::Black);
        assert_eq!(map_rgb_to_spectra6_nearest([250,240,10]), Spectra6::Yellow);
        assert_eq!(map_rgb_to_spectra6_nearest([250,10,10]), Spectra6::Red);
        assert_eq!(map_rgb_to_spectra6_nearest([10,250,10]), Spectra6::Green);
        assert_eq!(map_rgb_to_spectra6_nearest([10,10,250]), Spectra6::Blue);
    }

    #[test]
    fn spectra6_to_driver_color_nibbles() {
        // Verify that Spectra6 maps to the native nibble codes used by the panel,
        // as per the C++ reference _convert_to_native mapping.
        assert_eq!(Spectra6::Black.to_driver_color() as u8, 0x00);
        assert_eq!(Spectra6::White.to_driver_color() as u8, 0x01);
        assert_eq!(Spectra6::Yellow.to_driver_color() as u8, 0x02);
        assert_eq!(Spectra6::Red.to_driver_color() as u8, 0x03);
        assert_eq!(Spectra6::Blue.to_driver_color() as u8, 0x05);
        assert_eq!(Spectra6::Green.to_driver_color() as u8, 0x06);
    }
}
