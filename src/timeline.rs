//! Captured per-window levels keyed by the DAW sample clock.

use crate::analyzer::{FLOOR_DB, FrameTime, Levels, MAX_BANDS};

pub const MAX_TIMELINE_FRAMES: usize = 32_768;
pub const FRAME_BYTES: usize = 16 + 4 * MAX_BANDS;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimelineFrame {
    pub time: FrameTime,
    // A compact representation matters for a full song. Negative infinity
    // represents an unavailable band, distinct from the measured floor.
    levels: [f32; MAX_BANDS],
}

impl TimelineFrame {
    pub fn new(time: FrameTime, levels: Levels) -> Self {
        Self {
            time,
            levels: levels.map(|db| db.unwrap_or(f32::NEG_INFINITY)),
        }
    }

    pub fn levels(self) -> Levels {
        self.levels.map(|db| db.is_finite().then_some(db))
    }

    pub fn encode(self, bytes: &mut Vec<u8>) {
        bytes.extend(self.time.sample_rate_bits.to_le_bytes());
        bytes.extend(self.time.end_sample.to_le_bytes());
        for db in self.levels {
            bytes.extend(db.to_le_bytes());
        }
    }

    pub fn decode(bytes: &[u8], band_count: usize) -> Option<Self> {
        let (&rate, rest) = bytes.split_first_chunk::<8>()?;
        let (&sample, bands) = rest.split_first_chunk::<8>()?;
        let sample_rate_bits = u64::from_le_bytes(rate);
        if !(100.0..=2_000_000.0).contains(&f64::from_bits(sample_rate_bits)) {
            return None;
        }
        let (bands, remainder) = bands.as_chunks::<4>();
        if bands.len() != MAX_BANDS || !remainder.is_empty() {
            return None;
        }
        let levels = std::array::from_fn(|band| f32::from_le_bytes(bands[band]));
        for (band, &db) in levels.iter().enumerate() {
            if db != f32::NEG_INFINITY && (band >= band_count || !(FLOOR_DB..=800.0).contains(&db))
            {
                return None;
            }
        }
        Some(Self {
            time: FrameTime {
                sample_rate_bits,
                end_sample: i64::from_le_bytes(sample),
            },
            levels,
        })
    }
}
