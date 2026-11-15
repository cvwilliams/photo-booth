# Photo Booth

An arcade-cabinet photo booth that runs entirely in your browser, built with Rust, WebAssembly and [Leptos](https://leptos.dev).

**Try it: [photo-booth.cameronwillia.ms](https://photo-booth.cameronwillia.ms)**

- **Live filters:** the CRT screen shows your webcam with the selected filter applied every frame. Pick None, Mono, Sepia, Noir, Invert or Pop with the colour-coded plunger buttons.
- **Photo strips:** press SNAP for a countdown and four photos, then PRINT to download them as a captioned PNG strip.
- **Video mode:** unlock the keyhole to swing the control panel open, flip the PHOTO/VIDEO switch inside, and REC records a 6-second clip of the filtered preview (MP4 where the browser supports it, otherwise WebM).
- **GIF mode:** flip the HI-FI/GIF switch to get a looping GIF instead, encoded in Rust right in the browser: the four photos at a second each, or the 6-second video at 10 fps.
- **Private:** nothing leaves your device.

## Develop

The toolchain runs in Docker, so Docker and `make` are all you need.

```sh
make dev       # dev server with live reload at http://localhost:8080
make build     # fmt check, clippy, tests, then a release build to dist/
make preview   # serve the release build at http://localhost:8081
make test      # unit tests for the image-processing core
make lint      # clippy (wasm target)
make fmt       # format the code
make shell     # a shell inside the toolchain container
make clean     # remove containers, cache volumes and dist/
```

The first run builds the toolchain image and fills the cargo and target cache volumes, so it takes a few minutes. Later runs are fast.

The image is `rust:1-trixie` with the `wasm32-unknown-unknown` target and a prebuilt [Trunk](https://trunkrs.dev), which downloads matching `wasm-bindgen` and `wasm-opt` binaries on first build and caches them in a volume.

Camera access needs a secure context: `localhost` works for development, and anywhere else the app must be served over HTTPS.

## How it works

```
crates/booth-core/   pure Rust, no browser APIs: filters, strip layout and GIF encoding on RGBA buffers (unit-tested)
src/
  app.rs             Leptos UI and the capture state machine
  camera.rs          getUserMedia
  renderer.rs        per-frame loop: video → offscreen canvas → Rust filter → visible canvas
  export.rs          compose the strip, draw the caption, encode PNG
  recorder.rs        record the preview canvas with MediaRecorder
  animation.rs       grab preview frames and encode them as a GIF, yielding between frames
style.css            cabinet, CRT and the plunger buttons (colours are CSS custom properties)
assets/fonts/        Press Start 2P, bundled locally (OFL licence alongside)
```

Each animation frame, the mirrored video frame is drawn into an offscreen canvas. The pixels are read back, the selected filter is applied in Rust, and the result is painted to the visible canvas. Photos take the most recent filtered frame and videos record that canvas, so what you capture matches the preview exactly.

## Privacy

The camera feed, photos and videos never leave your device. There's no backend, no analytics and no third-party requests (even the font is bundled). The site is served with a `Permissions-Policy` that allows only the camera, and only for this site.

## Browser support

Recent Chrome, Edge, Firefox and Safari, on desktop and mobile. You'll be asked for camera permission on first use.

## Credits

- [Press Start 2P](https://fonts.google.com/specimen/Press+Start+2P) by The Press Start 2P Project Authors, under the [SIL Open Font License 1.1](assets/fonts/OFL.txt).
- Built with [Leptos](https://leptos.dev), [wasm-bindgen](https://github.com/rustwasm/wasm-bindgen) and [Trunk](https://trunkrs.dev).

## License

[MIT](LICENSE) © 2026 cvwilliams
