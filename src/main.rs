mod khr_util;

use ash::prelude::VkResult;
use ash::vk::DebugUtilsMessengerEXT;
use ash::{extensions::ext::DebugUtils, vk, Entry, Instance};
use log::log;
use std::{
    error::Error,
    ffi::{c_void, CStr, CString},
    result::Result,
};

#[cfg(debug_assertions)]
const ENABLE_VALIDATION_LAYERS: bool = true;

#[cfg(not(debug_assertions))]
const ENABLE_VALIDATION_LAYERS: bool = false;

//Validation Layerで必要な機能一覧
const REQUIRED_LAYERS: [&'static str; 1] = ["VK_LAYER_KHRONOS_validation"];

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

struct VulkanApp {
    _entry: Entry,
    instance: Instance,
    debug_utils: Option<DebugUtils>,
    debug_utils_messenger_ext: Option<DebugUtilsMessengerEXT>,
}

impl VulkanApp {
    fn new() -> Result<Self, Box<dyn Error>> {
        log::debug!("Creating application");

        let entry = unsafe { Entry::load().expect("Failed to create entry.") };
        let instance = Self::create_instance(&entry)?;

        let mut debug_utils = None;
        let mut debug_utils_messenger_ext = None;

        if ENABLE_VALIDATION_LAYERS {
            let _debug_utils = DebugUtils::new(&entry, &instance);

            debug_utils_messenger_ext = Some(
                Self::setup_debug_utils_messenger_ext(&_debug_utils)
                    .unwrap_or_else(|e| panic!("{}", e)),
            );

            debug_utils = Some(_debug_utils);
        }

        Ok(Self {
            _entry: entry,
            instance,
            debug_utils,
            debug_utils_messenger_ext,
        })
    }

    fn run(&mut self) {
        log::info!("Running application");
    }

    fn create_instance(entry: &Entry) -> Result<Instance, Box<dyn Error>> {
        let app_info = vk::ApplicationInfo::builder()
            .application_name(CString::new("vulkan app")?.as_c_str())
            .application_version(0)
            .engine_name(CString::new("No Engine")?.as_c_str()) //エンジン名を入力するとそれが既知なエンジンだったらそれように最適化をする
            .engine_version(0)
            .api_version(vk::make_api_version(0, 1, 2, 0)) //Vulkan自体のバージョン
            .build();

        let mut extension_names = khr_util::require_extension_names(); //本家チュートリアルではgetRequiredExtensions(glfwGetRequiredInstanceExtensions)

        //検証レイヤーでのデバック時にコールバックを設定できるように拡張機能を有効にする
        if ENABLE_VALIDATION_LAYERS {
            //DebugUtils::name()がVK_EXT_DEBUG_UTILS_EXTENSION_NAME
            extension_names.push(DebugUtils::name().as_ptr());
        }

        let layer_names = REQUIRED_LAYERS
            .iter()
            .map(|name| CString::new(*name).expect("Failed to build CString"))
            .collect::<Vec<_>>();
        let layer_names_ptrs = layer_names
            .iter()
            .map(|name| name.as_ptr())
            .collect::<Vec<_>>();

        let mut instance_create_info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(&extension_names);

        if ENABLE_VALIDATION_LAYERS {
            Self::check_validation_layer_support(entry);

            //enabled_layer_countのセットはenabled_layer_namesの中に入っている
            instance_create_info = instance_create_info.enabled_layer_names(&layer_names_ptrs);
        }

        unsafe { Ok(entry.create_instance(&instance_create_info, None)?) } //基本的に本家で返り値がVkResultなものはResult型で値が包まれて返ってくるので引数も減る
    }

    //指定されたレイヤーの検証レイヤーが有効かどうか
    fn check_validation_layer_support(entry: &Entry) {
        for required in REQUIRED_LAYERS.iter() {
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

    fn setup_debug_utils_messenger_ext(
        debug_utils: &DebugUtils,
    ) -> VkResult<DebugUtilsMessengerEXT> {
        let create_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
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
            .build();

        //よくVkDebugReportCallbackで代用しているのを見る
        unsafe { debug_utils.create_debug_utils_messenger(&create_info, None) }
    }
}

impl Drop for VulkanApp {
    fn drop(&mut self) {
        log::debug!("Dropping application.");
        unsafe {
            if let Some(debug_utils) = &self.debug_utils {
                debug_utils.destroy_debug_utils_messenger(
                    self.debug_utils_messenger_ext
                        .expect("DebugUtilsMessengerEXTが存在しません"),
                    None,
                );
            }

            self.instance.destroy_instance(None); //ライフタイムが聞いてても呼ばないと駄目
        }
    }
}

fn main() {
    env_logger::init();
    match VulkanApp::new() {
        Ok(mut app) => app.run(),
        Err(error) => log::error!("Failed to create application. Cause: {}", error),
    }
}
