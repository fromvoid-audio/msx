//! Stereo band power analysis. The input is observed without modifying it.

use std::{f64::consts::TAU, sync::Arc};

use rustfft::{Fft, FftPlanner, num_complex::Complex};

pub const MIN_BANDS: usize = 2;
pub const MAX_BANDS: usize = 8;
pub const LOW_HZ: u16 = 20;
pub const HIGH_HZ: u16 = 20_000;
pub const FLOOR_DB: f32 = -90.0;
pub const MIN_CAPTURE_SECONDS: f64 = 1.0;
pub const TRIGGER_DB: f32 = 1.0;
const ACTIVE_POWER: f64 = 3.162_277_660e-8; // −75 dBFS
pub const MIN_TARGET_DB: f32 = -84.0;
const SMOOTH_SECONDS: f64 = 0.15;
const MIN_CUT: u16 = 35;
const MAX_CUT: u16 = 16_000;

pub type Levels = [Option<f32>; MAX_BANDS];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FrameTime {
    pub sample_rate_bits: u64,
    pub end_sample: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    cuts: [u16; MAX_BANDS - 1],
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            cuts: [60, 160, 500, 2_000, 6_000, 0, 0],
        }
    }
}

impl Layout {
    pub fn from_cuts(cuts: [u16; MAX_BANDS - 1]) -> Option<Self> {
        let layout = Self { cuts };
        let used = layout.cutoffs();
        (!used.is_empty()
            && used.iter().all(|freq| (MIN_CUT..=MAX_CUT).contains(freq))
            && used.windows(2).all(|pair| separated(pair[0], pair[1]))
            && cuts[used.len()..].iter().all(|freq| *freq == 0))
        .then_some(layout)
    }

    pub fn cutoffs(&self) -> &[u16] {
        let count = self
            .cuts
            .iter()
            .position(|&cut| cut == 0)
            .unwrap_or(self.cuts.len());
        &self.cuts[..count]
    }

    pub fn encoded_cuts(&self) -> [u16; MAX_BANDS - 1] {
        self.cuts
    }

    pub fn len(&self) -> usize {
        self.cutoffs().len() + 1
    }

    pub fn bounds(&self, band: usize) -> (u16, u16) {
        let cuts = self.cutoffs();
        let low = if band == 0 { LOW_HZ } else { cuts[band - 1] };
        let high = cuts.get(band).copied().unwrap_or(HIGH_HZ);
        (low, high)
    }

    pub fn resize(&mut self, grow: bool) {
        let len = self.len();
        if grow && len < MAX_BANDS {
            // Divide the widest logarithmic band, including the end bands.
            let band = (0..len)
                .max_by(|&a, &b| {
                    let ratio = |i| {
                        let (lo, hi) = self.bounds(i);
                        f64::from(hi) / f64::from(lo)
                    };
                    ratio(a).total_cmp(&ratio(b))
                })
                .expect("the layout always contains bands");
            let (low, high) = self.bounds(band);
            let split = (f64::from(low) * f64::from(high)).sqrt().round() as u16;
            self.cuts.copy_within(band..len - 1, band + 1);
            self.cuts[band] = split.clamp(MIN_CUT, MAX_CUT);
        } else if !grow && len > MIN_BANDS {
            self.cuts[len - 2] = 0;
        }
    }

    pub fn adjust(&mut self, idx: usize, steps: i32) {
        let cuts = self.cutoffs();
        let Some(&current) = cuts.get(idx) else {
            return;
        };
        let min = if idx == 0 {
            MIN_CUT
        } else {
            (u32::from(cuts[idx - 1]) * 5).div_ceil(4) as u16
        };
        let max = cuts.get(idx + 1).map_or(MAX_CUT, |&next| next * 4 / 5);
        let proposed = (f64::from(current) * 2.0_f64.powf(f64::from(steps) / 12.0)).round();
        self.cuts[idx] = (proposed as u16).clamp(min, max);
    }
}

// Keep narrow bands measurable and prevent crossover handles from crossing.
fn separated(low: u16, high: u16) -> bool {
    u32::from(low) * 5 <= u32::from(high) * 4
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Command {
    pub layout: Layout,
    pub capture: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Report {
    pub command: Command,
    pub audible: bool,
    pub live: Levels,
    pub window: Levels,
    pub time: Option<FrameTime>,
    pub peaks: Levels,
    pub active_seconds: f64,
}

impl Report {
    pub fn ready(&self) -> bool {
        self.active_seconds >= MIN_CAPTURE_SECONDS && self.peaks.iter().any(Option::is_some)
    }
}

pub struct Analyzer {
    rate: f64,
    fft: Arc<dyn Fft<f64>>,
    history: Vec<Complex<f64>>,
    frame: Vec<Complex<f64>>,
    scratch: Vec<Complex<f64>>,
    window: Vec<f64>,
    bin_hz: f64,
    power_scale: f64,
    weight: f64,
    write: usize,
    until_frame: usize,
    power: [f64; MAX_BANDS],
    primed: bool,
    next_project_sample: Option<i64>,
    align_next: bool,
    ranges: [std::ops::Range<usize>; MAX_BANDS],
    report: Report,
}

impl Analyzer {
    pub fn new(rate: f64, command: Command) -> Self {
        // About 170 ms at the common 44.1/48/96/192 kHz rates.
        let size = ((rate / 6.0) as usize).next_power_of_two();
        let fft = FftPlanner::new().plan_fft_forward(size);
        let window: Vec<_> = (0..size)
            .map(|i| 0.5 - 0.5 * (TAU * i as f64 / size as f64).cos())
            .collect();
        let power_scale = 1.0 / (size as f64 * window.iter().map(|w| w * w).sum::<f64>());
        let scratch = vec![Complex::default(); fft.get_inplace_scratch_len()];
        let mut analyzer = Self {
            rate,
            fft,
            history: vec![Complex::default(); size],
            frame: vec![Complex::default(); size],
            scratch,
            window,
            bin_hz: rate / size as f64,
            power_scale,
            weight: 1.0 - (-((size / 4) as f64 / rate) / SMOOTH_SECONDS).exp(),
            write: 0,
            until_frame: size,
            power: [0.0; MAX_BANDS],
            primed: false,
            next_project_sample: None,
            align_next: true,
            ranges: std::array::from_fn(|_| 0..0),
            report: Report {
                command,
                ..Report::default()
            },
        };
        analyzer.update_ranges();
        analyzer
    }

    pub fn report(&self) -> Report {
        self.report
    }

    pub fn configure(&mut self, command: Command) {
        if self.report.command == command {
            return;
        }
        let layout_changed = self.report.command.layout != command.layout;
        self.report.command = command;
        self.report.peaks = [None; MAX_BANDS];
        self.report.active_seconds = 0.0;
        if layout_changed {
            self.update_ranges();
        }
        if layout_changed || command.capture.is_some() {
            self.reset_live();
        }
    }

    pub fn resume_capture(&mut self, previous: Report) {
        if self.report.command.capture.is_some() && self.report.command == previous.command {
            self.report.peaks = previous.peaks;
            self.report.active_seconds = previous.active_seconds;
        }
    }

    pub fn reset_live(&mut self) {
        // A complete window is written before the old history can be read.
        self.write = 0;
        self.until_frame = self.history.len();
        self.primed = false;
        self.report.audible = false;
        self.report.live = [None; MAX_BANDS];
        self.report.window = [None; MAX_BANDS];
        self.align_next = true;
    }

    /// Align complete windows to a fixed grid in DAW sample time. A seek
    /// starts a fresh window, with no samples carried across the boundary.
    pub fn begin_block(&mut self, start: Option<i64>, samples: i32) -> bool {
        let discontinuity = start != self.next_project_sample;
        if discontinuity {
            self.reset_live();
        }
        if self.align_next {
            if let Some(start) = start {
                let hop = (self.history.len() / 4) as i64;
                self.until_frame += (-start.rem_euclid(hop)).rem_euclid(hop) as usize;
            }
            self.align_next = false;
        }
        self.next_project_sample = start.and_then(|start| start.checked_add(i64::from(samples)));
        discontinuity
    }

    fn update_ranges(&mut self) {
        let layout = self.report.command.layout;
        let end = self.history.len() / 2 + 1;
        self.ranges = std::array::from_fn(|band| {
            if band >= layout.len() {
                return 0..0;
            }
            let (lo, hi) = layout.bounds(band);
            let bin = |hz| ((f64::from(hz) / self.bin_hz).ceil() as usize).clamp(1, end);
            bin(lo)..bin(hi)
        });
    }

    /// Returns a meter snapshot once per analysis hop.
    pub fn push(&mut self, left: f32, right: f32) -> Option<Report> {
        let finite = |sample: f32| {
            if sample.is_finite() {
                f64::from(sample)
            } else {
                0.0
            }
        };
        let size = self.history.len();
        self.history[self.write] = Complex::new(finite(left), finite(right));
        self.write = (self.write + 1) & (size - 1);
        self.until_frame -= 1;
        if self.until_frame != 0 {
            return None;
        }
        self.until_frame = size / 4;
        self.analyze();
        Some(self.report)
    }

    fn analyze(&mut self) {
        let size = self.history.len();
        for (i, value) in self.frame.iter_mut().enumerate() {
            *value = self.history[(self.write + i) & (size - 1)] * self.window[i];
        }
        self.fft
            .process_with_scratch(&mut self.frame, &mut self.scratch);
        let mut total_power = 0.0;
        for (band, range) in self.ranges.iter().enumerate() {
            if range.is_empty() {
                continue;
            }
            // The real and imaginary time-domain channels share one FFT. Summing
            // the two conjugate bins cancels the cross term and averages stereo
            // power, including when left and right are opposite in polarity.
            let power = range
                .clone()
                .map(|k| {
                    if k == size / 2 {
                        self.frame[k].norm_sqr() * 0.5
                    } else {
                        (self.frame[k].norm_sqr() + self.frame[size - k].norm_sqr()) * 0.5
                    }
                })
                .sum::<f64>()
                * self.power_scale;
            total_power += power;
            self.power[band] = if self.primed {
                self.power[band] + (power - self.power[band]) * self.weight
            } else {
                power
            };
            let db = (10.0 * self.power[band].max(0.0).log10()) as f32;
            self.report.live[band] = Some(db.max(FLOOR_DB));
            self.report.window[band] = Some(((10.0 * power.log10()) as f32).max(FLOOR_DB));
        }
        self.primed = true;
        self.report.audible = total_power >= ACTIVE_POWER;
        if self.report.command.capture.is_some() && self.report.audible {
            self.report.active_seconds += (size / 4) as f64 / self.rate;
            for (peak, live) in self.report.peaks.iter_mut().zip(self.report.live) {
                if let Some(db) = live.filter(|level| *level >= MIN_TARGET_DB) {
                    *peak = Some(peak.map_or(db, |previous| previous.max(db)));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_stereo_power_without_phase_cancellation() {
        let mut analyzer = Analyzer::new(48_000.0, Command::default());
        for i in 0..16_384 {
            let sample = (TAU * 750.0 * i as f64 / 48_000.0).sin() as f32 * 0.5;
            analyzer.push(sample, -sample);
        }
        let report = analyzer.report();
        assert!(report.audible);
        assert!((report.live[3].unwrap() - (-9.031)).abs() < 0.02);
        assert!(report.live[2].unwrap() < -75.0);
        assert!(report.live[4].unwrap() < -75.0);
    }

    #[test]
    fn capture_excludes_previous_sound_and_tracks_gain_in_db() {
        let rate = 48_000.0;
        let mut analyzer = Analyzer::new(rate, Command::default());
        let tone = |i: usize| (TAU * 750.0 * i as f64 / rate).sin() as f32;
        for i in 0..16_384 {
            analyzer.push(tone(i), tone(i));
        }
        analyzer.configure(Command {
            capture: Some(1),
            ..Command::default()
        });
        for i in 0..60_000 {
            analyzer.push(tone(i) * 0.25, tone(i) * 0.25);
        }
        let capture = analyzer.report();
        assert!(capture.ready());
        let target = capture.peaks[3].unwrap();
        assert!((target - (-15.052)).abs() < 0.02);
        analyzer.configure(Command::default());
        for i in 0..60_000 {
            analyzer.push(tone(i) * 0.5, tone(i) * 0.5);
        }
        assert!((analyzer.report().live[3].unwrap() - target - 6.02).abs() < 0.05);
    }

    #[test]
    fn silence_does_not_form_a_reference_and_high_bands_can_be_unavailable() {
        let mut analyzer = Analyzer::new(
            8_000.0,
            Command {
                capture: Some(1),
                ..Command::default()
            },
        );
        for _ in 0..16_384 {
            analyzer.push(0.0, 0.0);
        }
        let report = analyzer.report();
        assert_eq!(report.live[5], None);
        assert_eq!(report.active_seconds, 0.0);
        assert!(!report.ready());
    }

    #[test]
    fn layout_edits_preserve_order_and_limits() {
        let mut layout = Layout::default();
        for step in [1, -8, 72, -120, 40] {
            for idx in 0..MAX_BANDS {
                layout.adjust(idx, step);
            }
            for _ in 0..MAX_BANDS {
                layout.resize(true);
                assert_eq!(Layout::from_cuts(layout.encoded_cuts()), Some(layout));
            }
            assert_eq!(layout.len(), MAX_BANDS);
            for _ in 0..MAX_BANDS {
                layout.resize(false);
                assert_eq!(Layout::from_cuts(layout.encoded_cuts()), Some(layout));
            }
            assert_eq!(layout.len(), MIN_BANDS);
        }
    }
}
