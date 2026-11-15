//! Animated GIFs: downscale frames and encode them one at a time, so callers
//! can yield between frames and keep a UI responsive.

use std::fmt;

/// NeuQuant sampling speed, 1 (best) to 30 (fastest). 10 is the encoder's own
/// suggested compromise.
const QUANT_SPEED: i32 = 10;

/// Why a GIF couldn't be written.
#[derive(Debug)]
pub struct GifError(String);

impl fmt::Display for GifError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for GifError {}

impl From<gif::EncodingError> for GifError {
    fn from(e: gif::EncodingError) -> Self {
        Self(e.to_string())
    }
}

/// An infinitely looping GIF built up frame by frame in memory.
pub struct GifWriter {
    encoder: gif::Encoder<Vec<u8>>,
    width: u16,
    height: u16,
}

impl GifWriter {
    pub fn new(width: u32, height: u32) -> Result<Self, GifError> {
        let too_big = || GifError(format!("{width}×{height} is too large for a GIF"));
        let width = u16::try_from(width).map_err(|_| too_big())?;
        let height = u16::try_from(height).map_err(|_| too_big())?;
        let mut encoder = gif::Encoder::new(Vec::new(), width, height, &[])?;
        encoder.set_repeat(gif::Repeat::Infinite)?;
        Ok(Self {
            encoder,
            width,
            height,
        })
    }

    /// Append an RGBA frame shown for `delay_cs` hundredths of a second. Each
    /// frame gets its own 256-colour palette. `rgba` is used as scratch space.
    pub fn push(&mut self, rgba: &mut [u8], delay_cs: u16) -> Result<(), GifError> {
        let expected = self.width as usize * self.height as usize * 4;
        if rgba.len() != expected {
            return Err(GifError(format!(
                "frame is {} bytes, expected {expected}",
                rgba.len()
            )));
        }
        let mut frame = gif::Frame::from_rgba_speed(self.width, self.height, rgba, QUANT_SPEED);
        frame.delay = delay_cs;
        self.encoder.write_frame(&frame)?;
        Ok(())
    }

    /// Finish the file and return its bytes.
    pub fn finish(self) -> Result<Vec<u8>, GifError> {
        Ok(self.encoder.into_inner()?)
    }
}

/// Shrink an RGBA image so it's at most `max_width` wide, averaging each
/// source block (a box filter). Images that already fit are copied unchanged.
pub fn downscale(rgba: &[u8], width: u32, height: u32, max_width: u32) -> (Vec<u8>, u32, u32) {
    if width <= max_width || width == 0 || height == 0 {
        return (rgba.to_vec(), width, height);
    }
    let (sw, sh) = (width as usize, height as usize);
    let dw = max_width.max(1) as usize;
    let dh = (sh * dw / sw).max(1);

    let mut out = vec![0u8; dw * dh * 4];
    for y in 0..dh {
        let (y0, y1) = (y * sh / dh, ((y + 1) * sh / dh).max(y * sh / dh + 1));
        for x in 0..dw {
            let (x0, x1) = (x * sw / dw, ((x + 1) * sw / dw).max(x * sw / dw + 1));
            let mut sum = [0u32; 4];
            for row in rgba[y0 * sw * 4..y1 * sw * 4].chunks_exact(sw * 4) {
                for px in row[x0 * 4..x1 * 4].as_chunks::<4>().0 {
                    for (s, &v) in sum.iter_mut().zip(px) {
                        *s += v as u32;
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            let o = (y * dw + x) * 4;
            for (dst, s) in out[o..o + 4].iter_mut().zip(sum) {
                *dst = ((s + n / 2) / n) as u8;
            }
        }
    }
    (out, dw as u32, dh as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, px: [u8; 4]) -> Vec<u8> {
        px.repeat((w * h) as usize)
    }

    #[test]
    fn downscale_keeps_aspect_and_colour() {
        let (out, w, h) = downscale(&solid(1280, 720, [10, 200, 30, 255]), 1280, 720, 480);
        assert_eq!((w, h), (480, 270));
        assert_eq!(out.len(), 480 * 270 * 4);
        assert!(
            out.as_chunks::<4>()
                .0
                .iter()
                .all(|p| *p == [10, 200, 30, 255])
        );
    }

    #[test]
    fn downscale_averages_blocks() {
        // 4×1: black, white, black, white → 2×1 greys.
        let src = [
            0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255,
        ];
        let (out, w, h) = downscale(&src, 4, 1, 2);
        assert_eq!((w, h), (2, 1));
        assert_eq!(out, [128, 128, 128, 255, 128, 128, 128, 255]);
    }

    #[test]
    fn downscale_leaves_small_images_alone() {
        let src = solid(320, 240, [1, 2, 3, 255]);
        assert_eq!(downscale(&src, 320, 240, 480), (src, 320, 240));
    }

    #[test]
    fn writes_a_looping_animation() {
        let mut gif = GifWriter::new(8, 6).unwrap();
        gif.push(&mut solid(8, 6, [255, 0, 0, 255]), 100).unwrap();
        gif.push(&mut solid(8, 6, [0, 0, 255, 255]), 100).unwrap();
        let bytes = gif.finish().unwrap();

        assert_eq!(&bytes[..6], b"GIF89a");
        assert_eq!(bytes.last(), Some(&0x3B)); // trailer
        let mut decoder = gif::DecodeOptions::new()
            .read_info(bytes.as_slice())
            .unwrap();
        let mut frames = 0;
        while let Some(frame) = decoder.read_next_frame().unwrap() {
            assert_eq!((frame.width, frame.height, frame.delay), (8, 6, 100));
            frames += 1;
        }
        assert_eq!(frames, 2);
    }

    #[test]
    fn rejects_wrong_sized_frames() {
        let mut gif = GifWriter::new(8, 6).unwrap();
        assert!(gif.push(&mut solid(4, 4, [0, 0, 0, 255]), 10).is_err());
    }
}
