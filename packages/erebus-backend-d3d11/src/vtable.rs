//! D3D11 COM entries and native hook state.
use crate::{d3d11::D3D11_MAPPED_SUBRESOURCE, shadow::ShadowBuf};
use fxhash::FxHashMap;
use std::{
    ffi::c_void,
    sync::{Mutex, MutexGuard, PoisonError},
};
use windows_sys::core::HRESULT;
pub const VT_CTX_MAP: usize = 14;
pub const VT_CTX_UNMAP: usize = 15;
pub const VT_RES_GET_TYPE: usize = 7;
pub const VT_BUFFER_GET_DESC: usize = 10;
pub type MapFn = unsafe extern "system" fn(
    *mut c_void,
    *mut c_void,
    u32,
    u32,
    u32,
    *mut D3D11_MAPPED_SUBRESOURCE,
) -> HRESULT;
pub type UnmapFn = unsafe extern "system" fn(*mut c_void, *mut c_void, u32);
pub type ProtectFn = unsafe extern "system" fn(*mut c_void, i32) -> i32;
pub unsafe fn entry(object: *mut c_void, index: usize) -> *const c_void {
    unsafe { *(*(object as *const *const *const c_void)).add(index) }
}
pub struct TrackedShadow {
    pub buffer: ShadowBuf,
    pub stride: usize,
    pub real_mapping_addr: *mut u8,
}
pub struct Tracked {
    pub maps: Vec<(usize, MapFn)>,
    pub unmaps: Vec<(usize, UnmapFn)>,
    pub multithreads: FxHashMap<usize, ProtectFn>,
    pub shadows: FxHashMap<usize, TrackedShadow>,
}
unsafe impl Send for Tracked {}
static TRACKED: Mutex<Option<Tracked>> = Mutex::new(None);
pub fn tracked() -> MutexGuard<'static, Option<Tracked>> {
    let mut guard = TRACKED.lock().unwrap_or_else(PoisonError::into_inner);
    guard.get_or_insert_with(|| Tracked {
        maps: Vec::new(),
        unmaps: Vec::new(),
        multithreads: FxHashMap::default(),
        shadows: FxHashMap::default(),
    });
    guard
}
