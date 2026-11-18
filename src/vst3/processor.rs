use std::{
    cell::UnsafeCell,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use toybox::vst3::prelude::{Steinberg::*, *};

use crate::{
    analyzer::{Analyzer, Command},
    session::{AudioLink, Shared},
};

use super::{controller::Controller, load_state, store_state};

pub struct Processor {
    shared: Arc<Shared>,
    connection: InstanceConnection<Shared>,
    stereo: AtomicBool,
    busy: AtomicBool,
    runtime: UnsafeCell<Runtime>,
}

struct Runtime {
    link: AudioLink,
    analyzer: Analyzer,
}

// SAFETY: VST3 serializes process callbacks for an instance. The try-acquire
// guard also prevents mutable runtime aliasing with setup and lifecycle calls.
unsafe impl Sync for Processor {}

impl Processor {
    pub const CID: TUID = uid(0x7C78528A, 0xE13547D2, 0xA9658C68, 0xA634EDF7);

    pub fn new() -> Self {
        let (shared, link) = Shared::new();
        let shared = Arc::new(shared);
        Self {
            connection: InstanceConnection::new(
                InstanceConnectionRole::Processor,
                Arc::clone(&shared),
            ),
            shared,
            stereo: AtomicBool::new(true),
            busy: AtomicBool::new(false),
            runtime: UnsafeCell::new(Runtime {
                link,
                analyzer: Analyzer::new(48_000.0, Command::default()),
            }),
        }
    }

    fn with_runtime(&self, f: impl FnOnce(&mut Runtime)) -> tresult {
        if self
            .busy
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return kResultFalse;
        }
        struct Release<'a>(&'a AtomicBool);
        impl Drop for Release<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Release);
            }
        }
        let _release = Release(&self.busy);
        // SAFETY: the guard permits a single mutable borrower.
        f(unsafe { &mut *self.runtime.get() });
        kResultOk
    }

    fn channel_count(&self) -> i32 {
        if self.stereo.load(Ordering::Relaxed) {
            2
        } else {
            1
        }
    }
}

impl Class for Processor {
    type Interfaces = (
        IComponent,
        IAudioProcessor,
        IConnectionPoint,
        IToyboxSharedState,
    );
}

impl_vst3_instance_connection!(Processor, connection);

impl IPluginBaseTrait for Processor {
    unsafe fn initialize(&self, _: *mut FUnknown) -> tresult {
        kResultOk
    }
    unsafe fn terminate(&self) -> tresult {
        kResultOk
    }
}

impl IComponentTrait for Processor {
    unsafe fn getControllerClassId(&self, id: *mut TUID) -> tresult {
        let Some(id) = (unsafe { id.as_mut() }) else {
            return kInvalidArgument;
        };
        *id = Controller::CID;
        kResultOk
    }

    unsafe fn setIoMode(&self, _: IoMode) -> tresult {
        kResultOk
    }

    unsafe fn getBusCount(&self, media: MediaType, direction: BusDirection) -> i32 {
        i32::from(is_audio_bus(media, direction, 0))
    }

    unsafe fn getBusInfo(
        &self,
        media: MediaType,
        direction: BusDirection,
        index: i32,
        info: *mut BusInfo,
    ) -> tresult {
        if !is_audio_bus(media, direction, index) {
            return kInvalidArgument;
        }
        let Some(info) = (unsafe { info.as_mut() }) else {
            return kInvalidArgument;
        };
        *info = BusInfo {
            mediaType: media,
            direction,
            channelCount: self.channel_count(),
            name: [0; 128],
            busType: BusTypes_::kMain as BusType,
            flags: BusInfo_::BusFlags_::kDefaultActive,
        };
        let name = if direction == BusDirections_::kInput as BusDirection {
            "Input"
        } else {
            "Output"
        };
        copy_wstring(name, &mut info.name);
        kResultOk
    }

    unsafe fn getRoutingInfo(&self, _: *mut RoutingInfo, _: *mut RoutingInfo) -> tresult {
        kNotImplemented
    }

    unsafe fn activateBus(
        &self,
        media: MediaType,
        direction: BusDirection,
        index: i32,
        _: TBool,
    ) -> tresult {
        if is_audio_bus(media, direction, index) {
            kResultOk
        } else {
            kInvalidArgument
        }
    }

    unsafe fn setActive(&self, _: TBool) -> tresult {
        kResultOk
    }

    unsafe fn setState(&self, stream: *mut IBStream) -> tresult {
        let Some(preset) = (unsafe { load_state(stream) }) else {
            return kInvalidArgument;
        };
        self.shared.restore(preset);
        kResultOk
    }

    unsafe fn getState(&self, stream: *mut IBStream) -> tresult {
        unsafe { store_state(stream, self.shared.preset()) }
    }
}

impl IAudioProcessorTrait for Processor {
    unsafe fn setBusArrangements(
        &self,
        inputs: *mut SpeakerArrangement,
        num_inputs: i32,
        outputs: *mut SpeakerArrangement,
        num_outputs: i32,
    ) -> tresult {
        if num_inputs != 1 || num_outputs != 1 {
            return kResultFalse;
        }
        let (Some(&input), Some(&output)) = (unsafe { (inputs.as_ref(), outputs.as_ref()) }) else {
            return kInvalidArgument;
        };
        if input != output || !matches!(input, SpeakerArr::kMono | SpeakerArr::kStereo) {
            return kResultFalse;
        }
        self.stereo
            .store(input == SpeakerArr::kStereo, Ordering::Relaxed);
        kResultOk
    }

    unsafe fn getBusArrangement(
        &self,
        direction: BusDirection,
        index: i32,
        arrangement: *mut SpeakerArrangement,
    ) -> tresult {
        let Some(arrangement) = (unsafe { arrangement.as_mut() }) else {
            return kInvalidArgument;
        };
        if !is_audio_bus(MediaTypes_::kAudio as MediaType, direction, index) {
            return kInvalidArgument;
        }
        *arrangement = if self.channel_count() == 2 {
            SpeakerArr::kStereo
        } else {
            SpeakerArr::kMono
        };
        kResultOk
    }

    unsafe fn canProcessSampleSize(&self, sample_size: i32) -> tresult {
        if sample_size == SymbolicSampleSizes_::kSample32 as i32 {
            kResultOk
        } else {
            kResultFalse
        }
    }

    unsafe fn getLatencySamples(&self) -> u32 {
        0
    }
    unsafe fn getTailSamples(&self) -> u32 {
        0
    }

    unsafe fn setupProcessing(&self, setup: *mut ProcessSetup) -> tresult {
        let Some(setup) = (unsafe { setup.as_ref() }) else {
            return kInvalidArgument;
        };
        if !(8_000.0..=768_000.0).contains(&setup.sampleRate) {
            return kInvalidArgument;
        }
        self.with_runtime(|runtime| {
            // A host sample-rate change invalidates only live analysis. Keep an
            // in-flight capture's peaks and duration if its command is unchanged.
            let command = *runtime.link.commands.read();
            let previous = runtime.analyzer.report();
            let mut analyzer = Analyzer::new(setup.sampleRate, command);
            analyzer.resume_capture(previous);
            runtime.analyzer = analyzer;
            runtime.link.reports.write(runtime.analyzer.report());
        })
    }

    unsafe fn setProcessing(&self, processing: TBool) -> tresult {
        if processing == 0 {
            return kResultOk;
        }
        self.with_runtime(|runtime| {
            runtime.analyzer.reset_live();
            runtime.link.reports.write(runtime.analyzer.report());
        })
    }

    unsafe fn process(&self, data: *mut ProcessData) -> tresult {
        let Some(data) = (unsafe { data.as_ref() }) else {
            return kInvalidArgument;
        };
        if data.numSamples == 0 {
            return kResultOk;
        }
        if data.numSamples < 0 || data.symbolicSampleSize != SymbolicSampleSizes_::kSample32 as i32
        {
            return kInvalidArgument;
        }
        // A host may deactivate both buses. There is nothing to copy or measure.
        if data.numInputs == 0 && data.numOutputs == 0 {
            return kResultOk;
        }
        if data.numInputs != 1
            || data.numOutputs != 1
            || data.inputs.is_null()
            || data.outputs.is_null()
        {
            return kInvalidArgument;
        }
        let count = self.channel_count();
        let (input, output) = unsafe { (&*data.inputs, &mut *data.outputs) };
        if input.numChannels == 0 && output.numChannels == 0 {
            return kResultOk;
        }
        if input.numChannels != count || output.numChannels != count {
            return kInvalidArgument;
        }
        let (inputs, outputs) = unsafe {
            (
                input.__field0.channelBuffers32,
                output.__field0.channelBuffers32,
            )
        };
        if inputs.is_null() || outputs.is_null() {
            return kInvalidArgument;
        }
        // Keep sample buffers as raw pointers because VST3 allows in-place processing.
        let mut channels = [(std::ptr::null_mut(), std::ptr::null_mut()); 2];
        for (index, channel) in channels.iter_mut().take(count as usize).enumerate() {
            *channel = unsafe { (*inputs.add(index), *outputs.add(index)) };
            if channel.0.is_null() || channel.1.is_null() {
                return kInvalidArgument;
            }
        }
        let [(left_in, left_out), (right_in, right_out)] = channels;
        let stereo = count == 2;
        output.silenceFlags = input.silenceFlags;
        self.with_runtime(|runtime| {
            runtime.analyzer.configure(*runtime.link.commands.read());
            for index in 0..data.numSamples as usize {
                // The host provides buffers of numSamples. Read both inputs
                // before writing either output, including for in-place buses.
                let left = unsafe { left_in.add(index).read() };
                let right = if stereo {
                    unsafe { right_in.add(index).read() }
                } else {
                    left
                };
                unsafe {
                    left_out.add(index).write(left);
                    if stereo {
                        right_out.add(index).write(right);
                    }
                }
                if let Some(report) = runtime.analyzer.push(left, right) {
                    runtime.link.reports.write(report);
                }
            }
        })
    }
}

fn is_audio_bus(media: MediaType, direction: BusDirection, index: i32) -> bool {
    index == 0
        && media == MediaTypes_::kAudio as MediaType
        && matches!(
            direction as BusDirections,
            BusDirections_::kInput | BusDirections_::kOutput
        )
}
