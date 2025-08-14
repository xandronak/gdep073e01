//! Dithering and halftone strategies to map RGB->Spectra6.
//! Feature-gated implementations, no_std by default; FS requires alloc.

use crate::palette::{add_bias, map_rgb_to_spectra6_nearest, Spectra6};

/// Strategy trait for per-pixel mapping with spatial/temporal context.
pub trait DitherStrategy {
    /// Map an sRGB triple at pixel (x,y) to Spectra6.
    /// `x,y` are absolute framebuffer coords for matrix patterns.
    fn map(&mut self, x: u32, y: u32, rgb: [u8; 3]) -> Spectra6;
}

/// Ordered Bayer 4x4: zero-alloc, fast.
#[cfg(feature = "dither-bayer")]
pub struct Bayer4x4;

#[cfg(feature = "dither-bayer")]
impl DitherStrategy for Bayer4x4 {
    fn map(&mut self, x: u32, y: u32, rgb: [u8; 3]) -> Spectra6 {
        // 4x4 Bayer thresholds 0..15
        // Source: standard Bayer matrix
        const M: [[i16; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
        let t = M[(y as usize) & 3][(x as usize) & 3] as i16; // 0..15
                                                              // Convert t to a small bias in -8..+7
        let bias = t - 8;
        // Apply slight luminance-ish bias equally to channels
        let b = [bias, bias, bias];
        let nudged = add_bias(rgb, b);
        map_rgb_to_spectra6_nearest(nudged)
    }
}

/// Floyd–Steinberg: keeps 2 lines of error (alloc).
#[cfg(feature = "dither-fs")]
pub struct FloydSteinberg {
    width: u32,
    /// Two rows of error, interleaved RGB, i16 range to hold accumulated error.
    cur: alloc::vec::Vec<i16>,
    nxt: alloc::vec::Vec<i16>,
    x: u32,
    y: u32,
}

#[cfg(feature = "dither-fs")]
impl FloydSteinberg {
    pub fn new(width: u32) -> Self {
        let len = (width as usize) * 3;
        Self {
            width,
            cur: alloc::vec![0; len],
            nxt: alloc::vec![0; len],
            x: 0,
            y: 0,
        }
    }
    /// Call at the start of each new scanline y to advance the buffers if needed.
    pub fn start_line(&mut self, y: u32) {
        if y != self.y {
            core::mem::swap(&mut self.cur, &mut self.nxt);
            for v in &mut self.nxt {
                *v = 0;
            }
            self.y = y;
            self.x = 0;
        }
    }
}

#[cfg(feature = "dither-fs")]
impl DitherStrategy for FloydSteinberg {
    fn map(&mut self, x: u32, y: u32, rgb: [u8; 3]) -> Spectra6 {
        // Assume left-to-right scanline order. If new line, roll buffers.
        if y != self.y || (x == 0 && self.x != 0) {
            self.start_line(y);
        }
        self.x = x;
        let idx = (x as usize) * 3;
        let adj = [
            crate::palette::clamp_u8(rgb[0] as i32 + self.cur[idx + 0] as i32),
            crate::palette::clamp_u8(rgb[1] as i32 + self.cur[idx + 1] as i32),
            crate::palette::clamp_u8(rgb[2] as i32 + self.cur[idx + 2] as i32),
        ];
        let q = map_rgb_to_spectra6_nearest(adj);
        // Quantization error e = adj - q_color
        let qc = match q {
            Spectra6::White => [255, 255, 255],
            Spectra6::Black => [0, 0, 0],
            Spectra6::Yellow => [255, 255, 0],
            Spectra6::Red => [255, 0, 0],
            Spectra6::Green => [0, 255, 0],
            Spectra6::Blue => [0, 0, 255],
        };
        let er = adj[0] as i16 - qc[0] as i16;
        let eg = adj[1] as i16 - qc[1] as i16;
        let eb = adj[2] as i16 - qc[2] as i16;
        // Distribute error: right (7/16), down-left (3/16), down (5/16), down-right (1/16)
        // Right neighbor
        if x + 1 < self.width {
            let j = idx + 3;
            self.cur[j + 0] = self.cur[j + 0].saturating_add((er * 7) / 16);
            self.cur[j + 1] = self.cur[j + 1].saturating_add((eg * 7) / 16);
            self.cur[j + 2] = self.cur[j + 2].saturating_add((eb * 7) / 16);
        }
        // Next row indices
        let below_base = idx;
        // Down-left
        if x > 0 {
            let j = below_base - 3;
            self.nxt[j + 0] = self.nxt[j + 0].saturating_add((er * 3) / 16);
            self.nxt[j + 1] = self.nxt[j + 1].saturating_add((eg * 3) / 16);
            self.nxt[j + 2] = self.nxt[j + 2].saturating_add((eb * 3) / 16);
        }
        // Down
        {
            let j = below_base;
            self.nxt[j + 0] = self.nxt[j + 0].saturating_add((er * 5) / 16);
            self.nxt[j + 1] = self.nxt[j + 1].saturating_add((eg * 5) / 16);
            self.nxt[j + 2] = self.nxt[j + 2].saturating_add((eb * 5) / 16);
        }
        // Down-right
        if x + 1 < self.width {
            let j = below_base + 3;
            self.nxt[j + 0] = self.nxt[j + 0].saturating_add((er * 1) / 16);
            self.nxt[j + 1] = self.nxt[j + 1].saturating_add((eg * 1) / 16);
            self.nxt[j + 2] = self.nxt[j + 2].saturating_add((eb * 1) / 16);
        }
        q
    }
}

/// Halftone tiles 2x2/3x3 with discrete fill levels between two palette colors.
#[cfg(feature = "halftone")]
pub struct Halftone {
    /// Use 2 for 2x2 tiles or 3 for 3x3.
    pub tile: u8,
}

#[cfg(feature = "halftone")]
impl Halftone {
    pub fn new(tile: u8) -> Self {
        Self {
            tile: if tile < 2 { 2 } else { tile.min(3) },
        }
    }
    #[inline]
    fn level_from_rgb(rgb: [u8; 3]) -> u8 {
        // Simple luminance approximation 0..255
        let y = (3 * rgb[0] as u16 + 6 * rgb[1] as u16 + 1 * rgb[2] as u16) / 10;
        // Map to 0, 64,128,192,255 ~ 5 levels
        if y < 32 {
            0
        } else if y < 96 {
            1
        } else if y < 160 {
            2
        } else if y < 224 {
            3
        } else {
            4
        }
    }
}

#[cfg(feature = "halftone")]
impl DitherStrategy for Halftone {
    fn map(&mut self, x: u32, y: u32, rgb: [u8; 3]) -> Spectra6 {
        let lvl = Self::level_from_rgb(rgb);
        let n = self.tile as u32;
        let xi = (x % n) as u8;
        let yi = (y % n) as u8;
        // Between Black and White by default; colorized blends future work.
        // 2x2 ordering for levels 0..4
        let on = if self.tile == 2 {
            // 2x2 pattern order: [ (0,0), (1,1), (1,0), (0,1) ]
            let rank = match (xi, yi) {
                (0, 0) => 0,
                (1, 1) => 1,
                (1, 0) => 2,
                _ => 3,
            };
            lvl > rank
        } else {
            // 3x3 pattern order by Bayer-like ranks 0..8
            let rank = match (xi, yi) {
                (1, 1) => 0,
                (2, 0) | (0, 2) => 1,
                (0, 1) | (1, 0) | (1, 2) | (2, 1) => 2,
                (0, 0) | (2, 2) => 3,
                _ => 4,
            };
            lvl > rank
        };
        if on {
            Spectra6::White
        } else {
            Spectra6::Black
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "dither-bayer")]
    #[test]
    fn bayer_deterministic() {
        let mut b = Bayer4x4;
        let a = b.map(10, 10, [120, 130, 140]);
        let a2 = b.map(10, 10, [120, 130, 140]);
        assert_eq!(a, a2);
    }

    #[cfg(feature = "halftone")]
    #[test]
    fn halftone_levels() {
        let mut h = Halftone::new(2);
        // Dark should be black, light should be white at same coords
        let c1 = h.map(0, 0, [10, 10, 10]);
        let c2 = h.map(0, 0, [240, 240, 240]);
        assert!(matches!(c1, Spectra6::Black));
        assert!(matches!(c2, Spectra6::White));
    }
}
