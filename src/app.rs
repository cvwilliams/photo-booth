use std::{cell::RefCell, rc::Rc};

use booth_core::Filter;
use gloo_timers::future::TimeoutFuture;
use leptos::{html, prelude::*, task::spawn_local};

use crate::{
    animation, camera, export,
    recorder::{self, Clip},
    renderer::{self, Renderer},
};

/// Photos per strip.
const SHOTS: usize = 4;
/// Length of a video clip, in seconds. Keep in sync with `--clip` in style.css.
const CLIP_SECONDS: u32 = 6;
/// How long each photo shows in a photo GIF (hundredths of a second), so the
/// four shots loop over 4 seconds.
const PHOTO_GIF_DELAY_CS: u16 = 100;
/// Frame rate of a video GIF. GIF delays are in hundredths of a second, so
/// pick a rate that divides 100 evenly.
const VIDEO_GIF_FPS: u32 = 10;
/// Widest a GIF gets. Photo GIFs have only four frames, so they can be bigger.
const PHOTO_GIF_WIDTH: u32 = 640;
const VIDEO_GIF_WIDTH: u32 = 480;

#[derive(Clone, Debug, PartialEq)]
enum Phase {
    Idle,
    Starting,
    Live,
    Error(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Mode {
    #[default]
    Photo,
    Video,
}

/// What SNAP saves: the full-quality strip or video, or a looping GIF of it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Format {
    #[default]
    HiFi,
    Gif,
}

/// What the last SNAP produced.
#[derive(Clone, Debug, PartialEq)]
enum Take {
    /// PNG data URL of the photo strip.
    Strip(String),
    Clip(Clip),
    Gif(Clip),
}

impl Take {
    fn url(&self) -> &str {
        match self {
            Take::Strip(url) => url,
            Take::Clip(clip) | Take::Gif(clip) => &clip.url,
        }
    }

    fn file_name(&self) -> String {
        let ext = match self {
            Take::Strip(_) => "png",
            Take::Clip(clip) | Take::Gif(clip) => clip.ext,
        };
        format!("photo-booth-{}.{ext}", js_sys::Date::now() as u64)
    }
}

async fn count_down(secs: u32, set_countdown: WriteSignal<Option<u32>>) {
    for n in (1..=secs).rev() {
        set_countdown.set(Some(n));
        TimeoutFuture::new(1000).await;
    }
    set_countdown.set(None);
}

#[component]
pub fn App() -> impl IntoView {
    let video_ref = NodeRef::<html::Video>::new();
    let canvas_ref = NodeRef::<html::Canvas>::new();

    let (phase, set_phase) = signal(Phase::Idle);
    let (mode, set_mode) = signal(Mode::Photo);
    let (format, set_format) = signal(Format::HiFi);
    let (developing, set_developing) = signal(None::<u32>);
    let (filter, set_filter) = signal(Filter::None);
    let (countdown, set_countdown) = signal(None::<u32>);
    let (flash, set_flash) = signal(false);
    let (shooting, set_shooting) = signal(false);
    let (shots_taken, set_shots_taken) = signal(0usize);
    let (rec_left, set_rec_left) = signal(None::<u32>);
    let (take, set_take) = signal(None::<Take>);
    let (notice, set_notice) = signal(None::<String>);
    let (door_open, set_door_open) = signal(false);

    // The renderer isn't Send, so keep it in local (main-thread) storage.
    let slot = StoredValue::new_local(None::<Rc<RefCell<Renderer>>>);
    let is_live = move || phase.get() == Phase::Live;
    let is_video = move || mode.get() == Mode::Video;
    let is_gif = move || format.get() == Format::Gif;
    let recording = move || rec_left.get().is_some();
    // What a take is called on screen and in the marquee.
    let take_name = move || match (mode.get(), format.get()) {
        (_, Format::Gif) => "GIF",
        (Mode::Photo, Format::HiFi) => "STRIP",
        (Mode::Video, Format::HiFi) => "CLIP",
    };

    // Drop the current result (freeing a clip's memory) and any message.
    let clear_take = move || {
        if let Some(Take::Clip(clip) | Take::Gif(clip)) = take.get_untracked() {
            recorder::release(&clip);
        }
        set_take.set(None);
        set_notice.set(None);
        set_shots_taken.set(0);
    };

    let start_camera = move |_| {
        set_phase.set(Phase::Starting);
        spawn_local(async move {
            let (Some(video), Some(canvas)) =
                (video_ref.get_untracked(), canvas_ref.get_untracked())
            else {
                return;
            };
            let started = camera::start(&video)
                .await
                .and_then(|_stream| Renderer::new(video, canvas));
            match started {
                Ok(r) => {
                    let r = Rc::new(RefCell::new(r));
                    renderer::start_loop(r.clone(), move || filter.try_get_untracked());
                    slot.set_value(Some(r));
                    set_phase.set(Phase::Live);
                }
                Err(e) => set_phase.set(Phase::Error(e)),
            }
        });
    };

    let snap = move |_| {
        if shooting.get_untracked() {
            return;
        }
        let Some(r) = slot.get_value() else {
            return;
        };
        clear_take();
        set_shooting.set(true);
        let mode = mode.get_untracked();
        let format = format.get_untracked();
        let progress = move |pct: u32| set_developing.set(Some(pct));
        spawn_local(async move {
            let result = match (mode, format) {
                (Mode::Photo, _) => {
                    let mut frames = Vec::with_capacity(SHOTS);
                    for i in 0..SHOTS {
                        count_down(if i == 0 { 3 } else { 2 }, set_countdown).await;
                        if let Some(frame) = r.borrow().last_frame() {
                            frames.push(frame);
                        }
                        set_flash.set(true);
                        set_shots_taken.set(i + 1);
                        TimeoutFuture::new(180).await;
                        set_flash.set(false);
                        TimeoutFuture::new(320).await;
                    }
                    match format {
                        Format::HiFi => export::strip_data_url(&frames).map(Take::Strip),
                        Format::Gif => {
                            let frames = frames
                                .iter()
                                .map(|f| animation::shrink(f, PHOTO_GIF_WIDTH))
                                .collect();
                            animation::encode(frames, PHOTO_GIF_DELAY_CS, progress)
                                .await
                                .map(Take::Gif)
                        }
                    }
                }
                (Mode::Video, Format::Gif) => {
                    count_down(3, set_countdown).await;
                    let tick = move |left: u32| set_rec_left.set((left > 0).then_some(left));
                    let frames =
                        animation::capture(&r, CLIP_SECONDS, VIDEO_GIF_FPS, VIDEO_GIF_WIDTH, tick)
                            .await;
                    animation::encode(frames, (100 / VIDEO_GIF_FPS) as u16, progress)
                        .await
                        .map(Take::Gif)
                }
                (Mode::Video, Format::HiFi) => {
                    count_down(3, set_countdown).await;
                    match canvas_ref.get_untracked() {
                        Some(canvas) => {
                            let tick =
                                move |left: u32| set_rec_left.set((left > 0).then_some(left));
                            recorder::record(&canvas, CLIP_SECONDS, tick)
                                .await
                                .map(Take::Clip)
                        }
                        None => Err("The preview isn't ready.".to_string()),
                    }
                }
            };
            set_rec_left.set(None);
            set_developing.set(None);
            match result {
                Ok(t) => set_take.set(Some(t)),
                Err(e) => set_notice.set(Some(e)),
            }
            set_shooting.set(false);
        });
    };

    let toggle_mode = move |_| {
        if shooting.get_untracked() {
            return;
        }
        clear_take();
        set_mode.update(|m| {
            *m = match m {
                Mode::Photo => Mode::Video,
                Mode::Video => Mode::Photo,
            }
        });
    };

    let toggle_format = move |_| {
        if shooting.get_untracked() {
            return;
        }
        clear_take();
        set_format.update(|f| {
            *f = match f {
                Format::HiFi => Format::Gif,
                Format::Gif => Format::HiFi,
            }
        });
    };

    view! {
        <main class="arcade">
            <div class="cabinet">
                <header class="marquee">
                    <h1>"PHOTO BOOTH"</h1>
                    <p>
                        {move || if is_video() { "6 SECONDS · 1 " } else { "4 SHOTS · 1 " }}
                        {take_name}
                        " · NOTHING LEAVES YOUR DEVICE"
                    </p>
                </header>
                <div class="grill" aria-hidden="true"></div>

                <section class="monitor">
                    <div class="bezel">
                    <div class="screen" class:live=is_live class:recording=recording>
                        <video node_ref=video_ref class="source"></video>
                        <canvas node_ref=canvas_ref class="preview"></canvas>

                        <Show when=move || is_live() && (shooting.get() || take.with(Option::is_none))>
                            <div class="hud">
                                <span>
                                    "1P "
                                    {move || match (mode.get(), rec_left.get()) {
                                        (Mode::Video, Some(left)) => view! {
                                            <em class="t-red">"● REC " {left} "S"</em>
                                        }.into_any(),
                                        (Mode::Video, None) => view! {
                                            <em class="t-yellow">
                                                {move || if is_gif() { "GIF " } else { "VIDEO " }}
                                                {CLIP_SECONDS} "S"
                                            </em>
                                        }.into_any(),
                                        (Mode::Photo, _) => view! {
                                            <em class="t-yellow">{move || format!("SHOTS {}/{SHOTS}", shots_taken.get())}</em>
                                        }.into_any(),
                                    }}
                                </span>
                                <span>"FILTER " <em class="t-green">{move || filter.get().label().to_uppercase()}</em></span>
                            </div>
                        </Show>

                        {move || match phase.get() {
                            Phase::Idle => view! {
                                <div class="screen-msg">
                                    <p class="t-yellow">"INSERT SMILE"</p>
                                    <p class="blink">"PRESS 1P START"</p>
                                </div>
                            }.into_any(),
                            Phase::Starting => view! {
                                <div class="screen-msg"><p class="blink">"LOADING…"</p></div>
                            }.into_any(),
                            Phase::Live => ().into_any(),
                            Phase::Error(msg) => view! {
                                <div class="screen-msg">
                                    <p class="t-red">"GAME OVER"</p>
                                    <p class="small">{msg}</p>
                                    <p class="blink">"PRESS 1P START TO RETRY"</p>
                                </div>
                            }.into_any(),
                        }}

                        <Show when=move || is_live() && !shooting.get() && take.with(Option::is_none) && notice.with(Option::is_none)>
                            <p class="screen-prompt blink">
                                "PRESS " <em class="t-red">{move || if is_video() { "REC" } else { "SNAP" }}</em> " TO START"
                            </p>
                        </Show>

                        <Show when=recording>
                            <div class="rec-bar" aria-hidden="true"><span></span></div>
                        </Show>

                        {move || take.get().filter(|_| !shooting.get()).map(|t| match t {
                            Take::Strip(url) => view! {
                                <div class="strip-view">
                                    <img class="strip" src=url alt="Your photo strip" />
                                    <p class="screen-prompt">
                                        <em class="t-yellow">"STRIP READY!"</em>" PRESS PRINT TO SAVE"
                                    </p>
                                </div>
                            }.into_any(),
                            Take::Clip(clip) => view! {
                                <div class="clip-view">
                                    <video class="clip" src=clip.url autoplay=true loop=true prop:muted=true playsinline=true></video>
                                    <p class="screen-prompt">
                                        <em class="t-yellow">"CLIP READY!"</em>" PRESS PRINT TO SAVE"
                                    </p>
                                </div>
                            }.into_any(),
                            Take::Gif(gif) => view! {
                                <div class="clip-view">
                                    <img class="clip" src=gif.url alt="Your GIF" />
                                    <p class="screen-prompt">
                                        <em class="t-yellow">"GIF READY!"</em>" PRESS PRINT TO SAVE"
                                    </p>
                                </div>
                            }.into_any(),
                        })}

                        {move || developing.get().map(|pct| view! {
                            <div class="screen-msg developing">
                                <p class="t-yellow blink">"DEVELOPING…"</p>
                                <div class="dev-bar" aria-hidden="true">
                                    <span style:width=format!("{pct}%")></span>
                                </div>
                                <p class="small" role="status">{pct} "%"</p>
                            </div>
                        })}

                        {move || notice.get().map(|msg| view! {
                            <div class="screen-msg">
                                <p class="t-red">"TILT!"</p>
                                <p class="small">{msg}</p>
                                <p class="blink">"PRESS AGAIN"</p>
                            </div>
                        })}

                        <Show when=move || countdown.get().is_some()>
                            <div class="countdown">
                                <p class="t-yellow">
                                    {move || if shots_taken.get() == 0 {
                                        "GET READY!".to_string()
                                    } else {
                                        format!("SHOT {}/{SHOTS}", shots_taken.get() + 1)
                                    }}
                                </p>
                                <p class="count">{move || countdown.get().unwrap_or_default()}</p>
                            </div>
                        </Show>
                        <div class="flash" class:on=move || flash.get()></div>
                        <div class="scanlines" aria-hidden="true"></div>
                    </div>
                    </div>
                </section>

                <div class="bay">
                    // Behind the door: the cabinet's insides and the mode switch.
                    <section class="inside" aria-label="Service panel" inert=move || !door_open.get()>
                        <div class="sticker">
                            <p class="hazard" aria-hidden="true"></p>
                            <p>"OPERATOR"<br />"SETTINGS"</p>
                            <p class="fine">"AUTHORIZED SMILES ONLY"</p>
                        </div>

                        <div class="switches">
                            <Switch
                                name="Video mode"
                                up="PHOTO"
                                down="VIDEO"
                                lamp="red"
                                on=Signal::derive(is_video)
                                disabled=shooting
                                on_toggle=toggle_mode
                            />
                            <Switch
                                name="GIF mode"
                                up="HI-FI"
                                down="GIF"
                                lamp="cyan"
                                on=Signal::derive(is_gif)
                                disabled=shooting
                                on_toggle=toggle_format
                            />
                        </div>

                        <div class="control">
                            <div class="ph sm white">
                                <button class="pl" aria-label="Close service panel" on:click=move |_| set_door_open.set(false)></button>
                            </div>
                            <p class="label">"CLOSE"</p>
                        </div>
                    </section>

                    <div class="door" class:open=move || door_open.get() inert=move || door_open.get()>
                        <section class="panel" aria-label="Controls">
                            <button
                                class="keyhole"
                                aria-label="Open service panel"
                                aria-expanded=move || door_open.get().to_string()
                                disabled=move || shooting.get()
                                on:click=move |_| set_door_open.set(true)
                            ></button>

                            <div class="control start">
                                <div class="ph sm white" class:on=is_live>
                                    <button
                                        class="pl"
                                        aria-label="Start camera"
                                        disabled={move || matches!(phase.get(), Phase::Starting | Phase::Live)}
                                        on:click=start_camera
                                    ></button>
                                </div>
                                <p class="label">"1P START"<br />"CAMERA"</p>
                            </div>

                            <div class="filter-grid" role="group" aria-label="Filters">
                                {Filter::ALL
                                    .into_iter()
                                    .map(|f| {
                                        let colour = filter_colour(f);
                                        view! {
                                            <div class="control">
                                                <div class=format!("ph sm {colour}") class:on={move || is_live() && filter.get() == f}>
                                                    <button
                                                        class="pl"
                                                        aria-label=format!("{} filter", f.label())
                                                        aria-pressed={move || (filter.get() == f).to_string()}
                                                        disabled={move || !is_live()}
                                                        on:click=move |_| set_filter.set(f)
                                                    ></button>
                                                </div>
                                                <p class=format!("label t-{colour}")>{f.label().to_uppercase()}</p>
                                            </div>
                                        }
                                    })
                                    .collect_view()}
                            </div>

                            <div class="control snap">
                                <div class="ph lg red" class:on={move || shooting.get()} class:pulse={move || shooting.get()}>
                                    <button
                                        class="pl"
                                        aria-label=move || match (is_video(), is_gif()) {
                                            (true, false) => "Record a 6-second video",
                                            (true, true) => "Record a 6-second GIF",
                                            (false, false) => "Snap four photos",
                                            (false, true) => "Snap four photos for a GIF",
                                        }
                                        disabled={move || !is_live() || shooting.get()}
                                        on:click=snap
                                    ></button>
                                </div>
                                <p class="label big t-red">{move || if is_video() { "REC!" } else { "SNAP!" }}</p>
                            </div>

                            <div class="door-buttons">
                                <div class="control">
                                    <div class="ph sm yellow">
                                        {move || match take.get() {
                                            Some(t) => view! {
                                                <a class="pl" href=t.url().to_string() download=t.file_name() aria-label="Print (download)"></a>
                                            }.into_any(),
                                            None => view! {
                                                <button class="pl" disabled aria-label="Print (download)"></button>
                                            }.into_any(),
                                        }}
                                    </div>
                                    <p class="label">"PRINT"</p>
                                </div>
                                <div class="control">
                                    <div class="ph sm white">
                                        <button
                                            class="pl"
                                            aria-label="Play again"
                                            disabled={move || take.with(Option::is_none) && notice.with(Option::is_none)}
                                            on:click=move |_| clear_take()
                                        ></button>
                                    </div>
                                    <p class="label">"AGAIN"</p>
                                </div>
                            </div>
                        </section>

                        <div class="door-back" aria-hidden="true">
                            <p>"SERVICE PANEL"</p>
                        </div>
                    </div>
                </div>
            </div>
        </main>
    }
}

/// A chrome toggle switch on the service panel: the lever points up for the
/// first setting and flips down for the second, whose lamp lights in `lamp`.
#[component]
fn Switch(
    /// Accessible name; the switch is "on" when flipped down.
    name: &'static str,
    up: &'static str,
    down: &'static str,
    lamp: &'static str,
    #[prop(into)] on: Signal<bool>,
    #[prop(into)] disabled: Signal<bool>,
    on_toggle: impl FnMut(leptos::ev::MouseEvent) + 'static,
) -> impl IntoView {
    view! {
        <button
            class="toggle"
            class:down=on
            role="switch"
            aria-checked=move || on.get().to_string()
            aria-label=name
            disabled=disabled
            on:click=on_toggle
        >
            <span class="toggle-label" class:lit=move || !on.get()><i></i>{up}</span>
            <span class="toggle-plate" aria-hidden="true">
                <span class="nut"></span>
                <span class="bat"></span>
            </span>
            <span class=format!("toggle-label lamp-{lamp}") class:lit=on><i></i>{down}</span>
        </button>
    }
}

/// Arcade button colour for each filter; red is reserved for SNAP.
fn filter_colour(f: Filter) -> &'static str {
    match f {
        Filter::None => "white",
        Filter::Mono => "black",
        Filter::Sepia => "yellow",
        Filter::Noir => "purple",
        Filter::Invert => "blue",
        Filter::Pop => "green",
    }
}
