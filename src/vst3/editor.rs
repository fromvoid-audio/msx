use std::{sync::Arc, time::Duration};

use toybox::{
    gpui::{
        AppContext, Context, Div, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
        MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement, Render,
        StatefulInteractiveElement, Styled, Task, Window, div, px, rgb,
    },
    gpui_gui::GpuiHostedGui,
    vst3::prelude::{IComponentHandler2, IComponentHandler2Trait, InstanceConnection},
};

use crate::{
    analyzer::{FLOOR_DB, MAX_BANDS, MIN_BANDS},
    session::{Action, Shared, Snapshot},
};

use super::controller::Handler;

pub const SIZE: (u32, u32) = (850, 453);

const BG: u32 = 0x14171b;
const PANEL: u32 = 0x1b2025;
const INSET: u32 = 0x20262c;
const GRID: u32 = 0x303941;
const TEXT: u32 = 0xe7edf1;
const MUTED: u32 = 0xa0abb6;
const DIM: u32 = 0x657382;
const HIGH: u32 = 0xf8a477;
const LOW: u32 = 0x81b6f3;
const NEAR: u32 = 0x6fd8bb;
const GRAPH_H: f32 = 144.0;
const GRAPH_TOP_DB: f32 = 6.0;
const GRAPH_TICKS: [i16; 4] = [0, -30, -60, -90];

type Focuses = [FocusHandle; MAX_BANDS + 5];
const OFFSET: usize = MAX_BANDS + 3;

struct Editor {
    connection: Arc<InstanceConnection<Shared>>,
    handler: Handler,
    connected: bool,
    snapshot: Snapshot,
    focus: Focuses,
    drag: Option<(usize, f32)>,
    _refresh: Task<()>,
}

impl Editor {
    fn refresh(&mut self) -> bool {
        let connected = self.connection.is_connected();
        let next = self.connection.shared().snapshot();
        let changed = connected != self.connected || next != self.snapshot;
        self.connected = connected;
        self.snapshot = next;
        changed
    }

    fn apply(&mut self, action: Action, cx: &mut Context<Self>) {
        if !self.connected {
            return;
        }
        let changed = self.connection.shared().apply(action);
        self.refresh();
        cx.notify();
        if changed {
            let handler = self.handler.borrow().clone();
            if let Some(handler) = handler.and_then(|handler| handler.cast::<IComponentHandler2>())
            {
                // setDirty is a host main-thread callback and can reenter the plug-in.
                unsafe { handler.setDirty(1) };
            }
        }
    }

    fn button(
        &self,
        index: usize,
        label: &str,
        action: Action,
        enabled: bool,
        primary: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let focus = self.focus[index].clone();
        let base = div()
            .id(("button", index))
            .h(px(30.0))
            .px(px(10.0))
            .flex()
            .items_center()
            .justify_center()
            .border_1()
            .rounded(px(5.0))
            .border_color(rgb(GRID))
            .bg(rgb(if primary && enabled { NEAR } else { INSET }))
            .text_size(px(12.0))
            .text_color(rgb(if !enabled {
                DIM
            } else if primary {
                BG
            } else {
                TEXT
            }))
            .child(label.to_owned());
        if !enabled {
            return base;
        }
        base.track_focus(&focus)
            .tab_stop(true)
            .tab_index(index as isize)
            .focus(|style| style.border_color(rgb(TEXT)))
            .hover(move |style| style.bg(rgb(if primary { 0x8ae7cd } else { GRID })))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                window.focus(&focus, cx)
            })
            .on_click(cx.listener(move |this, _, _, cx| this.apply(action, cx)))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "return" | "space") {
                    cx.stop_propagation();
                    this.apply(action, cx);
                }
            }))
    }

    fn stepper(
        &self,
        control: Stepper,
        label: String,
        editable: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let focus_index = control.focus_index();
        let focus = self.focus[focus_index].clone();
        let base = div()
            .id(("stepper", focus_index))
            .w(px(100.0))
            .h(px(28.0))
            .flex()
            .items_center()
            .justify_center()
            .border_1()
            .rounded(px(5.0))
            .border_color(rgb(GRID))
            .bg(rgb(PANEL))
            .text_size(px(11.0));
        let value = div().flex_1().text_center().child(label);
        if !editable {
            return base.text_color(rgb(MUTED)).child(value);
        }
        base.track_focus(&focus)
            .tab_stop(true)
            .tab_index(focus_index as isize)
            .focus(|style| style.border_color(rgb(NEAR)))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                let delta = match event.keystroke.key.as_str() {
                    "left" | "down" => -1,
                    "right" | "up" => 1,
                    _ => return,
                };
                let step = if event.keystroke.modifiers.shift {
                    4
                } else {
                    1
                };
                cx.stop_propagation();
                this.apply(control.action(delta * step), cx);
            }))
            .child(self.step_button(control, -1, cx))
            .child(value.id("value").cursor_pointer().on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    window.focus(&focus, cx);
                    if let Stepper::Crossover(index) = control {
                        this.drag = Some((index, f32::from(event.position.y)));
                    }
                }),
            ))
            .child(self.step_button(control, 1, cx))
    }

    fn step_button(&self, control: Stepper, delta: i8, cx: &mut Context<Self>) -> impl IntoElement {
        let id = if delta < 0 { "down" } else { "up" };
        div()
            .id(id)
            .w(px(22.0))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .text_color(rgb(MUTED))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(GRID)).text_color(rgb(TEXT)))
            .on_click(cx.listener(move |this, _, window, cx| {
                window.focus(&this.focus[control.focus_index()], cx);
                this.apply(control.action(delta), cx);
            }))
            .child(if delta < 0 { "−" } else { "+" })
    }

    fn drag_crossover(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some((index, start)) = self.drag else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            self.drag = None;
            return;
        }
        let steps = ((start - f32::from(event.position.y)) / 5.0) as i32;
        if steps != 0 {
            self.drag = Some((index, start - steps as f32 * 5.0));
            self.apply(Action::Crossover(index, steps), cx);
        }
    }
}

#[derive(Clone, Copy)]
enum Stepper {
    Crossover(usize),
    Offset,
    HoldTime,
}

impl Stepper {
    fn focus_index(self) -> usize {
        match self {
            Self::Crossover(index) => 4 + index,
            Self::Offset => OFFSET,
            Self::HoldTime => OFFSET + 1,
        }
    }

    fn action(self, step: i8) -> Action {
        match self {
            Self::Crossover(index) => Action::Crossover(index, i32::from(step)),
            Self::Offset => Action::Offset(step),
            Self::HoldTime => Action::HoldTime(step),
        }
    }
}

impl Render for Editor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = self.snapshot;
        let layout = s.preset.layout;
        let has_reference = s.preset.has_reference();
        let compare = has_reference && !s.capturing;
        let editable = self.connected && !s.capturing && !has_reference;
        let status = if !self.connected {
            "Waiting for audio processor".to_owned()
        } else if s.capturing && s.active_seconds == 0.0 {
            "Play the reference in your DAW".to_owned()
        } else if s.capturing {
            format!("Capturing · {:.1} s of audio", s.active_seconds)
        } else if has_reference {
            "Reference loaded".to_owned()
        } else {
            "No reference captured".to_owned()
        };
        let mut capture_controls = div().flex().items_center().gap(px(6.0));
        if s.capturing {
            capture_controls = capture_controls
                .child(self.button(0, "Use reference", Action::Save, s.ready, true, cx))
                .child(self.button(1, "Cancel", Action::Cancel, true, false, cx));
        } else {
            capture_controls = capture_controls.child(self.button(
                0,
                if has_reference {
                    "Capture again"
                } else {
                    "Capture reference"
                },
                Action::Begin,
                self.connected,
                true,
                cx,
            ));
            if has_reference {
                capture_controls = capture_controls.child(self.button(
                    1,
                    "Clear reference",
                    Action::Clear,
                    self.connected,
                    false,
                    cx,
                ));
            }
        }

        let mut crossovers = div().flex().items_center().gap(px(5.0));
        for (i, hz) in layout.cutoffs().iter().enumerate() {
            crossovers = crossovers.child(self.stepper(
                Stepper::Crossover(i),
                format!("{} Hz", hz),
                editable,
                cx,
            ));
        }

        div()
            .id("mxh-editor")
            .size_full()
            .p(px(16.0))
            .flex()
            .flex_col()
            .gap(px(10.0))
            .bg(rgb(BG))
            .text_color(rgb(TEXT))
            .on_mouse_move(cx.listener(Self::drag_crossover))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.drag = None),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.drag = None),
            )
            .child(
                div()
                    .h(px(42.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(13.0))
                            .child(
                                div()
                                    .text_size(px(25.0))
                                    .font_weight(toybox::gpui::FontWeight::SEMIBOLD)
                                    .child("MXH"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgb(if s.capturing { NEAR } else { MUTED }))
                                    .child(status),
                            ),
                    )
                    .child(capture_controls),
            )
            .child(meter(s, compare))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(5.0))
                            .child(
                                div()
                                    .w(px(80.0))
                                    .text_size(px(11.0))
                                    .text_color(rgb(MUTED))
                                    .child("Bands"),
                            )
                            .child(self.button(
                                2,
                                "−",
                                Action::Resize(false),
                                editable && layout.len() > MIN_BANDS,
                                false,
                                cx,
                            ))
                            .child(
                                div()
                                    .w(px(24.0))
                                    .text_center()
                                    .text_size(px(12.0))
                                    .child(layout.len().to_string()),
                            )
                            .child(self.button(
                                3,
                                "+",
                                Action::Resize(true),
                                editable && layout.len() < MAX_BANDS,
                                false,
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(rgb(MUTED))
                                    .child("Target offset"),
                            )
                            .child(self.stepper(
                                Stepper::Offset,
                                format!("{:+.1} dB", s.preset.offset_db()),
                                compare && self.connected,
                                cx,
                            ))
                            .child(
                                div()
                                    .ml(px(8.0))
                                    .text_size(px(11.0))
                                    .text_color(rgb(MUTED))
                                    .child("Peak hold"),
                            )
                            .child(self.stepper(
                                Stepper::HoldTime,
                                format!("{} ms", s.preset.hold_time.milliseconds()),
                                self.connected && !s.capturing,
                                cx,
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(5.0))
                    .child(
                        div()
                            .w(px(80.0))
                            .text_size(px(11.0))
                            .text_color(rgb(MUTED))
                            .child("Crossovers"),
                    )
                    .child(crossovers),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_end()
                    .justify_between()
                    .text_size(px(10.0))
                    .text_color(rgb(MUTED))
                    .child(if s.capturing && !s.ready {
                        "Capture at least one second of audible material."
                    } else if s.capturing {
                        "Use reference to save the peak in each band."
                    } else if has_reference {
                        "Raise a mix element until its band triggers, then back off until the orange fill clears."
                    } else {
                        "Set your crossovers, then play a representative passage."
                    })
                    .child(
                        option_env!("MXH_DEV_BUILD_VERSION").unwrap_or(env!("CARGO_PKG_VERSION")),
                    ),
            )
    }
}

fn meter(s: Snapshot, compare: bool) -> Div {
    let mut bands = div().flex().gap(px(5.0)).flex_1();
    let count = s.preset.layout.len();
    for band in 0..count {
        let (lo, hi) = s.preset.layout.bounds(band);
        let live = s.live[band];
        let marker = if s.capturing {
            s.peaks[band]
        } else {
            s.preset.threshold(band)
        };
        let triggered = compare && s.excess[band].is_some();
        let held = s.held[band];
        let peak_over = held.zip(marker).map(|(level, threshold)| level - threshold);
        bands = bands.child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_center()
                        .text_size(px(11.0))
                        .text_color(rgb(TEXT))
                        .child(format!("{}–{}", short_hz(lo), short_hz(hi))),
                )
                .child(band_graph(live, marker, triggered, held))
                .child(
                    div()
                        .text_center()
                        .text_size(px(14.0))
                        .text_color(rgb(if triggered { HIGH } else { TEXT }))
                        .child(absolute_db(live)),
                )
                .child(
                    div()
                        .text_center()
                        .text_size(px(10.0))
                        .text_color(rgb(NEAR))
                        .child(absolute_db(marker)),
                )
                .children(compare.then(|| {
                    div()
                        .text_center()
                        .text_size(px(10.0))
                        .text_color(rgb(if peak_over.is_some() { HIGH } else { MUTED }))
                        .child(
                            peak_over.map_or_else(|| "—".to_owned(), |db| format!("{db:+.1} dB")),
                        )
                })),
        );
    }
    let legend = div().text_size(px(10.0)).text_color(rgb(MUTED));
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .p(px(11.0))
        .rounded(px(7.0))
        .bg(rgb(PANEL))
        .border_1()
        .border_color(rgb(GRID))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(div().text_size(px(13.0)).child(if compare {
                    "Mix vs reference"
                } else {
                    "Band levels"
                }))
                .child(legend.child(if compare {
                    "dBFS RMS · Threshold includes +1 dB"
                } else {
                    "dBFS RMS"
                })),
        )
        .child(
            div()
                .flex()
                .gap(px(7.0))
                .child(
                    div()
                        .w(px(58.0))
                        .pt(px(17.0))
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(axis())
                        .child(
                            div()
                                .h(px(19.0))
                                .text_size(px(10.0))
                                .text_color(rgb(MUTED))
                                .child("Live"),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(rgb(NEAR))
                                .child(if s.capturing { "Peak" } else { "Threshold" }),
                        )
                        .children(compare.then(|| {
                            div()
                                .text_size(px(10.0))
                                .text_color(rgb(MUTED))
                                .child("Peak over")
                        })),
                )
                .child(bands),
        )
}

fn meter_y(db: f32) -> f32 {
    ((GRAPH_TOP_DB - db) / (GRAPH_TOP_DB - FLOOR_DB)).clamp(0.0, 1.0) * GRAPH_H
}

fn axis() -> Div {
    div()
        .relative()
        .h(px(GRAPH_H))
        .text_color(rgb(MUTED))
        .text_size(px(10.0))
        .children(GRAPH_TICKS.map(|db| {
            div()
                .absolute()
                .top(px((meter_y(f32::from(db)) - 6.0).clamp(0.0, GRAPH_H - 12.0)))
                .child(db.to_string())
        }))
}

fn band_graph(live: Option<f32>, marker: Option<f32>, triggered: bool, held: Option<f32>) -> Div {
    let mut graph = div()
        .relative()
        .h(px(GRAPH_H))
        .bg(rgb(BG))
        .rounded(px(4.0))
        .overflow_hidden()
        .children(GRAPH_TICKS.map(|db| line(meter_y(f32::from(db)), 1.0, GRID)));
    let bar = |top: f32, bottom: f32, color: u32| {
        div()
            .absolute()
            .left(px(9.0))
            .right(px(9.0))
            .top(px(top))
            .h(px(bottom - top))
            .bg(rgb(color))
    };
    if let Some(top) = live.map(meter_y).filter(|&top| top < GRAPH_H) {
        graph = graph.child(bar(top, GRAPH_H, LOW));
        if let Some(marker) = marker.filter(|_| triggered) {
            let boundary = meter_y(marker).max(top + 1.0).min(GRAPH_H);
            graph = graph.child(bar(top, boundary, HIGH));
        }
    }
    if let Some(db) = marker {
        graph = graph.child(line(meter_y(db).min(GRAPH_H - 2.0), 2.0, NEAR));
    }
    if let Some(db) = held {
        let top = meter_y(db).min(GRAPH_H - 2.0);
        graph = graph.child(bar(top, top + 2.0, HIGH));
    }
    graph
}

fn line(top: f32, height: f32, color: u32) -> Div {
    div()
        .absolute()
        .left_0()
        .right_0()
        .top(px(top))
        .h(px(height))
        .bg(rgb(color))
}

fn absolute_db(db: Option<f32>) -> String {
    db.map_or_else(|| "—".to_owned(), |value| format!("{value:.1}"))
}

fn short_hz(hz: u16) -> String {
    if hz < 1_000 {
        hz.to_string()
    } else if hz.is_multiple_of(1_000) {
        format!("{}k", hz / 1_000)
    } else {
        let value = format!("{:.2}", f32::from(hz) / 1_000.0);
        format!("{}k", value.trim_end_matches('0').trim_end_matches('.'))
    }
}

pub fn new(connection: Arc<InstanceConnection<Shared>>, handler: Handler) -> GpuiHostedGui {
    let size = SIZE;
    GpuiHostedGui::new(
        "MxhEditorView",
        move |window, cx| {
            let connection = Arc::clone(&connection);
            let handler = handler.clone();
            cx.new(move |cx| {
                let focus = std::array::from_fn(|_| cx.focus_handle());
                let refresh = cx.spawn_in(window, async move |weak, cx| {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_millis(40))
                            .await;
                        if weak
                            .update_in(cx, |editor: &mut Editor, _, cx| {
                                if editor.refresh() {
                                    cx.notify();
                                }
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                });
                let mut editor = Editor {
                    connection,
                    handler,
                    connected: false,
                    snapshot: Snapshot::default(),
                    focus,
                    drag: None,
                    _refresh: refresh,
                };
                editor.refresh();
                editor
            })
            .into()
        },
        size.0,
        size.1,
    )
    .with_size_contract(size, size, size)
}
