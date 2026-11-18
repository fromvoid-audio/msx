//! Capture and project state. The audio thread only touches its triple-buffer endpoints.

use std::{
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

use triple_buffer::{Input, Output, triple_buffer};

use crate::analyzer::{
    Command, FLOOR_DB, Layout, Levels, MAX_BANDS, MIN_TARGET_DB, Report, TRIGGER_DB,
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

    /// Version 2 stored the same peak preset followed by the comparison
    /// mode and timestamped measurements. Preserve the peak preset on load.
    pub fn decode_timeline_state(bytes: &[u8]) -> Option<Self> {
        let (preset, extension) = bytes.split_at_checked(STATE_BYTES)?;
        let (&mode, frames) = extension.split_first()?;
        const LEGACY_FRAME_BYTES: usize = 16 + MAX_BANDS * 4;
        if mode > 1
            || !frames.len().is_multiple_of(LEGACY_FRAME_BYTES)
            || (mode == 1 && frames.is_empty())
        {
            return None;
        }
        Self::decode(preset)
    }

    fn excess(self, live: Levels) -> Levels {
        std::array::from_fn(|band| {
            let (level, target) = live[band].zip(self.target(band))?;
            let excess = level - target;
            (level >= MIN_TARGET_DB && excess > TRIGGER_DB).then_some(excess)
        })
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

#[derive(Clone, Copy, Debug)]
pub enum Action {
    Begin,
    Save,
    Cancel,
    Clear,
    Resize(bool),
    Crossover(usize, i32),
    Offset(i8),
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
}

pub struct Shared(Mutex<Session>);

pub struct AudioLink {
    pub commands: Output<Command>,
    pub reports: Input<Report>,
}

struct Session {
    preset: Preset,
    capture: Option<u64>,
    last_capture: u64,
    commands: Input<Command>,
    reports: Output<Report>,
    last_update: Option<Instant>,
}

impl Shared {
    pub fn new() -> (Self, AudioLink) {
        let (control_out, audio_in) = triple_buffer(&Command::default());
        let (audio_out, control_in) = triple_buffer(&Report::default());
        let session = Session {
            preset: Preset::default(),
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
            },
        )
    }

    fn lock(&self) -> MutexGuard<'_, Session> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn preset(&self) -> Preset {
        self.lock().preset
    }

    pub fn restore(&self, preset: Preset) {
        let mut session = self.lock();
        session.preset = preset;
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
        let preset = session.preset;
        let capturing = session.capture.is_some();
        let compare = !capturing && preset.has_reference();
        let live = if current && report.audible && report.command.layout == preset.layout {
            report.live
        } else {
            [None; MAX_BANDS]
        };
        Snapshot {
            preset,
            capturing,
            active_seconds: if valid_capture {
                report.active_seconds
            } else {
                0.0
            },
            ready: valid_capture && report.ready(),
            live: if compare { [None; MAX_BANDS] } else { live },
            excess: if compare {
                preset.excess(live)
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
        let previous = session.preset;
        let before = session.command();
        let idle = session.capture.is_none();
        let editable = idle && !previous.has_reference();
        match action {
            Action::Begin if idle => {
                session.last_capture = session.last_capture.wrapping_add(1);
                session.capture = Some(session.last_capture);
            }
            Action::Cancel => session.capture = None,
            Action::Save if !idle => {
                let report = session.report();
                if report.command == before && report.ready() {
                    session.preset.targets = report.peaks;
                    session.preset.offset_steps = 0;
                    session.capture = None;
                }
            }
            Action::Clear if idle => {
                session.preset.targets = [None; MAX_BANDS];
                session.preset.offset_steps = 0;
            }
            Action::Resize(grow) if editable => session.preset.layout.resize(grow),
            Action::Crossover(index, steps) if editable => {
                session.preset.layout.adjust(index, steps)
            }
            Action::Offset(steps) if idle && previous.has_reference() => {
                session.preset.offset_steps = session
                    .preset
                    .offset_steps
                    .saturating_add(steps)
                    .clamp(-OFFSET_LIMIT, OFFSET_LIMIT);
            }
            _ => {}
        }
        if before != session.command() {
            session.publish();
        }
        session.preset != previous
    }
}

impl Session {
    fn command(&self) -> Command {
        Command {
            layout: self.preset.layout,
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
        *self.reports.read()
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
        let preset = shared.preset();
        assert_eq!(preset.target(0), Some(-21.5));
        assert_eq!(Preset::decode(&preset.encode()), Some(preset));
        assert!(!shared.apply(Action::Resize(true)));

        shared.apply(Action::Begin);
        // The old capture's report still occupies the mailbox.
        assert!(!shared.apply(Action::Save));
        shared.apply(Action::Cancel);
        assert_eq!(shared.preset(), preset);

        assert!(shared.apply(Action::Clear));
        assert!(shared.apply(Action::Resize(true)));
        shared.restore(preset);
        assert_eq!(shared.preset(), preset);
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
    fn timeline_preset_migrates_to_its_constant_peaks() {
        let mut preset = Preset::default();
        preset.targets[0] = Some(-20.0);
        preset.offset_steps = 2;
        // Original v2 encoding: v1 preset, mode byte, then a timed frame
        // with a deliberately different level from the saved peak.
        let mut old = preset.encode().to_vec();
        old.push(1);
        old.extend(48_000.0_f64.to_le_bytes());
        old.extend(8192_i64.to_le_bytes());
        old.extend((-38.0_f32).to_le_bytes());
        for _ in 1..MAX_BANDS {
            old.extend(f32::NEG_INFINITY.to_le_bytes());
        }
        let restored = Preset::decode_timeline_state(&old).unwrap();
        assert_eq!(restored, preset);
        for (level, expected) in [
            (-29.0, None),
            (-19.0, None),
            (-18.0, None),
            (-17.5, Some(1.5)),
            (-8.0, Some(11.0)),
        ] {
            let mut live = [None; MAX_BANDS];
            live[0] = Some(level);
            assert_eq!(restored.excess(live)[0], expected);
        }
        assert!(Preset::decode_timeline_state(&old[..old.len() - 1]).is_none());
    }
}
