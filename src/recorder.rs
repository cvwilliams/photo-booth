//! Record short clips of the (already filtered) preview canvas with
//! `MediaRecorder`, so videos look exactly like the live preview.

use std::{cell::RefCell, rc::Rc};

use gloo_timers::future::TimeoutFuture;
use js_sys::{Array, Promise};
use wasm_bindgen::{JsCast, prelude::Closure};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Blob, BlobEvent, BlobPropertyBag, HtmlCanvasElement, MediaRecorder, MediaRecorderOptions,
    MediaStreamTrack, Url,
};

/// A finished recording, held as an object URL until [`release`] is called.
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub url: String,
    /// File extension matching the container the browser recorded.
    pub ext: &'static str,
}

/// Containers in order of preference. MP4 first: it plays almost everywhere
/// once downloaded. Safari only records MP4; older Chrome/Firefox only WebM.
const FORMATS: [(&str, &str); 5] = [
    ("video/mp4;codecs=avc1", "mp4"),
    ("video/mp4", "mp4"),
    ("video/webm;codecs=vp9", "webm"),
    ("video/webm;codecs=vp8", "webm"),
    ("video/webm", "webm"),
];

/// Record `seconds` of `canvas`, calling `on_tick` with the seconds left
/// (`seconds`, …, 1, then 0 when done).
pub async fn record(
    canvas: &HtmlCanvasElement,
    seconds: u32,
    on_tick: impl Fn(u32),
) -> Result<Clip, String> {
    let (mime, ext) = FORMATS
        .into_iter()
        .find(|(m, _)| MediaRecorder::is_type_supported(m))
        .ok_or("This browser can't record video.")?;

    let stream = canvas
        .capture_stream_with_frame_request_rate(30.0)
        .map_err(|_| "Couldn't capture the preview.")?;

    let options = MediaRecorderOptions::new();
    options.set_mime_type(mime);
    options.set_video_bits_per_second(6_000_000);
    let recorder =
        MediaRecorder::new_with_media_stream_and_media_recorder_options(&stream, &options)
            .map_err(|_| "Couldn't start recording.")?;

    let chunks: Rc<RefCell<Vec<Blob>>> = Rc::default();
    let on_data = {
        let chunks = chunks.clone();
        Closure::<dyn FnMut(BlobEvent)>::new(move |e: BlobEvent| {
            if let Some(blob) = e.data().filter(|b| b.size() > 0.0) {
                chunks.borrow_mut().push(blob);
            }
        })
    };
    recorder.set_ondataavailable(Some(on_data.as_ref().unchecked_ref()));
    // `stop` fires after the final `dataavailable`, so all chunks are in by then.
    let stopped = Promise::new(&mut |resolve, _reject| recorder.set_onstop(Some(&resolve)));

    recorder.start().map_err(|_| "Couldn't start recording.")?;
    for left in (1..=seconds).rev() {
        on_tick(left);
        TimeoutFuture::new(1000).await;
    }
    on_tick(0);
    recorder.stop().map_err(|_| "Couldn't stop recording.")?;
    JsFuture::from(stopped)
        .await
        .map_err(|_| "Recording failed.")?;

    for track in stream.get_tracks().iter() {
        track.unchecked_into::<MediaStreamTrack>().stop();
    }
    recorder.set_ondataavailable(None);
    drop(on_data);

    let parts = Array::new();
    for blob in chunks.borrow().iter() {
        parts.push(blob);
    }
    let bag = BlobPropertyBag::new();
    bag.set_type(mime.split(';').next().unwrap_or(mime));
    let blob = Blob::new_with_blob_sequence_and_options(&parts, &bag)
        .map_err(|_| "Couldn't save the recording.")?;
    let url =
        Url::create_object_url_with_blob(&blob).map_err(|_| "Couldn't save the recording.")?;
    Ok(Clip { url, ext })
}

/// Free the memory behind a clip's object URL.
pub fn release(clip: &Clip) {
    let _ = Url::revoke_object_url(&clip.url);
}
