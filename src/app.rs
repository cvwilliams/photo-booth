use std::{cell::RefCell, rc::Rc};

use booth_core::Filter;
use gloo_timers::future::TimeoutFuture;
use leptos::{html, prelude::*, task::spawn_local};

use crate::{
    camera, export,
    renderer::{self, Renderer},
};

/// Photos per strip.
const SHOTS: usize = 4;

#[derive(Clone, Debug, PartialEq)]
enum Phase {
    Idle,
    Starting,
    Live,
    Error(String),
}

#[component]
pub fn App() -> impl IntoView {
    let video_ref = NodeRef::<html::Video>::new();
    let canvas_ref = NodeRef::<html::Canvas>::new();

    let (phase, set_phase) = signal(Phase::Idle);
    let (filter, set_filter) = signal(Filter::None);
    let (countdown, set_countdown) = signal(None::<u32>);
    let (flash, set_flash) = signal(false);
    let (shooting, set_shooting) = signal(false);
    let (shots_taken, set_shots_taken) = signal(0usize);
    let (strip, set_strip) = signal(None::<String>);

    // The renderer isn't Send, so keep it in local (main-thread) storage.
    let slot = StoredValue::new_local(None::<Rc<RefCell<Renderer>>>);
    let is_live = move || phase.get() == Phase::Live;

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

    let take_photos = move |_| {
        if shooting.get_untracked() {
            return;
        }
        let Some(r) = slot.get_value() else {
            return;
        };
        set_shooting.set(true);
        set_strip.set(None);
        set_shots_taken.set(0);
        spawn_local(async move {
            let mut frames = Vec::with_capacity(SHOTS);
            for i in 0..SHOTS {
                let secs = if i == 0 { 3 } else { 2 };
                for n in (1..=secs).rev() {
                    set_countdown.set(Some(n));
                    TimeoutFuture::new(1000).await;
                }
                set_countdown.set(None);
                if let Some(frame) = r.borrow().last_frame() {
                    frames.push(frame);
                }
                set_flash.set(true);
                set_shots_taken.set(i + 1);
                TimeoutFuture::new(180).await;
                set_flash.set(false);
                TimeoutFuture::new(320).await;
            }
            match export::strip_data_url(&frames) {
                Ok(url) => set_strip.set(Some(url)),
                Err(e) => set_phase.set(Phase::Error(e)),
            }
            set_shooting.set(false);
        });
    };

    let print_name = move || format!("photo-booth-{}.png", js_sys::Date::now() as u64);

    view! {
        <main class="arcade">
            <div class="cabinet">
                <header class="marquee">
                    <h1>"PHOTO BOOTH"</h1>
                    <p>"4 SHOTS · 1 STRIP · NOTHING LEAVES YOUR DEVICE"</p>
                </header>
                <div class="grill" aria-hidden="true"></div>

                <section class="monitor">
                    <div class="bezel">
                    <div class="screen" class:live=is_live>
                        <video node_ref=video_ref class="source"></video>
                        <canvas node_ref=canvas_ref class="preview"></canvas>

                        <Show when=move || is_live() && (shooting.get() || strip.with(Option::is_none))>
                            <div class="hud">
                                <span>"1P " <em class="t-yellow">{move || format!("SHOTS {}/{SHOTS}", shots_taken.get())}</em></span>
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

                        <Show when=move || is_live() && !shooting.get() && strip.with(Option::is_none)>
                            <p class="screen-prompt blink">"PRESS " <em class="t-red">"SNAP"</em> " TO START"</p>
                        </Show>
                        {move || strip.get().filter(|_| !shooting.get()).map(|url| view! {
                            <div class="strip-view">
                                <img class="strip" src=url alt="Your photo strip" />
                                <p class="screen-prompt">
                                    <em class="t-yellow">"STRIP READY!"</em>" PRESS PRINT TO SAVE"
                                </p>
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

                <section class="panel" aria-label="Controls">
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
                                aria-label="Snap four photos"
                                disabled={move || !is_live() || shooting.get()}
                                on:click=take_photos
                            ></button>
                        </div>
                        <p class="label big t-red">"SNAP!"</p>
                    </div>
                    <div class="door">
                        <div class="control">
                            <div class="ph sm yellow">
                                {move || match strip.get() {
                                    Some(url) => view! {
                                        <a class="pl" href=url download=print_name() aria-label="Print (download) strip"></a>
                                    }.into_any(),
                                    None => view! {
                                        <button class="pl" disabled aria-label="Print (download) strip"></button>
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
                                    disabled={move || strip.with(Option::is_none)}
                                    on:click=move |_| {
                                        set_strip.set(None);
                                        set_shots_taken.set(0);
                                    }
                                ></button>
                            </div>
                            <p class="label">"AGAIN"</p>
                        </div>
                    </div>

                </section>
            </div>
        </main>
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
