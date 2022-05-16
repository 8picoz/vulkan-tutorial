use ash::extensions::khr::{Surface, Win32Surface};
use ash::prelude::VkResult;
use ash::{vk, Entry, Instance, RawPtr};
use std::mem;

pub fn require_extension_names() -> Vec<*const i8> {
    vec![Surface::name().as_ptr(), Win32Surface::name().as_ptr()]
}

//WindowsのSurfaceを取るときの関数
//今回はashのcreate_surfaceを使用してるのでこれはdead_code
#[allow(dead_code)]
pub unsafe fn create_win32_surface(
    entry: &Entry,
    instance: &Instance,
    create_info: &vk::Win32SurfaceCreateInfoKHR,
    allocation_callbacks: Option<&vk::AllocationCallbacks>,
) -> VkResult<vk::SurfaceKHR> {
    let handle = instance.handle();

    let fp = ash::vk::KhrWin32SurfaceFn::load(|name| unsafe {
        mem::transmute(entry.get_instance_proc_addr(handle, name.as_ptr()))
    });

    let mut surface = mem::zeroed();

    (fp.create_win32_surface_khr)(
        handle,
        create_info,
        allocation_callbacks.as_raw_ptr(),
        &mut surface,
    )
    .result_with_success(surface)
}
