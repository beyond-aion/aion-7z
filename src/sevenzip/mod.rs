pub mod com;
pub mod in_archive;
pub mod stream;

use com::*;
use core::ffi::c_void;
use std::ptr::null_mut;

#[unsafe(no_mangle)]
pub unsafe extern "system" fn CreateObject(
    class_id: *const GUID,
    iid: *const GUID,
    out_object: *mut *mut c_void,
) -> HRESULT {
    if iid.is_null() || out_object.is_null() {
        return E_INVALIDARG;
    }

    if class_id.is_null() || *class_id == CLSID_AION_PAK {
        let instance = in_archive::AionInArchive::new();
        let query_interface = (*(*instance).in_archive_vtable).query_interface;
        let hr = query_interface(instance as *mut in_archive::IInArchive, iid, out_object);
        let release = (*(*instance).in_archive_vtable).release;
        release(instance as *mut in_archive::IInArchive);
        return hr;
    }

    *out_object = null_mut();
    CLASS_E_CLASSNOTAVAILABLE
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn GetHandlerProperty(prop_id: u32, value: *mut PROPVARIANT) -> HRESULT {
    if value.is_null() {
        return E_INVALIDARG;
    }

    let val = &mut *value;
    val.set_empty();

    match prop_id {
        NARCHIVE_NAME => {
            val.set_str("Aion PAK");
        }
        NARCHIVE_CLASS_ID => {
            let clsid_bytes = core::slice::from_raw_parts(
                &CLSID_AION_PAK as *const GUID as *const u8,
                core::mem::size_of::<GUID>(),
            );
            val.set_bytes(clsid_bytes);
        }
        NARCHIVE_EXTENSION => {
            val.set_str("pak");
        }
        NARCHIVE_ADD_EXTENSION => {
            val.set_str("pak");
        }
        NARCHIVE_UPDATE => {
            val.set_bool(false);
        }
        NARCHIVE_KEEP_NAME => {
            val.set_bool(false);
        }
        NARCHIVE_START_SIGNATURE => {
            val.set_bytes(&[0xAF, 0xB4]);
        }
        _ => {}
    }

    S_OK
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn GetNumberOfFormats(num_formats: *mut u32) -> HRESULT {
    if num_formats.is_null() {
        return E_INVALIDARG;
    }
    *num_formats = 1;
    S_OK
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn GetHandlerProperty2(
    format_index: u32,
    prop_id: u32,
    value: *mut PROPVARIANT,
) -> HRESULT {
    if format_index != 0 {
        return E_INVALIDARG;
    }
    GetHandlerProperty(prop_id, value)
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn CreateObject2(
    format_index: u32,
    iid: *const GUID,
    out_object: *mut *mut c_void,
) -> HRESULT {
    if format_index != 0 || iid.is_null() || out_object.is_null() {
        return E_INVALIDARG;
    }
    CreateObject(&CLSID_AION_PAK, iid, out_object)
}
