# Photo Booth

A photo booth that runs entirely in the browser, built with Rust, WebAssembly and [Leptos](https://leptos.dev). It's styled as an arcade cabinet: a CRT shows the live, filtered webcam preview, colour-coded plunger buttons pick the filter, and SNAP counts down, takes four photos and ejects a downloadable photo strip from the prize slot. Nothing leaves your device. Requires Docker.

## Develop

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

Camera access needs a secure context: `localhost` is fine for development, and a deployed site must be served over HTTPS.

## Deploy

Hosted on Firebase Hosting (project `photo-booth-41`, https://photo-booth-41.web.app).

- **Pull requests** run the same checks as `make build` and, once the deploy secret exists, publish a preview channel that expires after 7 days. The link is posted on the PR.
- **Pushes to `main`** run the checks again and deploy to the live site.

Deploys authenticate with a Firebase service account stored in the repo's `FIREBASE_SERVICE_ACCOUNT_PHOTO_BOOTH_41` Actions secret.

## Layout

```
crates/booth-core/   pure Rust, no browser APIs: filters and strip layout on RGBA buffers (unit-tested)
src/
  app.rs             Leptos UI and the capture state machine
  camera.rs          getUserMedia
  renderer.rs        per-frame loop: video → offscreen canvas → Rust filter → visible canvas
  export.rs          compose the strip, draw the caption, encode PNG
style.css            cabinet, CRT and the plunger buttons (colours are CSS custom properties)
assets/fonts/        Press Start 2P, bundled locally so no request goes to a font CDN (OFL licence alongside)
```

Each animation frame, the mirrored video frame is drawn into an offscreen canvas. The pixels are read back, the selected filter is applied in Rust, and the result is painted to the visible canvas. Captures take the most recent filtered frame, so photos match the preview exactly.

## Toolchain

Everything runs in the `rust:1-trixie` image with the `wasm32-unknown-unknown` target and a prebuilt [Trunk](https://trunkrs.dev). Trunk downloads matching `wasm-bindgen` and `wasm-opt` binaries on first build and caches them in a volume.

## Privacy

Everything happens in your browser. The camera feed, filters, photos and the finished strip never leave your device: there is no backend, no analytics and no third-party requests (the font is bundled). The hosting serves the app with a `Permissions-Policy` that allows only the camera, and only for this site.

## Browser support

Recent Chrome, Edge, Firefox and Safari, on desktop and mobile. Camera access needs HTTPS (or `localhost`) and your permission.

## Credits

- [Press Start 2P](https://fonts.google.com/specimen/Press+Start+2P) by The Press Start 2P Project Authors, under the [SIL Open Font License 1.1](assets/fonts/OFL.txt).
- Built with [Leptos](https://leptos.dev), [wasm-bindgen](https://github.com/rustwasm/wasm-bindgen) and [Trunk](https://trunkrs.dev).

## License

[MIT](LICENSE) © 2026 cvwilliams
