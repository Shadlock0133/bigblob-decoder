use std::ffi::{c_int, c_uchar, c_uint, c_void};

#[link(name = "CMP_Core", kind = "static")]
extern "cdecl" {
    #[link_name = "?CompressBlockBC7@@YAHPEBEIQEAEPEBX@Z"]
    pub fn CompressBlockBC7(
        srcBlock: *const c_uchar,
        srcStrideInBytes: c_uint,
        cmpBlock: *mut [c_uchar; 16],
        options: *const c_void,
    ) -> c_int;
}
