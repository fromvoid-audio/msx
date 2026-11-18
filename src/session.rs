//! Capture and project state. The audio thread communicates through bounded channels.

use std::{
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use rtrb::{Consumer, Producer, RingBuffer};
use triple_buffer::{Input, Output, triple_buffer};

use crate::{
    analyzer::{Command, FLOOR_DB, Layout, Levels, MAX_BANDS, MIN_TARGET_DB, Report, TRIGGER_DB},
    timeline::{FRAME_BYTES, MAX_TIMELINE_FRAMES, TimelineFrame},
};

const OFFSET_LIMIT: i8 = 48; // Half-decibel steps.
const STATE_BYTES: usize = (MAX_BANDS - 1) * 2 + 1 + MAX_BANDS * 4;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Preset {
    pub layout: Layout,
    pub targets: Levels,
    offset_steps: i8,
}

impl Preset {
    pub fn has_reference(self) -> bool {
        self.targets.iter().any(Option::is_some)
    }

    pub fn offset_db(self) -> f32 {
        f32::from(self.offset_steps) * 0.5
    }

    pub fn target(self, band: usize) -> Option<f32> {
        self.targets[band].map(|level| level + self.offset_db())
    }

    pub fn encode(self) -> [u8; STATE_BYTES] {
        let mut bytes = [0; STATE_BYTES];
        let (cut_bytes, rest) = bytes.split_at_mut((MAX_BANDS - 1) * 2);
        let (offset, target_bytes) = rest.split_first_mut().expect("preset has an offset byte");
        for (slot, cut) in cut_bytes
            .as_chunks_mut::<2>()
            .0
            .iter_mut()
            .zip(self.layout.encoded_cuts())
        {
            *slot = cut.to_le_bytes();
        }
        *offset = self.offset_steps as u8;
        for (slot, target) in target_bytes
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(self.targets)
        {
            *slot = target.unwrap_or(f32::NEG_INFINITY).to_le_bytes();
        }
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let bytes: &[u8; STATE_BYTES] = bytes.try_into().ok()?;
        let (cut_bytes, rest) = bytes.split_at((MAX_BANDS - 1) * 2);
        let (offset, target_bytes) = rest.split_first()?;
        let cut_chunks = cut_bytes.as_chunks::<2>().0;
        let layout = Layout::from_cuts(std::array::from_fn(|i| u16::from_le_bytes(cut_chunks[i])))?;
        let offset_steps = *offset as i8;
        if !(-OFFSET_LIMIT..=OFFSET_LIMIT).contains(&offset_steps) {
            return None;
        }
        let mut targets = [None; MAX_BANDS];
        for (index, (slot, &chunk)) in targets
            .iter_mut()
            .zip(target_bytes.as_chunks::<4>().0)
            .enumerate()
        {
            let value = f32::from_le_bytes(chunk);
            if value == f32::NEG_INFINITY {
                continue;
            }
            // A valid f32 signal cannot exceed this analysis level.
            if index >= layout.len() || !(FLOOR_DB..=800.0).contains(&value) {
                return None;
            }
            *slot = Some(value);
        }
        let preset = Self {
            layout,
            targets,
            offset_steps,
        };
        (offset_steps == 0 || preset.has_reference()).then_some(preset)
    }
}

/// Host-persisted levels and the user's comparison mode. Old states are peak-only.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SavedState {
    pub preset: Preset,
    aligned: bool,
    timeline: Arc<[TimelineFrame]>,
}

impl SavedState {
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(STATE_BYTES + 1 + self.timeline.len() * FRAME_BYTES);
        bytes.extend(self.preset.encode());
        bytes.push(u8::from(self.aligned));
        for frame in self.timeline.iter() {
            frame.encode(&mut bytes);
        }
        bytes
    }

    pub fn decode(version: u32, bytes: &[u8]) -> Option<Self> {
        if version == 1 {
            return Some(Self {
                preset: Preset::decode(bytes)?,
                ..Self::default()
            });
        }
        let (old, rest) = bytes.split_at_checked(STATE_BYTES)?;
        let preset = Preset::decode(old)?;
        let (&mode, frames) = rest.split_first()?;
        if mode > 1
            || !frames.len().is_multiple_of(FRAME_BYTES)
            || frames.len() / FRAME_BYTES > MAX_TIMELINE_FRAMES
            || (mode == 1 && frames.is_empty())
            || (!frames.is_empty() && !preset.has_reference())
        {
            return None;
        }
        let timeline: Vec<_> = frames
            .as_chunks::<FRAME_BYTES>()
            .0
            .iter()
            .map(|frame| TimelineFrame::decode(frame, preset.layout.len()))
            .collect::<Option<_>>()?;
        if !timeline.windows(2).all(|pair| pair[0].time < pair[1].time) {
            return None;
        }
        Some(Self {
            preset,
            aligned: mode == 1,
            timeline: timeline.into(),
        })
    }

    fn aligned_levels(&self, report: Report) -> Option<Levels> {
        let time = report.time?;
        let frame = self
            .timeline
            .binary_search_by_key(&time, |frame| frame.time)
            .ok()?;
        Some(self.timeline[frame].levels())
    }

    fn excess(&self, report: Report, aligned_levels: Option<Levels>) -> Levels {
        let (live, target) = if self.aligned {
            let Some(target) = aligned_levels else {
                return [None; MAX_BANDS];
            };
            (report.window, target)
        } else if report.audible {
            (report.live, self.preset.targets)
        } else {
            return [None; MAX_BANDS];
        };
        std::array::from_fn(|band| {
            let (live, target) = live[band].zip(target[band])?;
            let excess = live - target.max(MIN_TARGET_DB) - self.preset.offset_db();
            (live >= MIN_TARGET_DB && excess > TRIGGER_DB).then_some(excess)
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Action {
    Begin,
    Save,
    Cancel,
    Clear,
    Resize(bool),
    Crossover(usize, i32),
    Offset(i8),
    ToggleComparison,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub preset: Preset,
    pub capturing: bool,
    pub active_seconds: f64,
    pub ready: bool,
    pub live: Levels,
    pub peaks: Levels,
    pub excess: Levels,
    pub aligned: bool,
    pub can_align: bool,
    pub aligned_frame: bool,
    pub timeline_full: bool,
}

pub struct Shared(Mutex<Session>);

pub struct AudioLink {
    pub commands: Output<Command>,
    pub reports: Input<Report>,
    frames: Producer<(u64, TimelineFrame)>,
}

impl AudioLink {
    pub fn record(&mut self, report: Report) {
        if let (Some(capture), Some(time)) = (report.command.capture, report.time) {
            let _ = self
                .frames
                .push((capture, TimelineFrame::new(time, report.window)));
        }
    }
}

struct Session {
    saved: SavedState,
    captured: Vec<TimelineFrame>,
    frames: Consumer<(u64, TimelineFrame)>,
    capture: Option<u64>,
    last_capture: u64,
    commands: Input<Command>,
    reports: Output<Report>,
    last_update: Option<Instant>,
}

impl Shared {
    pub fn new() -> (Self, AudioLink) {
        Self::with_capacity(MAX_TIMELINE_FRAMES)
    }

    pub fn detached() -> Self {
        Self::with_capacity(1).0
    }

    fn with_capacity(capacity: usize) -> (Self, AudioLink) {
        let (control_out, audio_in) = triple_buffer(&Command::default());
        let (audio_out, control_in) = triple_buffer(&Report::default());
        let (frames_out, frames_in) = RingBuffer::new(capacity);
        let session = Session {
            saved: SavedState::default(),
            captured: Vec::new(),
            frames: frames_in,
            capture: None,
            last_capture: 0,
            commands: control_out,
            reports: control_in,
            last_update: None,
        };
        (
            Self(Mutex::new(session)),
            AudioLink {
                commands: audio_in,
                reports: audio_out,
                frames: frames_out,
            },
        )
    }

    fn lock(&self) -> MutexGuard<'_, Session> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn state(&self) -> SavedState {
        self.lock().saved.clone()
    }

    pub fn restore(&self, state: SavedState) {
        let mut session = self.lock();
        session.saved = state;
        session.captured.clear();
        session.capture = None;
        session.publish();
    }

    pub fn snapshot(&self) -> Snapshot {
        let mut session = self.lock();
        let report = session.report();
        let current = session
            .last_update
            .is_some_and(|time| time.elapsed() < Duration::from_millis(500));
        let valid_capture = session.capture.is_some() && report.command == session.command();
        let preset = session.saved.preset;
        let capturing = session.capture.is_some();
        let compare = !capturing && preset.has_reference();
        let valid_meter = current && report.command.layout == preset.layout;
        let aligned_levels = if compare && valid_meter && session.saved.aligned {
            session.saved.aligned_levels(report)
        } else {
            None
        };
        Snapshot {
            preset,
            aligned_frame: aligned_levels.is_some(),
            aligned: session.saved.aligned,
            can_align: !session.saved.timeline.is_empty(),
            timeline_full: session.captured.len() == MAX_TIMELINE_FRAMES,
            excess: if compare && valid_meter {
                session.saved.excess(report, aligned_levels)
            } else {
                [None; MAX_BANDS]
            },
            capturing,
            active_seconds: if valid_capture {
                report.active_seconds
            } else {
                0.0
            },
            ready: valid_capture && report.ready(),
            live: if !compare && valid_meter && report.audible {
                report.live
            } else {
                [None; MAX_BANDS]
            },
            peaks: if valid_capture {
                report.peaks
            } else {
                [None; MAX_BANDS]
            },
        }
    }

    /// Returns whether the operation changed the host's persistent state.
    pub fn apply(&self, action: Action) -> bool {
        let mut session = self.lock();
        let previous = session.saved.preset;
        let was_aligned = session.saved.aligned;
        let mut reference_changed = false;
        let before = session.command();
        let idle = session.capture.is_none();
        let editable = idle && !previous.has_reference();
        match action {
            Action::Begin if idle => {
                session.captured.clear();
                session.last_capture = session.last_capture.wrapping_add(1);
                session.capture = Some(session.last_capture);
            }
            Action::Cancel => {
                session.capture = None;
                session.captured.clear();
            }
            Action::Save if !idle => {
                let report = session.report();
                if report.command == before && report.ready() {
                    let mut captured = std::mem::take(&mut session.captured);
                    captured.sort_by_key(|frame| frame.time);
                    captured.dedup_by_key(|frame| frame.time);
                    session.saved.aligned = !captured.is_empty();
                    session.saved.timeline = captured.into();
                    session.saved.preset.targets = report.peaks;
                    session.saved.preset.offset_steps = 0;
                    session.capture = None;
                    reference_changed = true;
                }
            }
            Action::Clear if idle => {
                session.saved.timeline = Arc::default();
                session.saved.aligned = false;
                session.saved.preset.targets = [None; MAX_BANDS];
                session.saved.preset.offset_steps = 0;
            }
            Action::Resize(grow) if editable => session.saved.preset.layout.resize(grow),
            Action::Crossover(index, steps) if editable => {
                session.saved.preset.layout.adjust(index, steps)
            }
            Action::Offset(steps) if idle && previous.has_reference() => {
                session.saved.preset.offset_steps = session
                    .saved
                    .preset
                    .offset_steps
                    .saturating_add(steps)
                    .clamp(-OFFSET_LIMIT, OFFSET_LIMIT);
            }
            Action::ToggleComparison if idle && !session.saved.timeline.is_empty() => {
                session.saved.aligned = !session.saved.aligned;
            }
            _ => {}
        }
        if before != session.command() {
            session.publish();
        }
        reference_changed
            || session.saved.preset != previous
            || session.saved.aligned != was_aligned
    }
}

impl Session {
    fn command(&self) -> Command {
        Command {
            layout: self.saved.preset.layout,
            capture: self.capture,
        }
    }

    fn publish(&mut self) {
        self.commands.write(self.command());
    }

    fn report(&mut self) -> Report {
        if self.reports.updated() {
            self.last_update = Some(Instant::now());
        }
        let report = *self.reports.read();
        // The audio side enqueues each frame before publishing its report.
        // Keep only the active capture, even if commands change mid-block.
        while let Ok((capture, frame)) = self.frames.pop() {
            if Some(capture) == self.capture && self.captured.len() < MAX_TIMELINE_FRAMES {
                self.captured.push(frame);
            }
        }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::MIN_CAPTURE_SECONDS;

    #[test]
    fn capture_can_be_saved_recalled_and_replaced_without_losing_the_previous_reference() {
        let (shared, mut audio) = Shared::new();
        assert!(shared.apply(Action::Crossover(0, 2)));
        shared.apply(Action::Begin);
        assert!(!shared.apply(Action::Save));
        let command = *audio.commands.read();
        let mut peaks = [None; MAX_BANDS];
        peaks[0] = Some(-20.0);
        peaks[1] = Some(-11.5);
        audio.reports.write(Report {
            command,
            peaks,
            active_seconds: MIN_CAPTURE_SECONDS,
            ..Report::default()
        });
        assert!(shared.apply(Action::Save));
        assert!(shared.apply(Action::Offset(-3)));
        let state = shared.state();
        let preset = state.preset;
        assert_eq!(preset.target(0), Some(-21.5));
        assert_eq!(Preset::decode(&preset.encode()), Some(preset));
        assert!(!shared.apply(Action::Resize(true)));

        shared.apply(Action::Begin);
        // The old capture's report still occupies the mailbox.
        assert!(!shared.apply(Action::Save));
        shared.apply(Action::Cancel);
        assert_eq!(shared.state(), state);

        assert!(shared.apply(Action::Clear));
        assert!(shared.apply(Action::Resize(true)));
        shared.restore(state.clone());
        assert_eq!(shared.state(), state);
    }

    #[test]
    fn malformed_state_cannot_construct_a_preset() {
        let mut bytes = Preset::default().encode();
        assert!(Preset::decode(&bytes[..bytes.len() - 1]).is_none());
        bytes[..2].copy_from_slice(&1_u16.to_le_bytes());
        assert!(Preset::decode(&bytes).is_none());
        let mut bytes = Preset::default().encode();
        bytes[15..19].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(Preset::decode(&bytes).is_none());
    }

    #[test]
    fn legacy_peak_reference_is_preserved_and_only_excess_triggers() {
        let mut preset = Preset::default();
        preset.targets[0] = Some(-20.0);
        let state = SavedState::decode(1, &preset.encode()).unwrap();
        assert_eq!(state.preset, preset);
        assert!(!state.aligned);
        assert_eq!(SavedState::decode(2, &state.encode()), Some(state.clone()));
        for (db, expected) in [
            (-29.0, None),
            (-20.0, None),
            (-19.0, None),
            (-18.5, Some(1.5)),
            (-9.0, Some(11.0)),
        ] {
            let mut report = Report {
                audible: true,
                ..Report::default()
            };
            report.live[0] = Some(db);
            assert_eq!(state.excess(report, None)[0], expected);
        }
    }
}
