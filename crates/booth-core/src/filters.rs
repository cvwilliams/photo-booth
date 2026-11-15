//! Per-pixel filters applied in place to RGBA8 buffers.

/// A photo filter. Alpha is always left untouched.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Filter {
    #[default]
    None,
    Mono,
    Sepia,
    Noir,
    Invert,
    Pop,
}

impl Filter {
    pub const ALL: [Filter; 6] = [
        Filter::None,
        Filter::Mono,
        Filter::Sepia,
        Filter::Noir,
        Filter::Invert,
        Filter::Pop,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Filter::None => "Original",
            Filter::Mono => "Mono",
            Filter::Sepia => "Sepia",
            Filter::Noir => "Noir",
            Filter::Invert => "Invert",
            Filter::Pop => "Pop",
        }
    }

    /// Apply the filter in place. `rgba.len()` should be a multiple of 4;
    /// any trailing partial pixel is ignored.
    pub fn apply(self, rgba: &mut [u8]) {
        match self {
            Filter::None => {}
            Filter::Mono => map_pixels(rgba, |r, g, b| {
                let y = luma(r, g, b);
                (y, y, y)
            }),
            Filter::Sepia => map_pixels(rgba, |r, g, b| {
                let (r, g, b) = (r as f32, g as f32, b as f32);
                (
                    clamp(0.393 * r + 0.769 * g + 0.189 * b),
                    clamp(0.349 * r + 0.686 * g + 0.168 * b),
                    clamp(0.272 * r + 0.534 * g + 0.131 * b),
                )
            }),
            Filter::Noir => map_pixels(rgba, |r, g, b| {
                // High-contrast black & white: stretch luma around mid-grey.
                let y = luma(r, g, b) as f32;
                let v = clamp((y - 128.0) * 1.8 + 128.0);
                (v, v, v)
            }),
            Filter::Invert => map_pixels(rgba, |r, g, b| (255 - r, 255 - g, 255 - b)),
            Filter::Pop => map_pixels(rgba, |r, g, b| {
                // Boost saturation, then posterize to 4 levels per channel.
                let y = luma(r, g, b) as f32;
                let sat = |c: u8| clamp(y + (c as f32 - y) * 1.6);
                (posterize(sat(r)), posterize(sat(g)), posterize(sat(b)))
            }),
        }
    }
}

#[inline]
fn map_pixels(rgba: &mut [u8], f: impl Fn(u8, u8, u8) -> (u8, u8, u8)) {
    for px in rgba.as_chunks_mut::<4>().0 {
        let (r, g, b) = f(px[0], px[1], px[2]);
        px[0] = r;
        px[1] = g;
        px[2] = b;
    }
}

/// Rec. 601 luma using integer math.
#[inline]
fn luma(r: u8, g: u8, b: u8) -> u8 {
    ((r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000) as u8
}

#[inline]
fn clamp(v: f32) -> u8 {
    v.clamp(0.0, 255.0) as u8
}

#[inline]
fn posterize(v: u8) -> u8 {
    const STEP: u8 = 85; // 255 / 3 -> levels 0, 85, 170, 255
    ((v as u16 + STEP as u16 / 2) / STEP as u16 * STEP as u16).min(255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(r: u8, g: u8, b: u8, a: u8) -> Vec<u8> {
        vec![r, g, b, a]
    }

    #[test]
    fn none_is_identity() {
        let mut buf = px(10, 20, 30, 40);
        Filter::None.apply(&mut buf);
        assert_eq!(buf, px(10, 20, 30, 40));
    }

    #[test]
    fn mono_produces_equal_channels_and_keeps_alpha() {
        let mut buf = px(200, 100, 50, 77);
        Filter::Mono.apply(&mut buf);
        assert_eq!(buf[0], buf[1]);
        assert_eq!(buf[1], buf[2]);
        assert_eq!(buf[3], 77);
    }

    #[test]
    fn mono_white_and_black_are_preserved() {
        let mut buf = [px(255, 255, 255, 255), px(0, 0, 0, 255)].concat();
        Filter::Mono.apply(&mut buf);
        assert_eq!(&buf[..3], &[255, 255, 255]);
        assert_eq!(&buf[4..7], &[0, 0, 0]);
    }

    #[test]
    fn invert_twice_is_identity() {
        let original = px(12, 200, 99, 255);
        let mut buf = original.clone();
        Filter::Invert.apply(&mut buf);
        assert_eq!(buf, px(243, 55, 156, 255));
        Filter::Invert.apply(&mut buf);
        assert_eq!(buf, original);
    }

    #[test]
    fn sepia_is_warm() {
        let mut buf = px(128, 128, 128, 255);
        Filter::Sepia.apply(&mut buf);
        assert!(buf[0] > buf[1] && buf[1] > buf[2], "got {buf:?}");
    }

    #[test]
    fn noir_pushes_away_from_mid_grey() {
        let mut buf = [px(100, 100, 100, 255), px(160, 160, 160, 255)].concat();
        Filter::Noir.apply(&mut buf);
        assert!(buf[0] < 100);
        assert!(buf[4] > 160);
    }

    #[test]
    fn pop_uses_four_levels() {
        let mut buf: Vec<u8> = (0..=255u8).flat_map(|v| [v, v / 2, 255 - v, 255]).collect();
        Filter::Pop.apply(&mut buf);
        for p in buf.as_chunks::<4>().0 {
            for &c in &p[..3] {
                assert!([0, 85, 170, 255].contains(&c), "unexpected level {c}");
            }
        }
    }

    #[test]
    fn ignores_trailing_partial_pixel() {
        let mut buf = vec![0, 0, 0, 255, 9, 9];
        Filter::Invert.apply(&mut buf);
        assert_eq!(buf, vec![255, 255, 255, 255, 9, 9]);
    }
}
