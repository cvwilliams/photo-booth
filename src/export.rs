//! Turn captured frames into a downloadable PNG strip.

use booth_core::{StripLayout, compose_strip};
use wasm_bindgen::{Clamped, JsCast, JsValue};
use web_sys::{HtmlCanvasElement, ImageData};

use crate::renderer::{Frame, context_2d};

/// Compose `frames` into a strip with a date caption and return a PNG data URL.
pub fn strip_data_url(frames: &[Frame]) -> Result<String, String> {
    let first = frames.first().ok_or("no frames captured")?;
    let (w, h) = (first.width, first.height);
    let layout = StripLayout {
        border: w / 20,
        gap: w / 32,
        footer: w / 8,
        ..StripLayout::default()
    };
    let buffers: Vec<&[u8]> = frames.iter().map(|f| f.rgba.as_slice()).collect();
    let (rgba, sw, sh) = compose_strip(&buffers, w, h, &layout);

    let canvas: HtmlCanvasElement = web_sys::window()
        .and_then(|w| w.document())
        .ok_or("no document")?
        .create_element("canvas")
        .map_err(|_| "couldn't create canvas")?
        .unchecked_into();
    canvas.set_width(sw);
    canvas.set_height(sh);
    let ctx = context_2d(&canvas, false)?;

    let image = ImageData::new_with_u8_clamped_array_and_sh(Clamped(&rgba), sw, sh)
        .map_err(|_| "couldn't build image")?;
    ctx.put_image_data(&image, 0.0, 0.0)
        .map_err(|_| "couldn't draw strip")?;

    // Caption
    let date = js_sys::Date::new_0()
        .to_locale_date_string("en-US", &JsValue::UNDEFINED)
        .as_string()
        .unwrap_or_default();
    let font_px = (layout.footer as f64 * 0.32).round();
    ctx.set_fill_style_str("#2b2620");
    ctx.set_font(&format!(
        "600 {font_px}px ui-rounded, 'SF Pro Rounded', system-ui, sans-serif"
    ));
    ctx.set_text_align("center");
    ctx.set_text_baseline("middle");
    let _ = ctx.fill_text(
        &format!("photo booth · {date}"),
        sw as f64 / 2.0,
        layout.caption_center_y(frames.len() as u32, h) as f64,
    );

    canvas
        .to_data_url_with_type("image/png")
        .map_err(|_| "couldn't encode PNG".to_string())
}
