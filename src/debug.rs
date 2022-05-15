use ash::extensions::ext::DebugUtils;
use ash::prelude::VkResult;
use ash::vk::{DebugUtilsMessengerCreateInfoEXT, DebugUtilsMessengerEXT};
use ash::{vk, Entry};
use std::ffi::{c_void, CStr};

//指定されたレイヤーの検証レイヤーが有効かどうか
pub fn check_validation_layer_support(entry: &Entry) {
    for required in crate::vulkan_app::REQUIRED_LAYERS.iter() {
        let found = entry
            .enumerate_instance_layer_properties()
            .unwrap()
            .iter()
            .any(|layer| {
                let name = unsafe { CStr::from_ptr(layer.layer_name.as_ptr()) };
                let name = name.to_str().expect("Failed to get layer name pointer");
                required == &name
            });

        if !found {
            panic!("Validation layer not supported: {}", required);
        }
    }
}

//DebugUtilsMessengerCreateInfoEXTを作成するためのもの
pub fn populate_debug_messenger_create_info() -> DebugUtilsMessengerCreateInfoEXT {
    DebugUtilsMessengerCreateInfoEXT::builder()
        //受け取ったメッセージの内容の危険度
        .message_severity(
            vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
                | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
        )
        //メッセージの種類
        .message_type(
            vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
        )
        .pfn_user_callback(Some(vulkan_debug_callback))
        .build()
}

// DebugUtilsMessengerEXTはデバック情報をvulkan_debug_callbackに渡すためのもの
pub fn setup_debug_utils_messenger_ext(
    debug_utils: &DebugUtils,
) -> VkResult<DebugUtilsMessengerEXT> {
    let create_info = populate_debug_messenger_create_info();

    //よくVkDebugReportCallbackで代用しているのを見る
    unsafe { debug_utils.create_debug_utils_messenger(&create_info, None) }
}

unsafe extern "system" fn vulkan_debug_callback(
    //受け取ったメッセージの重要度が入ったフラグ
    //比較対象の重要度より悪い状況かどうかはbitで来るので等号以外にも大なり小なりで比較することができる
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    //仕様とは違う使い方をしたりなどの原因が含まれる
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    //pMessage : null終端文字学組まれたデバッグメッセージ
    //pObjects : Vulkan object handles
    //objectCount : Number of objects in array
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    //任意のデータを設定できる
    p_user_data: *mut c_void,
) -> vk::Bool32 {
    let data = *p_callback_data;
    let message = CStr::from_ptr(data.p_message).to_string_lossy();

    log::debug!("validation layer: {:?}", message);

    //返り値はValidation Layerを中止するべきかどうかを返す
    vk::FALSE
}
