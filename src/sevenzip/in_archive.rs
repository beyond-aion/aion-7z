use super::com::*;
use super::stream::*;
use crate::archive::{ContentKind, PakArchive};
use core::ffi::c_void;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicU32, Ordering};

#[repr(C)]
pub struct IInArchiveVTable {
    // IUnknown
    pub query_interface: unsafe extern "system" fn(this: *mut IInArchive, iid: *const GUID, out: *mut *mut c_void) -> HRESULT,
    pub add_ref: unsafe extern "system" fn(this: *mut IInArchive) -> u32,
    pub release: unsafe extern "system" fn(this: *mut IInArchive) -> u32,
    // IInArchive
    pub open: unsafe extern "system" fn(
        this: *mut IInArchive,
        stream: *mut IInStream,
        max_check_start_position: *const u64,
        open_callback: *mut IArchiveOpenCallback,
    ) -> HRESULT,
    pub close: unsafe extern "system" fn(this: *mut IInArchive) -> HRESULT,
    pub get_number_of_items: unsafe extern "system" fn(this: *mut IInArchive, num_items: *mut u32) -> HRESULT,
    pub get_property: unsafe extern "system" fn(
        this: *mut IInArchive,
        index: u32,
        prop_id: u32,
        value: *mut PROPVARIANT,
    ) -> HRESULT,
    pub extract: unsafe extern "system" fn(
        this: *mut IInArchive,
        indices: *const u32,
        num_items: u32,
        test_mode: i32,
        extract_callback: *mut IArchiveExtractCallback,
    ) -> HRESULT,
    pub get_archive_property: unsafe extern "system" fn(this: *mut IInArchive, prop_id: u32, value: *mut PROPVARIANT) -> HRESULT,
    pub get_number_of_properties: unsafe extern "system" fn(this: *mut IInArchive, num_properties: *mut u32) -> HRESULT,
    pub get_property_info: unsafe extern "system" fn(
        this: *mut IInArchive,
        index: u32,
        name: *mut *mut u16,
        prop_id: *mut u32,
        var_type: *mut VARTYPE,
    ) -> HRESULT,
    pub get_number_of_archive_properties: unsafe extern "system" fn(this: *mut IInArchive, num_properties: *mut u32) -> HRESULT,
    pub get_archive_property_info: unsafe extern "system" fn(
        this: *mut IInArchive,
        index: u32,
        name: *mut *mut u16,
        prop_id: *mut u32,
        var_type: *mut VARTYPE,
    ) -> HRESULT,
}

#[repr(C)]
pub struct IInArchive {
    pub vtable: *const IInArchiveVTable,
}

#[repr(C)]
pub struct ISetCompressCodecsInfoVTable {
    pub query_interface: unsafe extern "system" fn(this: *mut ISetCompressCodecsInfo, iid: *const GUID, out: *mut *mut c_void) -> HRESULT,
    pub add_ref: unsafe extern "system" fn(this: *mut ISetCompressCodecsInfo) -> u32,
    pub release: unsafe extern "system" fn(this: *mut ISetCompressCodecsInfo) -> u32,
    pub set_compress_codecs_info: unsafe extern "system" fn(this: *mut ISetCompressCodecsInfo, compress_codecs_info: *mut c_void) -> HRESULT,
}

#[repr(C)]
pub struct ISetCompressCodecsInfo {
    pub vtable: *const ISetCompressCodecsInfoVTable,
}

#[repr(C)]
pub struct AionInArchive {
    pub in_archive_vtable: *const IInArchiveVTable,
    pub set_compress_codecs_info_vtable: *const ISetCompressCodecsInfoVTable,
    ref_count: AtomicU32,
    archive: Option<PakArchive>,
    stream: Option<ComInStream>,
    /// Lazily computed content kinds per entry (outer None = not computed yet).
    kinds: Vec<Option<Option<ContentKind>>>,
}

static IN_ARCHIVE_VTABLE: IInArchiveVTable = IInArchiveVTable {
    query_interface: in_archive_query_interface,
    add_ref: in_archive_add_ref,
    release: in_archive_release,
    open: in_archive_open,
    close: in_archive_close,
    get_number_of_items: in_archive_get_number_of_items,
    get_property: in_archive_get_property,
    extract: in_archive_extract,
    get_archive_property: in_archive_get_archive_property,
    get_number_of_properties: in_archive_get_number_of_properties,
    get_property_info: in_archive_get_property_info,
    get_number_of_archive_properties: in_archive_get_number_of_archive_properties,
    get_archive_property_info: in_archive_get_archive_property_info,
};

static SET_COMPRESS_CODECS_INFO_VTABLE: ISetCompressCodecsInfoVTable = ISetCompressCodecsInfoVTable {
    query_interface: codecs_info_query_interface,
    add_ref: codecs_info_add_ref,
    release: codecs_info_release,
    set_compress_codecs_info: codecs_info_set_compress_codecs_info,
};

impl AionInArchive {
    pub fn new() -> *mut AionInArchive {
        let instance = Box::new(Self {
            in_archive_vtable: &IN_ARCHIVE_VTABLE,
            set_compress_codecs_info_vtable: &SET_COMPRESS_CODECS_INFO_VTABLE,
            ref_count: AtomicU32::new(1),
            archive: None,
            stream: None,
            kinds: Vec::new(),
        });
        Box::into_raw(instance)
    }
}

unsafe extern "system" fn in_archive_query_interface(
    this: *mut IInArchive,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() || iid.is_null() {
        return E_INVALIDARG;
    }

    let aion = this as *mut AionInArchive;
    let iid_val = *iid;

    if iid_val == IID_IUNKNOWN || iid_val == IID_IINARCHIVE {
        *out = this as *mut c_void;
        (*aion).ref_count.fetch_add(1, Ordering::SeqCst);
        return S_OK;
    }

    if iid_val == IID_ISETCOMPRESSCODECSINFO {
        *out = &raw mut (*aion).set_compress_codecs_info_vtable as *mut c_void;
        (*aion).ref_count.fetch_add(1, Ordering::SeqCst);
        return S_OK;
    }

    *out = null_mut();
    E_NOINTERFACE
}

unsafe extern "system" fn in_archive_add_ref(this: *mut IInArchive) -> u32 {
    let aion = this as *mut AionInArchive;
    (*aion).ref_count.fetch_add(1, Ordering::SeqCst) + 1
}

unsafe extern "system" fn in_archive_release(this: *mut IInArchive) -> u32 {
    let aion = this as *mut AionInArchive;
    let prev = (*aion).ref_count.fetch_sub(1, Ordering::SeqCst);
    if prev == 1 {
        let _ = Box::from_raw(aion);
        0
    } else {
        prev - 1
    }
}

unsafe extern "system" fn in_archive_open(
    this: *mut IInArchive,
    stream: *mut IInStream,
    _max_check_start_position: *const u64,
    _open_callback: *mut IArchiveOpenCallback,
) -> HRESULT {
    if stream.is_null() {
        return E_INVALIDARG;
    }

    let aion = &mut *(this as *mut AionInArchive);
    let mut com_stream = ComInStream::new(stream);

    let total_size = match com_stream.get_size() {
        Ok(sz) => sz,
        Err(_) => return E_FAIL,
    };

    match PakArchive::from_reader(&mut com_stream, total_size) {
        Ok(arc) => {
            aion.kinds = vec![None; arc.entries().len()];
            aion.archive = Some(arc);
            aion.stream = Some(com_stream);
            S_OK
        }
        Err(_) => S_FALSE,
    }
}

unsafe extern "system" fn in_archive_close(this: *mut IInArchive) -> HRESULT {
    let aion = &mut *(this as *mut AionInArchive);
    aion.archive = None;
    aion.stream = None;
    aion.kinds.clear();
    S_OK
}

unsafe extern "system" fn in_archive_get_number_of_items(
    this: *mut IInArchive,
    num_items: *mut u32,
) -> HRESULT {
    if num_items.is_null() {
        return E_INVALIDARG;
    }

    let aion = &*(this as *mut AionInArchive);
    match &aion.archive {
        Some(arc) => {
            *num_items = arc.entries().len() as u32;
            S_OK
        }
        None => E_FAIL,
    }
}

/// Content kind of an entry, computed with the same decoding path as extraction and cached.
fn content_kind(aion: &mut AionInArchive, index: usize) -> Option<ContentKind> {
    if let Some(cached) = aion.kinds.get(index) {
        if let Some(result) = cached {
            return *result;
        }
    } else {
        return None;
    }
    let result = match (&aion.archive, &mut aion.stream) {
        (Some(arc), Some(stream)) => arc
            .entries()
            .get(index)
            .filter(|e| !e.directory)
            .and_then(|e| PakArchive::classify_entry_from_reader(stream, e).ok())
            .flatten(),
        _ => None,
    };
    aion.kinds[index] = Some(result);
    result
}
unsafe extern "system" fn in_archive_get_property(
    this: *mut IInArchive,
    index: u32,
    prop_id: u32,
    value: *mut PROPVARIANT,
) -> HRESULT {
    if value.is_null() {
        return E_INVALIDARG;
    }

    let aion = &mut *(this as *mut AionInArchive);
    let entry = match &aion.archive {
        Some(a) => match a.entries().get(index as usize) {
            Some(e) => e,
            None => return E_INVALIDARG,
        },
        None => return E_FAIL,
    };
    let val = &mut *value;
    val.set_empty();

    match prop_id {
        KPID_PATH => {
            // Directory entries are stored as "dir/sub/"; 7-Zip expects no trailing
            // separator, otherwise it creates an extra "_" folder (empty name).
            val.set_str(entry.path.trim_end_matches('/'));
        }
        KPID_NAME => {
            let trimmed = entry.path.trim_end_matches('/');
            let name = trimmed.rsplit('/').next().unwrap_or(trimmed);
            val.set_str(name);
        }
        KPID_SIZE => {
            val.set_u64(entry.uncompressed_size as u64);
        }
        KPID_PACK_SIZE => {
            val.set_u64(entry.compressed_size as u64);
        }
        KPID_IS_DIR => {
            val.set_bool(entry.directory);
        }
        KPID_CRC => {
            val.set_u32(entry.crc32);
        }
        KPID_METHOD => {
            if entry.method == 8 {
                val.set_str("Deflate");
            } else if entry.method == 0 {
                val.set_str("Store");
            } else {
                val.set_u32(entry.method as u32);
            }
        }
        KPID_COMMENT => {
            if let Some(kind) = content_kind(aion, index as usize) {
                val.set_str(kind.name());
            }
        }
        _ => {}
    }

    S_OK
}

unsafe extern "system" fn in_archive_extract(
    this: *mut IInArchive,
    indices: *const u32,
    num_items: u32,
    test_mode: i32,
    extract_callback: *mut IArchiveExtractCallback,
) -> HRESULT {
    if extract_callback.is_null() {
        return E_INVALIDARG;
    }

    let aion = &mut *(this as *mut AionInArchive);
    let arc = match &aion.archive {
        Some(a) => a,
        None => return E_FAIL,
    };
    let stream = match &mut aion.stream {
        Some(s) => s,
        None => return E_FAIL,
    };

    let entries = arc.entries();
    let count = if num_items == u32::MAX {
        entries.len() as u32
    } else {
        num_items
    };

    let mut total_bytes: u64 = 0;
    for i in 0..count {
        let idx = if indices.is_null() || num_items == u32::MAX {
            i
        } else {
            *indices.add(i as usize)
        };
        if let Some(entry) = entries.get(idx as usize) {
            total_bytes += entry.uncompressed_size as u64;
        }
    }

    let cb = &*(*extract_callback).vtable;
    (cb.set_total)(extract_callback, total_bytes);

    let mut completed_bytes: u64 = 0;

    for i in 0..count {
        let idx = if indices.is_null() || num_items == u32::MAX {
            i
        } else {
            *indices.add(i as usize)
        };

        let entry = match entries.get(idx as usize) {
            Some(e) => e,
            None => return E_INVALIDARG,
        };

        let ask_mode = if test_mode != 0 {
            NASK_MODE_TEST
        } else {
            NASK_MODE_EXTRACT
        };

        let mut out_stream: *mut ISequentialOutStream = null_mut();
        let hr = (cb.get_stream)(extract_callback, idx, &mut out_stream, ask_mode);
        if hr != S_OK {
            return hr;
        }

        (cb.prepare_operation)(extract_callback, ask_mode);

        let extract_result = PakArchive::extract_entry_from_reader(stream, entry);

        if ask_mode == NASK_MODE_EXTRACT && !out_stream.is_null() {
            match extract_result {
                Ok(data) => {
                    let mut processed: u32 = 0;
                    let write_hr = ((*(*out_stream).vtable).write)(
                        out_stream,
                        data.as_ptr(),
                        data.len() as u32,
                        &mut processed,
                    );
                    let release_fn = (*(*out_stream).vtable).release;
                    release_fn(out_stream);

                    if write_hr != S_OK {
                        (cb.set_operation_result)(extract_callback, NOPERATION_RESULT_DATA_ERROR);
                    } else {
                        (cb.set_operation_result)(extract_callback, NOPERATION_RESULT_OK);
                    }
                }
                Err(_) => {
                    let release_fn = (*(*out_stream).vtable).release;
                    release_fn(out_stream);
                    (cb.set_operation_result)(extract_callback, NOPERATION_RESULT_DATA_ERROR);
                }
            }
        } else if ask_mode == NASK_MODE_TEST {
            match extract_result {
                Ok(_) => {
                    (cb.set_operation_result)(extract_callback, NOPERATION_RESULT_OK);
                }
                Err(_) => {
                    (cb.set_operation_result)(extract_callback, NOPERATION_RESULT_DATA_ERROR);
                }
            }
        }

        completed_bytes += entry.uncompressed_size as u64;
        (cb.set_completed)(extract_callback, &completed_bytes);
    }

    S_OK
}

unsafe extern "system" fn in_archive_get_archive_property(
    this: *mut IInArchive,
    prop_id: u32,
    value: *mut PROPVARIANT,
) -> HRESULT {
    if value.is_null() {
        return E_INVALIDARG;
    }
    let val = &mut *value;
    val.set_empty();

    if prop_id == KPID_COMMENT {
        let aion = &mut *(this as *mut AionInArchive);
        let mut counts = [0usize; 2];
        for i in 0..aion.kinds.len() {
            match content_kind(aion, i) {
                Some(ContentKind::BinaryXml) => counts[0] += 1,
                Some(ContentKind::EncryptedHtml) => counts[1] += 1,
                _ => {}
            }
        }
        val.set_str(&format!(
            "Binary XMLs: {}, Encrypted HTMLs: {}",
            counts[0], counts[1]
        ));
    }
    S_OK
}

unsafe extern "system" fn in_archive_get_number_of_properties(
    _this: *mut IInArchive,
    num_properties: *mut u32,
) -> HRESULT {
    if num_properties.is_null() {
        return E_INVALIDARG;
    }
    *num_properties = 7;
    S_OK
}

unsafe extern "system" fn in_archive_get_property_info(
    _this: *mut IInArchive,
    index: u32,
    _name: *mut *mut u16,
    prop_id: *mut u32,
    var_type: *mut VARTYPE,
) -> HRESULT {
    if prop_id.is_null() || var_type.is_null() {
        return E_INVALIDARG;
    }

    match index {
        0 => {
            *prop_id = KPID_PATH;
            *var_type = VT_BSTR;
        }
        1 => {
            *prop_id = KPID_SIZE;
            *var_type = VT_UI8;
        }
        2 => {
            *prop_id = KPID_PACK_SIZE;
            *var_type = VT_UI8;
        }
        3 => {
            *prop_id = KPID_IS_DIR;
            *var_type = VT_BOOL;
        }
        4 => {
            *prop_id = KPID_CRC;
            *var_type = VT_UI4;
        }
        5 => {
            *prop_id = KPID_METHOD;
            *var_type = VT_BSTR;
        }
        6 => {
            *prop_id = KPID_COMMENT;
            *var_type = VT_BSTR;
        }
        _ => return E_INVALIDARG,
    }

    S_OK
}

unsafe extern "system" fn in_archive_get_number_of_archive_properties(
    _this: *mut IInArchive,
    num_properties: *mut u32,
) -> HRESULT {
    if num_properties.is_null() {
        return E_INVALIDARG;
    }
    *num_properties = 1;
    S_OK
}

unsafe extern "system" fn in_archive_get_archive_property_info(
    _this: *mut IInArchive,
    index: u32,
    name: *mut *mut u16,
    prop_id: *mut u32,
    var_type: *mut VARTYPE,
) -> HRESULT {
    if prop_id.is_null() || var_type.is_null() {
        return E_INVALIDARG;
    }
    if !name.is_null() {
        *name = null_mut();
    }
    if index != 0 {
        return E_INVALIDARG;
    }
    *prop_id = KPID_COMMENT;
    *var_type = VT_BSTR;
    S_OK
}

unsafe extern "system" fn codecs_info_query_interface(
    this: *mut ISetCompressCodecsInfo,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    let aion = (this as usize - std::mem::offset_of!(AionInArchive, set_compress_codecs_info_vtable)) as *mut IInArchive;
    in_archive_query_interface(aion, iid, out)
}

unsafe extern "system" fn codecs_info_add_ref(this: *mut ISetCompressCodecsInfo) -> u32 {
    let aion = (this as usize - std::mem::offset_of!(AionInArchive, set_compress_codecs_info_vtable)) as *mut IInArchive;
    in_archive_add_ref(aion)
}

unsafe extern "system" fn codecs_info_release(this: *mut ISetCompressCodecsInfo) -> u32 {
    let aion = (this as usize - std::mem::offset_of!(AionInArchive, set_compress_codecs_info_vtable)) as *mut IInArchive;
    in_archive_release(aion)
}

unsafe extern "system" fn codecs_info_set_compress_codecs_info(
    _this: *mut ISetCompressCodecsInfo,
    _compress_codecs_info: *mut c_void,
) -> HRESULT {
    S_OK
}
