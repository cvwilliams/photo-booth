//! Webcam access via `getUserMedia`.

use js_sys::{Object, Reflect};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{HtmlVideoElement, MediaStream, MediaStreamConstraints};

/// Ask for the front camera and start playing it into `video`.
pub async fn start(video: &HtmlVideoElement) -> Result<MediaStream, String> {
    let devices = web_sys::window()
        .ok_or("no window")?
        .navigator()
        .media_devices()
        .map_err(|_| "This browser doesn't support camera access.".to_string())?;

    let constraints = MediaStreamConstraints::new();
    constraints.set_audio(&JsValue::FALSE);
    constraints.set_video(&video_constraints());

    let promise = devices
        .get_user_media_with_constraints(&constraints)
        .map_err(describe)?;
    let stream: MediaStream = JsFuture::from(promise)
        .await
        .map_err(describe)?
        .unchecked_into();

    // iOS Safari only plays inline (instead of fullscreen) with this attribute.
    let _ = video.set_attribute("playsinline", "");
    video.set_muted(true);
    video.set_src_object(Some(&stream));
    JsFuture::from(video.play().map_err(describe)?)
        .await
        .map_err(describe)?;
    Ok(stream)
}

/// `{ facingMode: "user", width: { ideal: 1280 }, height: { ideal: 720 } }`
fn video_constraints() -> JsValue {
    let ideal = |v: f64| {
        let o = Object::new();
        let _ = Reflect::set(&o, &"ideal".into(), &v.into());
        o
    };
    let video = Object::new();
    let _ = Reflect::set(&video, &"facingMode".into(), &"user".into());
    let _ = Reflect::set(&video, &"width".into(), &ideal(1280.0));
    let _ = Reflect::set(&video, &"height".into(), &ideal(720.0));
    video.into()
}

/// Turn a `DOMException` from getUserMedia into a friendly message.
fn describe(err: JsValue) -> String {
    let name = Reflect::get(&err, &"name".into())
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_default();
    match name.as_str() {
        "NotAllowedError" | "SecurityError" => {
            "Camera permission was denied. Allow camera access in your browser and try again."
                .into()
        }
        "NotFoundError" | "OverconstrainedError" => "No camera was found.".into(),
        "NotReadableError" => "The camera is in use by another app.".into(),
        _ => format!("Couldn't start the camera: {err:?}"),
    }
}
