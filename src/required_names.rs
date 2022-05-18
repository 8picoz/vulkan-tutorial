use ash::extensions::khr::Swapchain;
use std::ffi::CStr;

//使用を要求するデバイス拡張の名前一覧取得
pub fn get_required_device_extensions() -> [&'static CStr; 1] {
    // presentation queueのサポートがされていればSwapchainのサポートもされていることになるがそれでも一応確認はしておいたほうが良い
    [Swapchain::name()]
}
