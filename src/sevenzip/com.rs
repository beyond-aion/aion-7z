use core::ffi::c_void;

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct GUID {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

pub type HRESULT = i32;

pub const S_OK: HRESULT = 0;
pub const S_FALSE: HRESULT = 1;
pub const E_NOTIMPL: HRESULT = 0x80004001u32 as i32;
pub const E_NOINTERFACE: HRESULT = 0x80004002u32 as i32;
pub const E_POINTER: HRESULT = 0x80004003u32 as i32;
pub const E_ABORT: HRESULT = 0x80004004u32 as i32;
pub const E_FAIL: HRESULT = 0x80004005u32 as i32;
pub const E_OUTOFMEMORY: HRESULT = 0x8007000Eu32 as i32;
pub const E_INVALIDARG: HRESULT = 0x80070057u32 as i32;
pub const CLASS_E_CLASSNOTAVAILABLE: HRESULT = 0x80040111u32 as i32;

// IUnknown: {00000000-0000-0000-C000-000000000046}
pub const IID_IUNKNOWN: GUID = GUID {
    data1: 0x00000000,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

// IInArchive: {23170F69-40C1-278A-0000-000600600000}
pub const IID_IINARCHIVE: GUID = GUID {
    data1: 0x23170F69,
    data2: 0x40C1,
    data3: 0x278A,
    data4: [0x00, 0x00, 0x00, 0x06, 0x00, 0x60, 0x00, 0x00],
};

// ISetCompressCodecsInfo: {23170F69-40C1-278A-0000-000400040000}
pub const IID_ISETCOMPRESSCODECSINFO: GUID = GUID {
    data1: 0x23170F69,
    data2: 0x40C1,
    data3: 0x278A,
    data4: [0x00, 0x00, 0x00, 0x04, 0x00, 0x04, 0x00, 0x00],
};

// CLSID for Aion PAK format: {23170F69-40C1-278A-1000-000110070000}
pub const CLSID_AION_PAK: GUID = GUID {
    data1: 0x23170F69,
    data2: 0x40C1,
    data3: 0x278A,
    data4: [0x10, 0x00, 0x00, 0x01, 0x10, 0x07, 0x00, 0x00],
};

pub type VARTYPE = u16;

pub const VT_EMPTY: VARTYPE = 0;
pub const VT_NULL: VARTYPE = 1;
pub const VT_I2: VARTYPE = 2;
pub const VT_I4: VARTYPE = 3;
pub const VT_R4: VARTYPE = 4;
pub const VT_R8: VARTYPE = 5;
pub const VT_BSTR: VARTYPE = 8;
pub const VT_BOOL: VARTYPE = 11;
pub const VT_UI1: VARTYPE = 17;
pub const VT_UI2: VARTYPE = 18;
pub const VT_UI4: VARTYPE = 19;
pub const VT_I8: VARTYPE = 20;
pub const VT_UI8: VARTYPE = 21;
pub const VT_INT: VARTYPE = 22;
pub const VT_UINT: VARTYPE = 23;
pub const VT_FILETIME: VARTYPE = 64;

pub const VARIANT_TRUE: i16 = -1;
pub const VARIANT_FALSE: i16 = 0;

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct FILETIME {
    pub dw_low_date_time: u32,
    pub dw_high_date_time: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union PropVariantData {
    pub b_val: u8,
    pub i_val: i16,
    pub ui_val: u16,
    pub l_val: i32,
    pub ul_val: u32,
    pub int_val: i32,
    pub uint_val: u32,
    pub h_val: i64,
    pub uh_val: u64,
    pub bool_val: i16,
    pub bstr_val: *mut u16,
    pub ptr_val: *mut c_void,
    pub filetime: FILETIME,
}

#[repr(C)]
pub struct PROPVARIANT {
    pub vt: VARTYPE,
    pub w_reserved1: u16,
    pub w_reserved2: u16,
    pub w_reserved3: u16,
    pub data: PropVariantData,
}

impl Default for PROPVARIANT {
    fn default() -> Self {
        Self {
            vt: VT_EMPTY,
            w_reserved1: 0,
            w_reserved2: 0,
            w_reserved3: 0,
            data: PropVariantData { uh_val: 0 },
        }
    }
}

impl PROPVARIANT {
    pub unsafe fn set_str(&mut self, s: &str) {
        self.vt = VT_BSTR;
        self.data.bstr_val = bstr_from_str(s);
    }

    pub unsafe fn set_bytes(&mut self, bytes: &[u8]) {
        self.vt = VT_BSTR;
        self.data.bstr_val = bstr_from_bytes(bytes);
    }

    pub fn set_bool(&mut self, val: bool) {
        self.vt = VT_BOOL;
        self.data.bool_val = if val { VARIANT_TRUE } else { VARIANT_FALSE };
    }

    pub fn set_u32(&mut self, val: u32) {
        self.vt = VT_UI4;
        self.data.ul_val = val;
    }

    pub fn set_u64(&mut self, val: u64) {
        self.vt = VT_UI8;
        self.data.uh_val = val;
    }

    pub fn set_empty(&mut self) {
        self.vt = VT_EMPTY;
        self.data.uh_val = 0;
    }
}

#[link(name = "oleaut32")]
unsafe extern "system" {
    pub fn SysAllocStringLen(psz: *const u16, len: u32) -> *mut u16;
    pub fn SysAllocStringByteLen(psz: *const u8, len: u32) -> *mut u16;
    pub fn SysStringLen(bstr: *mut u16) -> u32;
    pub fn SysFreeString(bstr: *mut u16);
}

pub unsafe fn bstr_to_string(bstr: *mut u16) -> String {
    if bstr.is_null() {
        return String::new();
    }
    let len = SysStringLen(bstr) as usize;
    let slice = core::slice::from_raw_parts(bstr, len);
    String::from_utf16_lossy(slice)
}

pub unsafe fn bstr_from_str(s: &str) -> *mut u16 {
    let wide: Vec<u16> = s.encode_utf16().collect();
    SysAllocStringLen(wide.as_ptr(), wide.len() as u32)
}

pub unsafe fn bstr_from_bytes(bytes: &[u8]) -> *mut u16 {
    SysAllocStringByteLen(bytes.as_ptr(), bytes.len() as u32)
}

// 7-Zip item property IDs (PROPID)
pub const KPID_PATH: u32 = 3;
pub const KPID_NAME: u32 = 4;
pub const KPID_IS_DIR: u32 = 6;
pub const KPID_SIZE: u32 = 7;
pub const KPID_PACK_SIZE: u32 = 8;
pub const KPID_CRC: u32 = 19;
pub const KPID_METHOD: u32 = 22;
pub const KPID_COMMENT: u32 = 28;

// Archive handler property IDs (NArchive)
pub const NARCHIVE_NAME: u32 = 0;
pub const NARCHIVE_CLASS_ID: u32 = 1;
pub const NARCHIVE_EXTENSION: u32 = 2;
pub const NARCHIVE_ADD_EXTENSION: u32 = 3;
pub const NARCHIVE_UPDATE: u32 = 4;
pub const NARCHIVE_KEEP_NAME: u32 = 5;
pub const NARCHIVE_START_SIGNATURE: u32 = 6;
pub const NARCHIVE_FINISH_SIGNATURE: u32 = 7;
pub const NARCHIVE_ASSOCIATE: u32 = 8;
pub const NARCHIVE_SIGNATURE: u32 = 9;
pub const NARCHIVE_SIGNATURE_OFFSET: u32 = 10;
pub const NARCHIVE_FLAGS: u32 = 12;

// Extraction operation results (NExtract::NOperationResult)
pub const NOPERATION_RESULT_OK: i32 = 0;
pub const NOPERATION_RESULT_UNSUPPORTED_METHOD: i32 = 1;
pub const NOPERATION_RESULT_DATA_ERROR: i32 = 2;
pub const NOPERATION_RESULT_CRC_ERROR: i32 = 3;
pub const NOPERATION_RESULT_UNAVAILABLE: i32 = 4;
pub const NOPERATION_RESULT_UNEXPECTED_END: i32 = 5;
pub const NOPERATION_RESULT_DATA_AFTER_END: i32 = 6;
pub const NOPERATION_RESULT_IS_NOT_ARC: i32 = 7;
pub const NOPERATION_RESULT_HEADERS_ERROR: i32 = 8;
pub const NOPERATION_RESULT_WRONG_PASSWORD: i32 = 9;

// Ask extraction modes (NAskMode)
pub const NASK_MODE_EXTRACT: i32 = 0;
pub const NASK_MODE_TEST: i32 = 1;
pub const NASK_MODE_SKIP: i32 = 2;

// Seek origins
pub const STREAM_SEEK_SET: u32 = 0;
pub const STREAM_SEEK_CUR: u32 = 1;
pub const STREAM_SEEK_END: u32 = 2;
