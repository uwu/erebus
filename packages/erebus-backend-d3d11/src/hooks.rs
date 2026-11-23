//! Hooks into D3D11 functions.

use std::{cell::Cell, ffi::c_void, mem::transmute, thread::LocalKey};

use erebus_backend::themes;

use crate::{
    d3d11::{
        D3D11_BIND_SHADER_RESOURCE, D3D11_BUFFER_DESC, D3D11_CPU_ACCESS_WRITE,
        D3D11_MAP_WRITE_DISCARD, D3D11_MAPPED_SUBRESOURCE, D3D11_RESOURCE_DIMENSION_BUFFER,
        D3D11_RESOURCE_MISC_BUFFER_STRUCTURED, D3D11_USAGE_DYNAMIC,
    },
    shadow::{ShadowBuf, is_primitive_stride},
    vtable::{
        self, MapFn, TrackedShadow, UnmapFn, VT_BUFFER_GET_DESC, VT_CTX_MAP, VT_CTX_UNMAP,
        VT_RES_GET_TYPE, tracked,
    },
};
use windows_sys::core::{GUID, HRESULT};

thread_local! {
    static IN_MAP: Cell<bool> = const { Cell::new(false) };
    static IN_UNMAP: Cell<bool> = const { Cell::new(false) };
}
struct HookGuard(&'static LocalKey<Cell<bool>>);
impl HookGuard {
    fn enter(key: &'static LocalKey<Cell<bool>>) -> Option<Self> {
        if key.with(|flag| flag.replace(true)) {
            None
        } else {
            Some(Self(key))
        }
    }
}
impl Drop for HookGuard {
    fn drop(&mut self) {
        self.0.with(|flag| flag.set(false));
    }
}

/// D3D11 rewrites its per-context method table when thread protection changes.
///
/// Native detours remain installed across those rewrites and cover other contexts
/// using the same implementation. Each detour has its own trampoline so nested
/// protected/unprotected wrappers can forward without applying a theme twice.
pub unsafe fn device_create(device: *mut c_void, context: *mut c_void) {
    let mut context = context;
    let acquired = context.is_null() && !device.is_null();
    if acquired {
        let get: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) =
            unsafe { transmute(vtable::entry(device, 40)) };
        unsafe { get(device, &mut context) };
    }

    if context.is_null() {
        return;
    }

    themes::start("d3d11");
    unsafe { install_context(context) };

    let query: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT =
        unsafe { transmute(vtable::entry(context, 0)) };
    let iid = GUID::from_u128(0x9b7e4e00_342c_4106_a19f_4f2704f689f0);
    let mut mt = std::ptr::null_mut();

    if unsafe { query(context, &iid, &mut mt) } >= 0 && !mt.is_null() {
        unsafe {
            install_protection_hook(mt);
            release(mt);
        }
    }

    if acquired {
        unsafe { release(context) }
    }
}
unsafe fn release(object: *mut c_void) {
    let release: unsafe extern "system" fn(*mut c_void) -> u32 =
        unsafe { transmute(vtable::entry(object, 2)) };
    unsafe { release(object) };
}

unsafe fn install_context(context: *mut c_void) {
    for (slot, is_map) in [(VT_CTX_MAP, true), (VT_CTX_UNMAP, false)] {
        let target = unsafe { vtable::entry(context, slot) } as *mut c_void;
        let mut guard = tracked();
        let state = guard.as_mut().unwrap();
        let (exists, index) = if is_map {
            (
                state.maps.iter().any(|(addr, _)| *addr == target.addr()),
                state.maps.len(),
            )
        } else {
            (
                state.unmaps.iter().any(|(addr, _)| *addr == target.addr()),
                state.unmaps.len(),
            )
        };

        if exists {
            continue;
        }
        if index >= MAP_DETOURS.len() {
            log::error!(target:"erebus_d3d11","too many native D3D11 implementations");
            continue;
        }

        let detour = if is_map {
            MAP_DETOURS[index] as *mut c_void
        } else {
            UNMAP_DETOURS[index] as *mut c_void
        };

        match unsafe { minhook::MinHook::create_hook(target, detour) } {
            Ok(original) => {
                unsafe {
                    if is_map {
                        state.maps.push((target.addr(), transmute(original)));
                    } else {
                        state.unmaps.push((target.addr(), transmute(original)));
                    }
                }
                drop(guard);
                match unsafe { minhook::MinHook::enable_hook(target) } {
                    Ok(()) => {
                        log::info!(target:"erebus_d3d11","installed native {} hook at {target:p}",if is_map{"Map"}else{"Unmap"})
                    }
                    Err(err) => {
                        log::error!(target:"erebus_d3d11","enable native D3D11 hook: {err:?}")
                    }
                }
            }
            Err(err) => log::error!(target:"erebus_d3d11","create native D3D11 hook: {err:?}"),
        }
    }
}

unsafe fn install_protection_hook(mt: *mut c_void) {
    let target = unsafe { vtable::entry(mt, 5) } as *mut c_void;
    let mut guard = tracked();
    let state = guard.as_mut().unwrap();
    if state.multithreads.contains_key(&target.addr()) {
        return;
    }
    match unsafe { minhook::MinHook::create_hook(target, SetMultithreadProtected as *mut c_void) } {
        Ok(original) => {
            unsafe {
                state
                    .multithreads
                    .insert(target.addr(), transmute(original));
            }
            drop(guard);
            if let Err(err) = unsafe { minhook::MinHook::enable_hook(target) } {
                log::error!(target:"erebus_d3d11","enable thread protection hook: {err:?}");
            }
        }
        Err(err) => log::error!(target:"erebus_d3d11","create thread protection hook: {err:?}"),
    }
}

unsafe extern "system" fn SetMultithreadProtected(this: *mut c_void, enabled: i32) -> i32 {
    let target = unsafe { vtable::entry(this, 5) };
    let original = tracked().as_ref().unwrap().multithreads[&target.addr()];
    let previous = unsafe { original(this, enabled) };
    let query: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT =
        unsafe { transmute(vtable::entry(this, 0)) };
    let iid = GUID::from_u128(0xc0bfa96c_e089_44fb_8eaf_26f8796190da);
    let mut context = std::ptr::null_mut();
    if unsafe { query(this, &iid, &mut context) } >= 0 && !context.is_null() {
        unsafe {
            install_context(context);
            release(context);
        }
    }
    previous
}

macro_rules! detours {
    ($(($map:ident,$unmap:ident,$index:expr)),*)=>{
        $(unsafe extern "system" fn $map(this:*mut c_void,resource:*mut c_void,sub:u32,kind:u32,flags:u32,mapped:*mut D3D11_MAPPED_SUBRESOURCE)->HRESULT{
        unsafe{map_impl($index,this,resource,sub,kind,flags,mapped)}
    }
    unsafe extern "system" fn $unmap(this:*mut c_void,resource:*mut c_void,sub:u32){unsafe{unmap_impl($index,this,resource,sub)}})*
    const MAP_DETOURS:&[MapFn]=&[$($map),*];
    const UNMAP_DETOURS:&[UnmapFn]=&[$($unmap),*];
 }
}
detours!(
    (Map0, Unmap0, 0),
    (Map1, Unmap1, 1),
    (Map2, Unmap2, 2),
    (Map3, Unmap3, 3),
    (Map4, Unmap4, 4),
    (Map5, Unmap5, 5),
    (Map6, Unmap6, 6),
    (Map7, Unmap7, 7),
    (Map8, Unmap8, 8),
    (Map9, Unmap9, 9),
    (Map10, Unmap10, 10),
    (Map11, Unmap11, 11),
    (Map12, Unmap12, 12),
    (Map13, Unmap13, 13),
    (Map14, Unmap14, 14),
    (Map15, Unmap15, 15)
);
unsafe fn map_impl(
    index: usize,
    vptr: *mut c_void,
    resource: *mut c_void,
    subresource: u32,
    map_type: u32,
    map_flags: u32,
    mapped: *mut D3D11_MAPPED_SUBRESOURCE,
) -> HRESULT {
    let original = tracked().as_ref().unwrap().maps[index].1;
    let Some(_guard) = HookGuard::enter(&IN_MAP) else {
        return unsafe { original(vptr, resource, subresource, map_type, map_flags, mapped) };
    };
    
    unsafe {
        // we only care about stuff mapped from this function:
        //
        // <https://github.com/zed-industries/zed/blob/main/crates/gpui_windows/src/directx_renderer.rs#L1637>
        let layout = if subresource == 0
            && map_type == D3D11_MAP_WRITE_DISCARD
            && !mapped.is_null()
            && !resource.is_null()
        {
            primitive_buffer_layout(resource)
        } else {
            None
        };

        let result = original(vptr, resource, subresource, map_type, map_flags, mapped);
        let Some((byte_width, stride)) = layout else {
            return result;
        };
        if result < 0 || (*mapped).pData.is_null() || themes::active_themes().is_empty() {
            return result;
        }

        let mapping_addr = (*mapped).pData.cast::<u8>();
        let mut guard = tracked();
        let state = guard.as_mut().unwrap();

        // keep the table from growing forever as GPUI swaps in bigger buffers
        if state.shadows.len() > ShadowBuf::MAX_IDLE_SHADOWS {
            state
                .shadows
                .retain(|_, shadow| !shadow.real_mapping_addr.is_null());
        }

        let shadow = state
            .shadows
            .entry(resource as usize)
            .or_insert_with(|| TrackedShadow {
                buffer: ShadowBuf::new(byte_width),
                stride,
                real_mapping_addr: core::ptr::null_mut(),
            });

        if !shadow.real_mapping_addr.is_null() {
            return result;
        }

        if shadow.buffer.len != byte_width || shadow.stride != stride {
            // address reused by a different buffer
            *shadow = TrackedShadow {
                buffer: ShadowBuf::new(byte_width),
                stride,
                real_mapping_addr: core::ptr::null_mut(),
            };
        }

        shadow.real_mapping_addr = mapping_addr;
        (*mapped).pData = shadow.buffer.data.as_mut_ptr().cast();
        result
    }
}

unsafe fn unmap_impl(index: usize, this: *mut c_void, resource: *mut c_void, subresource: u32) {
    let original = tracked().as_ref().unwrap().unmaps[index].1;
    let Some(_guard) = HookGuard::enter(&IN_UNMAP) else {
        unsafe { original(this, resource, subresource) };
        return;
    };
    if subresource == 0 {
        let mut guard = tracked();
        let state = guard.as_mut().unwrap();

        if let Some(shadow) = state.shadows.get_mut(&(resource as usize))
            && !shadow.real_mapping_addr.is_null()
        {
            let real = std::mem::replace(&mut shadow.real_mapping_addr, core::ptr::null_mut());
            let stride = shadow.stride;

            let mut active = themes::active_themes();
            _ = unsafe { shadow.buffer.flush(stride, &mut active, real) };
        }
    }

    unsafe { original(this, resource, subresource) }
}

/// If resource contains GPUI primitives, calculate its (byte_width, stride)`.
///
/// This is deliberately strict: `DYNAMIC` + `SHADER_RESOURCE` + `STRUCTURED` + CPU write,
/// with a stride equal to one of our primitive sizes. Everything else (constant buffers,
/// path vertex data, staging readbacks, ...) goes straight to the driver.
unsafe fn primitive_buffer_layout(resource: *mut c_void) -> Option<(usize, usize)> {
    unsafe {
        let GetType: unsafe extern "system" fn(*mut c_void, *mut u32) =
            transmute(vtable::entry(resource, VT_RES_GET_TYPE));
        let mut dimension = 0;
        GetType(resource, &mut dimension);
        if dimension != D3D11_RESOURCE_DIMENSION_BUFFER {
            return None;
        }

        let GetDesc: unsafe extern "system" fn(*mut c_void, *mut D3D11_BUFFER_DESC) =
            transmute(vtable::entry(resource, VT_BUFFER_GET_DESC));
        let mut desc: D3D11_BUFFER_DESC = std::mem::zeroed();
        GetDesc(resource, &mut desc);

        let stride = desc.StructureByteStride as usize;

        (desc.Usage == D3D11_USAGE_DYNAMIC
            && desc.BindFlags & D3D11_BIND_SHADER_RESOURCE != 0
            && desc.CPUAccessFlags & D3D11_CPU_ACCESS_WRITE != 0
            && desc.MiscFlags & D3D11_RESOURCE_MISC_BUFFER_STRUCTURED != 0
            && desc.ByteWidth <= ShadowBuf::MAX_LEN
            && is_primitive_stride(stride)
            && desc.ByteWidth as usize % stride == 0)
            .then_some((desc.ByteWidth as usize, stride))
    }
}
