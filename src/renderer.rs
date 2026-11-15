//! Live preview: each animation frame, draw the video (mirrored) into an
//! offscreen canvas, run the selected Rust filter over the pixels, and paint
//! the result to the visible canvas.

use std::{cell::RefCell, rc::Rc};

use booth_core::Filter;
use js_sys::{Object, Reflect};
use wasm_bindgen::{Clamped, JsCast, prelude::Closure};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, HtmlVideoElement, ImageData};

/// Longest edge of the processed frame. Keeps per-frame work bounded on
/// high-resolution webcams.
const MAX_WIDTH: u32 = 1280;

/// A processed RGBA frame.
#[derive(Clone)]
pub struct Frame {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub struct Renderer {
    video: HtmlVideoElement,
    work: CanvasRenderingContext2d,
    out: CanvasRenderingContext2d,
    width: u32,
    height: u32,
    last: Option<Frame>,
}

impl Renderer {
    pub fn new(video: HtmlVideoElement, canvas: HtmlCanvasElement) -> Result<Self, String> {
        let (vw, vh) = (video.video_width().max(1), video.video_height().max(1));
        let width = vw.min(MAX_WIDTH);
        let height = vh * width / vw;

        let doc = web_sys::window()
            .and_then(|w| w.document())
            .ok_or("no document")?;
        let work_canvas: HtmlCanvasElement = doc
            .create_element("canvas")
            .map_err(|_| "couldn't create canvas")?
            .unchecked_into();
        for c in [&work_canvas, &canvas] {
            c.set_width(width);
            c.set_height(height);
        }

        Ok(Self {
            video,
            work: context_2d(&work_canvas, true)?,
            out: context_2d(&canvas, false)?,
            width,
            height,
            last: None,
        })
    }

    /// The most recent filtered frame, as shown on screen.
    pub fn last_frame(&self) -> Option<Frame> {
        self.last.clone()
    }

    fn draw(&mut self, filter: Filter) {
        let (w, h) = (self.width as f64, self.height as f64);
        // Mirror horizontally so it behaves like a mirror, as people expect.
        let _ = self.work.set_transform(-1.0, 0.0, 0.0, 1.0, w, 0.0);
        let _ =
            self.work
                .draw_image_with_html_video_element_and_dw_and_dh(&self.video, 0.0, 0.0, w, h);
        let _ = self.work.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);

        let Ok(image) = self.work.get_image_data(0.0, 0.0, w, h) else {
            return;
        };
        let mut rgba = image.data().0;
        filter.apply(&mut rgba);

        if let Ok(processed) =
            ImageData::new_with_u8_clamped_array_and_sh(Clamped(&rgba), self.width, self.height)
        {
            let _ = self.out.put_image_data(&processed, 0.0, 0.0);
        }
        self.last = Some(Frame {
            rgba,
            width: self.width,
            height: self.height,
        });
    }
}

/// Run the render loop until `filter` returns `None` (its signal was disposed).
pub fn start_loop(renderer: Rc<RefCell<Renderer>>, filter: impl Fn() -> Option<Filter> + 'static) {
    type Callback = Rc<RefCell<Option<Closure<dyn FnMut()>>>>;
    let callback: Callback = Rc::new(RefCell::new(None));
    let handle = callback.clone();

    *callback.borrow_mut() = Some(Closure::new(move || {
        let Some(f) = filter() else {
            handle.borrow_mut().take(); // drop the closure, ending the loop
            return;
        };
        renderer.borrow_mut().draw(f);
        request_frame(handle.borrow().as_ref().unwrap());
    }));
    request_frame(callback.borrow().as_ref().unwrap());
}

fn request_frame(cb: &Closure<dyn FnMut()>) {
    if let Some(w) = web_sys::window() {
        let _ = w.request_animation_frame(cb.as_ref().unchecked_ref());
    }
}

pub fn context_2d(
    canvas: &HtmlCanvasElement,
    will_read_frequently: bool,
) -> Result<CanvasRenderingContext2d, String> {
    let opts = Object::new();
    let _ = Reflect::set(
        &opts,
        &"willReadFrequently".into(),
        &will_read_frequently.into(),
    );
    canvas
        .get_context_with_context_options("2d", &opts)
        .ok()
        .flatten()
        .ok_or_else(|| "canvas 2D context unavailable".to_string())?
        .dyn_into()
        .map_err(|_| "canvas 2D context unavailable".to_string())
}
