use std::ffi::CStr;
use std::os::raw::c_char;

// build.rs에서 .compile("metal_info") 한 이름과 일치해야 함
#[cfg_attr(target_os = "macos", link(name = "metal_info"))]
unsafe extern "C" {
    fn slvt_metal_is_supported() -> bool;
    fn slvt_metal_device_name() -> *mut c_char; // strdup이므로 *mut
    fn slvt_metal_recommended_max_working_set_size() -> u64;
    fn slvt_metal_has_unified_memory() -> bool;
}

unsafe extern "C" {
    // SalvationFFI.h 에 선언된 함수들 여기에 직접 작성
    // 예시 — 실제 SalvationFFI.h 내용에 맞게 수정
    pub fn salvation_add(a: i32, b: i32) -> i32;
    pub fn salvation_log(message: *const c_char);
}

pub fn is_supported() -> bool {
    #[cfg(target_os = "macos")]
    unsafe { slvt_metal_is_supported() }

    #[cfg(not(target_os = "macos"))]
    false
}

pub fn device_name() -> Option<String> {
    #[cfg(target_os = "macos")]
    unsafe {
        let ptr = slvt_metal_device_name();
        if ptr.is_null() {
            return None;
        }
        let s = CStr::from_ptr(ptr).to_string_lossy().into_owned();
        // strdup()으로 할당된 메모리 해제
        libc::free(ptr as *mut _);
        Some(s)
    }

    #[cfg(not(target_os = "macos"))]
    None
}

pub fn recommended_max_working_set_size() -> u64 {
    #[cfg(target_os = "macos")]
    unsafe { slvt_metal_recommended_max_working_set_size() }

    #[cfg(not(target_os = "macos"))]
    0
}

pub fn has_unified_memory() -> bool {
    #[cfg(target_os = "macos")]
    unsafe { slvt_metal_has_unified_memory() }

    #[cfg(not(target_os = "macos"))]
    false
}