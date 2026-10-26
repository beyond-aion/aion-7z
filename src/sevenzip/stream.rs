use super::com::*;
use core::ffi::c_void;
use std::io::{self, Read, Seek, SeekFrom};

#[repr(C)]
pub struct IInStreamVTable {
    // IUnknown
    pub query_interface: unsafe extern "system" fn(this: *mut IInStream, iid: *const GUID, out: *mut *mut c_void) -> HRESULT,
    pub add_ref: unsafe extern "system" fn(this: *mut IInStream) -> u32,
    pub release: unsafe extern "system" fn(this: *mut IInStream) -> u32,
    // ISequentialInStream
    pub read: unsafe extern "system" fn(this: *mut IInStream, data: *mut u8, size: u32, processed_size: *mut u32) -> HRESULT,
    // IInStream
    pub seek: unsafe extern "system" fn(this: *mut IInStream, offset: i64, seek_origin: u32, new_position: *mut u64) -> HRESULT,
}

#[repr(C)]
pub struct IInStream {
    pub vtable: *const IInStreamVTable,
}

#[repr(C)]
pub struct ISequentialOutStreamVTable {
    // IUnknown
    pub query_interface: unsafe extern "system" fn(this: *mut ISequentialOutStream, iid: *const GUID, out: *mut *mut c_void) -> HRESULT,
    pub add_ref: unsafe extern "system" fn(this: *mut ISequentialOutStream) -> u32,
    pub release: unsafe extern "system" fn(this: *mut ISequentialOutStream) -> u32,
    // ISequentialOutStream
    pub write: unsafe extern "system" fn(this: *mut ISequentialOutStream, data: *const u8, size: u32, processed_size: *mut u32) -> HRESULT,
}

#[repr(C)]
pub struct ISequentialOutStream {
    pub vtable: *const ISequentialOutStreamVTable,
}

#[repr(C)]
pub struct IArchiveExtractCallbackVTable {
    // IUnknown
    pub query_interface: unsafe extern "system" fn(this: *mut IArchiveExtractCallback, iid: *const GUID, out: *mut *mut c_void) -> HRESULT,
    pub add_ref: unsafe extern "system" fn(this: *mut IArchiveExtractCallback) -> u32,
    pub release: unsafe extern "system" fn(this: *mut IArchiveExtractCallback) -> u32,
    // IProgress
    pub set_total: unsafe extern "system" fn(this: *mut IArchiveExtractCallback, total: u64) -> HRESULT,
    pub set_completed: unsafe extern "system" fn(this: *mut IArchiveExtractCallback, complete_value: *const u64) -> HRESULT,
    // IArchiveExtractCallback
    pub get_stream: unsafe extern "system" fn(
        this: *mut IArchiveExtractCallback,
        index: u32,
        out_stream: *mut *mut ISequentialOutStream,
        ask_extract_mode: i32,
    ) -> HRESULT,
    pub prepare_operation: unsafe extern "system" fn(this: *mut IArchiveExtractCallback, ask_extract_mode: i32) -> HRESULT,
    pub set_operation_result: unsafe extern "system" fn(this: *mut IArchiveExtractCallback, result_e_operation_result: i32) -> HRESULT,
}

#[repr(C)]
pub struct IArchiveExtractCallback {
    pub vtable: *const IArchiveExtractCallbackVTable,
}

#[repr(C)]
pub struct IArchiveOpenCallbackVTable {
    // IUnknown
    pub query_interface: unsafe extern "system" fn(this: *mut IArchiveOpenCallback, iid: *const GUID, out: *mut *mut c_void) -> HRESULT,
    pub add_ref: unsafe extern "system" fn(this: *mut IArchiveOpenCallback) -> u32,
    pub release: unsafe extern "system" fn(this: *mut IArchiveOpenCallback) -> u32,
    // IArchiveOpenCallback
    pub set_total: unsafe extern "system" fn(this: *mut IArchiveOpenCallback, files: *const u64, bytes: *const u64) -> HRESULT,
    pub set_completed: unsafe extern "system" fn(this: *mut IArchiveOpenCallback, files: *const u64, bytes: *const u64) -> HRESULT,
}

#[repr(C)]
pub struct IArchiveOpenCallback {
    pub vtable: *const IArchiveOpenCallbackVTable,
}

pub struct ComInStream {
    stream: *mut IInStream,
}

impl ComInStream {
    pub fn new(stream: *mut IInStream) -> Self {
        if !stream.is_null() {
            unsafe {
                let add_ref = (*(*stream).vtable).add_ref;
                add_ref(stream);
            }
        }
        Self { stream }
    }

    pub fn get_size(&mut self) -> io::Result<u64> {
        let current = self.seek(SeekFrom::Current(0))?;
        let size = self.seek(SeekFrom::End(0))?;
        self.seek(SeekFrom::Start(current))?;
        Ok(size)
    }

    pub fn is_valid(&self) -> bool {
        !self.stream.is_null()
    }
}

impl Drop for ComInStream {
    fn drop(&mut self) {
        if !self.stream.is_null() {
            unsafe {
                let release = (*(*self.stream).vtable).release;
                release(self.stream);
            }
            self.stream = core::ptr::null_mut();
        }
    }
}

impl Read for ComInStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.stream.is_null() {
            return Err(io::Error::new(io::ErrorKind::Other, "Null stream"));
        }
        let mut processed: u32 = 0;
        let hr = unsafe {
            let read_fn = (*(*self.stream).vtable).read;
            read_fn(self.stream, buf.as_mut_ptr(), buf.len() as u32, &mut processed)
        };
        if hr != S_OK {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("IInStream::Read failed with HRESULT 0x{:08X}", hr),
            ));
        }
        Ok(processed as usize)
    }
}

impl Seek for ComInStream {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        if self.stream.is_null() {
            return Err(io::Error::new(io::ErrorKind::Other, "Null stream"));
        }
        let (offset, origin) = match pos {
            SeekFrom::Start(off) => (off as i64, STREAM_SEEK_SET),
            SeekFrom::Current(off) => (off, STREAM_SEEK_CUR),
            SeekFrom::End(off) => (off, STREAM_SEEK_END),
        };
        let mut new_pos: u64 = 0;
        let hr = unsafe {
            let seek_fn = (*(*self.stream).vtable).seek;
            seek_fn(self.stream, offset, origin, &mut new_pos)
        };
        if hr != S_OK {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("IInStream::Seek failed with HRESULT 0x{:08X}", hr),
            ));
        }
        Ok(new_pos)
    }
}
