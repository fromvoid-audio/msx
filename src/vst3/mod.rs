//! Toybox VST3 adapter.

use toybox::vst3::prelude::{Steinberg::*, *};

use crate::session::SavedState;

mod controller;
mod editor;
mod factory;
mod processor;

const NAME: &str = "MXH";
const STATE_MAGIC: u32 = u32::from_le_bytes(*b"MXH1");
const STATE_VERSION: u32 = 2;

unsafe fn load_state(stream: *mut IBStream) -> Option<SavedState> {
    let state = unsafe { read_versioned_payload(stream, STATE_MAGIC, &[1, STATE_VERSION]) }.ok()?;
    SavedState::decode(state.version, &state.payload)
}

unsafe fn store_state(stream: *mut IBStream, state: &SavedState) -> tresult {
    match unsafe { write_versioned_payload(stream, STATE_MAGIC, STATE_VERSION, &state.encode()) } {
        Ok(()) => kResultOk,
        Err(_) => kResultFalse,
    }
}

toybox::vst3_plugin_entry!(factory::Factory);

#[cfg(test)]
mod tests;
