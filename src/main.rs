mod khr_util;

use std::{error::Error, result::Result, ffi::CString};
use ash::{vk, Entry, Instance};

struct VulkanApp {
    _entry: Entry,
    instance: Instance,
}

impl VulkanApp {
    fn new() -> Result<Self, Box<dyn Error>> {
        log::debug!("Creating application");

        let entry = unsafe { Entry::load().expect("Failed to create entry.") };
        let instance = Self::create_instance(&entry)?;

        Ok(Self {
            _entry: entry,
            instance,
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

        let extension_names = khr_util::require_extension_names(); //本家チュートリアルではglfwGetRequiredInstanceExtensions

        let instance_create_info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(&extension_names);

        unsafe { Ok(entry.create_instance(&instance_create_info, None)?) } //基本的に本家で返り値がVkResultなものはResult型で値が包まれて返ってくるので引数も減る
    }
}

fn main() {
    env_logger::init();
    match VulkanApp::new() {
        Ok(mut app) => app.run(),
        Err(error) => log::error!("Failed to create application. Cause: {}", error),
    }
}