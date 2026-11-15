//! Compose captured frames into a classic vertical photo strip.

/// Geometry and colour of a vertical strip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StripLayout {
    /// Outer border around the whole strip, in pixels.
    pub border: u32,
    /// Gap between frames, in pixels.
    pub gap: u32,
    /// Extra space below the last frame for a caption, in pixels.
    pub footer: u32,
    /// Background colour as RGBA.
    pub background: [u8; 4],
}

impl Default for StripLayout {
    fn default() -> Self {
        Self {
            border: 32,
            gap: 24,
            footer: 96,
            background: [250, 247, 240, 255],
        }
    }
}

impl StripLayout {
    /// Size of the strip for `count` frames of `w`×`h`.
    pub fn size(&self, count: u32, w: u32, h: u32) -> (u32, u32) {
        let width = w + self.border * 2;
        let height = self.border * 2 + h * count + self.gap * count.saturating_sub(1) + self.footer;
        (width, height)
    }

    /// Top-left corner of frame `i`.
    pub fn frame_origin(&self, i: u32, h: u32) -> (u32, u32) {
        (self.border, self.border + i * (h + self.gap))
    }

    /// Vertical centre of the caption area.
    pub fn caption_center_y(&self, count: u32, h: u32) -> u32 {
        let (_, total) = self.size(count, 0, h);
        total - self.border - self.footer / 2
    }
}

/// Stack `frames` (each `w`×`h` RGBA8) into a strip. Returns `(rgba, width, height)`.
///
/// # Panics
/// If any frame is not exactly `w * h * 4` bytes.
pub fn compose_strip(
    frames: &[&[u8]],
    w: u32,
    h: u32,
    layout: &StripLayout,
) -> (Vec<u8>, u32, u32) {
    let count = frames.len() as u32;
    let (sw, sh) = layout.size(count, w, h);
    let mut out: Vec<u8> = layout
        .background
        .iter()
        .copied()
        .cycle()
        .take((sw * sh * 4) as usize)
        .collect();

    let row_bytes = (w * 4) as usize;
    let stride = (sw * 4) as usize;
    for (i, frame) in frames.iter().enumerate() {
        assert_eq!(
            frame.len(),
            row_bytes * h as usize,
            "frame {i} has wrong size"
        );
        let (ox, oy) = layout.frame_origin(i as u32, h);
        for row in 0..h as usize {
            let dst = (oy as usize + row) * stride + ox as usize * 4;
            let src = row * row_bytes;
            out[dst..dst + row_bytes].copy_from_slice(&frame[src..src + row_bytes]);
        }
    }
    (out, sw, sh)
}

#[cfg(test)]
mod tests {
    use super::*;

    const L: StripLayout = StripLayout {
        border: 2,
        gap: 1,
        footer: 3,
        background: [1, 2, 3, 255],
    };

    fn pixel(buf: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * width + x) * 4) as usize;
        buf[i..i + 4].try_into().unwrap()
    }

    #[test]
    fn size_accounts_for_border_gaps_and_footer() {
        assert_eq!(L.size(4, 10, 5), (14, 2 * 2 + 5 * 4 + 3 + 3));
        assert_eq!(L.size(1, 10, 5), (14, 2 * 2 + 5 + 3));
    }

    #[test]
    fn frames_land_in_order_and_background_fills_rest() {
        let red = [255, 0, 0, 255].repeat(2 * 2);
        let blue = [0, 0, 255, 255].repeat(2 * 2);
        let (buf, sw, sh) = compose_strip(&[&red, &blue], 2, 2, &L);
        assert_eq!((sw, sh), (6, 2 * 2 + 2 * 2 + 1 + 3));
        assert_eq!(buf.len(), (sw * sh * 4) as usize);

        assert_eq!(pixel(&buf, sw, 0, 0), [1, 2, 3, 255]);
        assert_eq!(pixel(&buf, sw, 2, 2), [255, 0, 0, 255]);
        assert_eq!(pixel(&buf, sw, 3, 3), [255, 0, 0, 255]);
        assert_eq!(pixel(&buf, sw, 2, 4), [1, 2, 3, 255]); // gap
        assert_eq!(pixel(&buf, sw, 2, 5), [0, 0, 255, 255]);
        assert_eq!(pixel(&buf, sw, 2, sh - 1), [1, 2, 3, 255]); // footer
    }

    #[test]
    #[should_panic(expected = "wrong size")]
    fn rejects_mis_sized_frames() {
        compose_strip(&[&[0u8; 4]], 2, 2, &L);
    }
}
