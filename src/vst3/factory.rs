//! MXH component and controller factory.

use std::ffi::c_void;

use toybox::vst3::prelude::Steinberg::*;
use toybox::vst3::prelude::*;

use super::NAME;
use super::controller::Controller;
use super::processor::Processor;

/// Plugin factory exposed to the host.
#[derive(Default)]
pub(crate) struct Factory;

impl Class for Factory {
    type Interfaces = (IPluginFactory,);
}

impl IPluginFactoryTrait for Factory {
    unsafe fn getFactoryInfo(&self, info: *mut PFactoryInfo) -> tresult {
        if info.is_null() {
            return kInvalidArgument;
        }

        // SAFETY: pointer validated non-null above.
        let info = unsafe { &mut *info };
        copy_cstring("PORTALSURFER", &mut info.vendor);
        copy_cstring("https://portalsurfer.org", &mut info.url);
        copy_cstring("", &mut info.email);
        info.flags = PFactoryInfo_::FactoryFlags_::kUnicode as int32;

        kResultOk
    }

    unsafe fn countClasses(&self) -> i32 {
        2
    }

    unsafe fn getClassInfo(&self, index: i32, info: *mut PClassInfo) -> tresult {
        if info.is_null() {
            return kInvalidArgument;
        }

        // SAFETY: pointer validated non-null above.
        let info = unsafe { &mut *info };
        match index {
            0 => {
                write_class_info_many(info, Processor::CID, CATEGORY_AUDIO_MODULE_CLASS, NAME);
                kResultOk
            }
            1 => {
                write_class_info_many(
                    info,
                    Controller::CID,
                    CATEGORY_COMPONENT_CONTROLLER_CLASS,
                    NAME,
                );
                kResultOk
            }
            _ => kInvalidArgument,
        }
    }

    unsafe fn createInstance(
        &self,
        cid: FIDString,
        iid: FIDString,
        obj: *mut *mut c_void,
    ) -> tresult {
        if cid.is_null() || iid.is_null() || obj.is_null() {
            return kInvalidArgument;
        }

        // SAFETY: caller provides valid pointers per VST3 ABI.
        let class_id = unsafe { *(cid as *const TUID) };
        let instance = match class_id {
            Processor::CID => ComWrapper::new(Processor::new()).to_com_ptr::<FUnknown>(),
            Controller::CID => ComWrapper::new(Controller::new()).to_com_ptr::<FUnknown>(),
            _ => None,
        };

        let Some(instance) = instance else {
            return kInvalidArgument;
        };

        let ptr = instance.as_ptr();
        // SAFETY: object pointer and requested interface are provided by the host.
        unsafe { ((*(*ptr).vtbl).queryInterface)(ptr, iid as *mut TUID, obj) }
    }
}
