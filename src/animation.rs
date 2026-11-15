//! GIF mode: grab filtered frames from the renderer and encode them as a
//! looping GIF in Rust, a frame at a time so the preview keeps running.

use std::{cell::RefCell, rc::Rc};

use booth_core::{GifWriter, downscale};
use gloo_timers::future::TimeoutFuture;
use js_sys::{Array, Uint8Array};
use web_sys::{Blob, BlobPropertyBag, Url};

use crate::{
    recorder::Clip,
    renderer::{Frame, Renderer},
};

/// Shrink a frame for a GIF. GIFs are big, so they don't need full resolution.
pub fn shrink(frame: &Frame, max_width: u32) -> Frame {
    let (rgba, width, height) = downscale(&frame.rgba, frame.width, frame.height, max_width);
    Frame {
        rgba,
        width,
        height,
    }
}

/// Grab `seconds` of the preview at `fps`, shrunk to `max_width`, calling
/// `on_tick` with the whole seconds left (`seconds`, …, 1, then 0 when done).
pub async fn capture(
    renderer: &Rc<RefCell<Renderer>>,
    seconds: u32,
    fps: u32,
    max_width: u32,
    on_tick: impl Fn(u32),
) -> Vec<Frame> {
    let total = seconds * fps;
    let step = 1000.0 / fps as f64;
    let start = js_sys::Date::now();
    let mut frames = Vec::with_capacity(total as usize);
    for i in 0..total {
        if i % fps == 0 {
            on_tick(seconds - i / fps);
        }
        // Wait for this frame's slot on a fixed schedule, so slow frames
        // don't stretch the clip.
        let wait = start + i as f64 * step - js_sys::Date::now();
        TimeoutFuture::new(wait.max(0.0) as u32).await;
        if let Some(frame) = renderer.borrow().last_frame() {
            frames.push(shrink(&frame, max_width));
        }
    }
    let wait = start + total as f64 * step - js_sys::Date::now();
    TimeoutFuture::new(wait.max(0.0) as u32).await;
    on_tick(0);
    frames
}

/// Encode `frames` (all the same size), each shown for `delay_cs` hundredths
/// of a second, calling `on_progress` with 0–100.
pub async fn encode(
    frames: Vec<Frame>,
    delay_cs: u16,
    on_progress: impl Fn(u32),
) -> Result<Clip, String> {
    let first = frames.first().ok_or("No frames were captured.")?;
    let mut gif = GifWriter::new(first.width, first.height).map_err(|e| e.to_string())?;
    let total = frames.len() as u32;
    on_progress(0);
    for (i, mut frame) in frames.into_iter().enumerate() {
        // Hand control back to the browser between frames so the preview
        // and the progress readout keep updating.
        TimeoutFuture::new(0).await;
        gif.push(&mut frame.rgba, delay_cs)
            .map_err(|e| e.to_string())?;
        on_progress((i as u32 + 1) * 100 / total);
    }
    let bytes = gif.finish().map_err(|e| e.to_string())?;

    let parts = Array::of1(&Uint8Array::from(bytes.as_slice()));
    let bag = BlobPropertyBag::new();
    bag.set_type("image/gif");
    let blob = Blob::new_with_u8_array_sequence_and_options(&parts, &bag)
        .map_err(|_| "Couldn't save the GIF.")?;
    let url = Url::create_object_url_with_blob(&blob).map_err(|_| "Couldn't save the GIF.")?;
    Ok(Clip { url, ext: "gif" })
}
