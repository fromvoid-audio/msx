use std::{cell::RefCell, ffi::CStr, ptr, rc::Rc, sync::Arc};

use toybox::vst3::prelude::{Steinberg::*, *};

use crate::session::Shared;

use super::{editor, load_state};

pub type Handler = Rc<RefCell<Option<ComPtr<IComponentHandler>>>>;

pub struct Controller {
    pub(super) connection: Arc<InstanceConnection<Shared>>,
    handler: Handler,
}

impl Controller {
    pub const CID: TUID = uid(0x05EF5EED, 0xDB6346BC, 0x80428B55, 0xF6FFD38E);

    pub fn new() -> Self {
        let (shared, _) = Shared::new();
        Self {
            connection: Arc::new(InstanceConnection::new(
                InstanceConnectionRole::Controller,
                Arc::new(shared),
            )),
            handler: Rc::new(RefCell::new(None)),
        }
    }
}

impl Class for Controller {
    type Interfaces = (IEditController, IConnectionPoint, IToyboxSharedState);
}

impl_vst3_instance_connection!(Controller, connection);

impl IPluginBaseTrait for Controller {
    unsafe fn initialize(&self, _: *mut FUnknown) -> tresult {
        kResultOk
    }

    unsafe fn terminate(&self) -> tresult {
        self.handler.borrow_mut().take();
        kResultOk
    }
}

impl IEditControllerTrait for Controller {
    unsafe fn setComponentState(&self, stream: *mut IBStream) -> tresult {
        // The connected processor already owns this state. Validate the stream
        // without replaying it over edits that may have since been made.
        if unsafe { load_state(stream) }.is_some() {
            kResultOk
        } else {
            kInvalidArgument
        }
    }

    unsafe fn setState(&self, _: *mut IBStream) -> tresult {
        kNotImplemented
    }
    unsafe fn getState(&self, _: *mut IBStream) -> tresult {
        kNotImplemented
    }
    unsafe fn getParameterCount(&self) -> i32 {
        0
    }
    unsafe fn getParameterInfo(&self, _: i32, _: *mut ParameterInfo) -> tresult {
        kInvalidArgument
    }
    unsafe fn getParamStringByValue(
        &self,
        _: ParamID,
        _: ParamValue,
        _: *mut String128,
    ) -> tresult {
        kInvalidArgument
    }
    unsafe fn getParamValueByString(
        &self,
        _: ParamID,
        _: *mut TChar,
        _: *mut ParamValue,
    ) -> tresult {
        kInvalidArgument
    }
    unsafe fn normalizedParamToPlain(&self, _: ParamID, value: ParamValue) -> ParamValue {
        value
    }
    unsafe fn plainParamToNormalized(&self, _: ParamID, value: ParamValue) -> ParamValue {
        value
    }
    unsafe fn getParamNormalized(&self, _: ParamID) -> ParamValue {
        0.0
    }
    unsafe fn setParamNormalized(&self, _: ParamID, _: ParamValue) -> tresult {
        kInvalidArgument
    }

    unsafe fn setComponentHandler(&self, handler: *mut IComponentHandler) -> tresult {
        let next = unsafe { ComRef::from_raw(handler) }.map(|handler| handler.to_com_ptr());
        let previous = self.handler.replace(next);
        drop(previous);
        kResultOk
    }

    unsafe fn createView(&self, name: FIDString) -> *mut IPlugView {
        if name.is_null()
            || unsafe { CStr::from_ptr(name) } != unsafe { CStr::from_ptr(ViewType::kEditor) }
        {
            return ptr::null_mut();
        }
        let gui = editor::new(Arc::clone(&self.connection), Rc::clone(&self.handler));
        let (width, height) = editor::SIZE;
        ComWrapper::new(
            HostedVst3View::new(gui, width, height).with_size_bounds(width, height, width, height),
        )
        .to_com_ptr::<IPlugView>()
        .map_or(ptr::null_mut(), ComPtr::into_raw)
    }
}
