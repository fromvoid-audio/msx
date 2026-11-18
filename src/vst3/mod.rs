//! Toybox VST3 adapter.

use toybox::vst3::prelude::{Steinberg::*, *};

use crate::session::Preset;

mod controller;
mod editor;
mod factory;
mod processor;

const NAME: &str = "MXH";
const STATE_MAGIC: u32 = u32::from_le_bytes(*b"MXH1");
const STATE_VERSION: u32 = 3;

unsafe fn load_state(stream: *mut IBStream) -> Option<Preset> {
    const PEAK_STATE_VERSION: u32 = 1;
    const TIMELINE_STATE_VERSION: u32 = 2;
    let data = unsafe {
        read_versioned_payload(
            stream,
            STATE_MAGIC,
            &[PEAK_STATE_VERSION, TIMELINE_STATE_VERSION, STATE_VERSION],
        )
    }
    .ok()?;
    match data.version {
        PEAK_STATE_VERSION => Preset::decode_peak_state(&data.payload),
        STATE_VERSION => Preset::decode(&data.payload),
        TIMELINE_STATE_VERSION => Preset::decode_timeline_state(&data.payload),
        _ => None,
    }
}

unsafe fn store_state(stream: *mut IBStream, preset: Preset) -> tresult {
    match unsafe { write_versioned_payload(stream, STATE_MAGIC, STATE_VERSION, &preset.encode()) } {
        Ok(()) => kResultOk,
        Err(_) => kResultFalse,
    }
}

toybox::vst3_plugin_entry!(factory::Factory);

#[cfg(test)]
mod tests;
