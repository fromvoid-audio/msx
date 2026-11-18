use super::*;
use crate::session::Action;
use std::{
    cell::RefCell,
    f64::consts::TAU,
    ffi::c_void,
    io::{Cursor, Read, Write},
    ptr,
};

struct Stream(RefCell<Cursor<Vec<u8>>>);

impl Class for Stream {
    type Interfaces = (IBStream,);
}

impl IBStreamTrait for Stream {
    unsafe fn read(&self, buffer: *mut c_void, count: i32, actual: *mut i32) -> tresult {
        let bytes = unsafe { std::slice::from_raw_parts_mut(buffer.cast(), count as usize) };
        let size = self.0.borrow_mut().read(bytes).unwrap() as i32;
        unsafe { *actual = size };
        if size == count {
            kResultOk
        } else {
            kResultFalse
        }
    }

    unsafe fn write(&self, buffer: *mut c_void, count: i32, actual: *mut i32) -> tresult {
        let bytes = unsafe { std::slice::from_raw_parts(buffer.cast(), count as usize) };
        self.0.borrow_mut().write_all(bytes).unwrap();
        unsafe { *actual = count };
        kResultOk
    }

    unsafe fn seek(&self, _: i64, _: i32, _: *mut i64) -> tresult {
        kNotImplemented
    }
    unsafe fn tell(&self, _: *mut i64) -> tresult {
        kNotImplemented
    }
}

#[test]
fn host_capture_passes_stereo_through_and_restores_a_distinct_instance() {
    let processor = ComWrapper::new(processor::Processor::new());
    let controller = ComWrapper::new(controller::Controller::new());
    let component = processor.to_com_ptr::<IComponent>().unwrap();
    let audio = processor.to_com_ptr::<IAudioProcessor>().unwrap();
    let proc_endpoint = processor.to_com_ptr::<IConnectionPoint>().unwrap();
    let edit_endpoint = controller.to_com_ptr::<IConnectionPoint>().unwrap();
    assert_eq!(
        unsafe { edit_endpoint.connect(proc_endpoint.as_ptr()) },
        kResultOk
    );
    let shared = controller.connection.shared();
    assert!(shared.apply(Action::Crossover(0, 2)));
    shared.apply(Action::Begin);

    const BLOCK: usize = 512;
    let mut setup = ProcessSetup {
        processMode: ProcessModes_::kRealtime as i32,
        symbolicSampleSize: SymbolicSampleSizes_::kSample32 as i32,
        maxSamplesPerBlock: BLOCK as i32,
        sampleRate: 48_000.0,
    };
    assert_eq!(unsafe { audio.setupProcessing(&mut setup) }, kResultOk);
    assert_eq!(unsafe { audio.setProcessing(1) }, kResultOk);
    assert_eq!(unsafe { audio.getLatencySamples() }, 0);

    for block in 0..120 {
        let mut left: [f32; BLOCK] = std::array::from_fn(|i| {
            let phase = TAU * 750.0 * (block * BLOCK + i) as f64 / 48_000.0;
            (phase.sin() * 0.25) as f32
        });
        let mut right = left.map(|sample| -sample);
        let expected = [left, right];
        let mut right_out = [f32::NAN; BLOCK];
        let mut inputs = [left.as_mut_ptr(), right.as_mut_ptr()];
        let mut outputs = [left.as_mut_ptr(), right_out.as_mut_ptr()];
        // SAFETY: zero initializes only scalar, raw pointer, and union fields.
        let (mut input, mut output, mut data): (AudioBusBuffers, AudioBusBuffers, ProcessData) =
            unsafe { std::mem::zeroed() };
        input.numChannels = 2;
        output.numChannels = 2;
        input.__field0.channelBuffers32 = inputs.as_mut_ptr();
        output.__field0.channelBuffers32 = outputs.as_mut_ptr();
        data.numSamples = BLOCK as i32;
        data.numInputs = 1;
        data.numOutputs = 1;
        data.inputs = &mut input;
        data.outputs = &mut output;
        data.symbolicSampleSize = setup.symbolicSampleSize;
        assert_eq!(unsafe { audio.process(&mut data) }, kResultOk);
        assert_eq!(left, expected[0]);
        assert_eq!(right_out, expected[1]);
    }
    assert_eq!(unsafe { audio.setProcessing(0) }, kResultOk);
    assert!(shared.apply(Action::Save));
    assert!(shared.apply(Action::Offset(-2)));
    let saved = shared.preset();
    assert!((saved.target(3).unwrap() - (-16.052)).abs() < 0.02);

    let stream = ComWrapper::new(Stream(RefCell::new(Cursor::new(Vec::new()))));
    let stream_ptr = stream.to_com_ptr::<IBStream>().unwrap();
    assert_eq!(
        unsafe { component.getState(stream_ptr.as_ptr()) },
        kResultOk
    );
    stream.0.borrow_mut().set_position(0);
    let restored = ComWrapper::new(processor::Processor::new());
    let restored_component = restored.to_com_ptr::<IComponent>().unwrap();
    let restored_editor = ComWrapper::new(controller::Controller::new());
    let new_proc = restored.to_com_ptr::<IConnectionPoint>().unwrap();
    let new_edit = restored_editor.to_com_ptr::<IConnectionPoint>().unwrap();
    assert_eq!(unsafe { new_proc.connect(new_edit.as_ptr()) }, kResultOk);
    assert_eq!(
        restored_editor.connection.shared().preset(),
        Preset::default()
    );
    assert_eq!(
        unsafe { restored_component.setState(stream_ptr.as_ptr()) },
        kResultOk
    );
    assert_eq!(restored_editor.connection.shared().preset(), saved);
    assert_eq!(
        unsafe { restored_component.setState(ptr::null_mut()) },
        kInvalidArgument
    );
    assert_eq!(restored_editor.connection.shared().preset(), saved);
}
