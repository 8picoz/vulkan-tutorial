use crate::{debug, khr_util};

use crate::queue_family::QueueFamilyIndices;
use ash::vk::{DebugUtilsMessengerCreateInfoEXT, DebugUtilsMessengerEXT, PhysicalDevice, Queue};
use ash::{extensions::ext::DebugUtils, vk, Entry, Instance};
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
pub const REQUIRED_LAYERS: [&str; 1] = ["VK_LAYER_KHRONOS_validation"];

pub struct VulkanApp {
    _entry: Entry,
    instance: Instance,
    //物理デバイス
    //こいつは保持する必要ないかも
    physical_device: PhysicalDevice,
    debug_utils: Option<DebugUtils>,
    debug_utils_messenger_ext: Option<DebugUtilsMessengerEXT>,
    //倫理デバイス
    device: ash::Device,
    queue: Queue,
}

impl VulkanApp {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        log::debug!("Creating application");

        let entry = unsafe { Entry::load().expect("Failed to create entry.") };
        let instance = Self::create_instance(&entry)?;

        let mut debug_utils = None;
        let mut debug_utils_messenger_ext = None;

        if ENABLE_VALIDATION_LAYERS {
            let _debug_utils = DebugUtils::new(&entry, &instance);

            debug_utils_messenger_ext = Some(
                debug::setup_debug_utils_messenger_ext(&_debug_utils)
                    .unwrap_or_else(|e| panic!("{}", e)),
            );

            debug_utils = Some(_debug_utils);
        }

        let physical_device = Self::pick_physical_device(&instance);

        let (device, queue) = Self::pick_device_and_queue(&instance, physical_device);

        Ok(Self {
            _entry: entry,
            instance,
            physical_device,
            debug_utils,
            debug_utils_messenger_ext,
            device,
            queue,
        })
    }

    pub fn run(&mut self) {
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
            debug::check_validation_layer_support(entry);

            let debug_create_info = debug::populate_debug_messenger_create_info();

            //enabled_layer_countのセットはenabled_layer_namesの中に入っている
            instance_create_info = instance_create_info.enabled_layer_names(&layer_names_ptrs);
            //勉強のために型の変換の遷移を書いているが as *const _ as _;でも可
            instance_create_info.p_next =
                &debug_create_info as *const DebugUtilsMessengerCreateInfoEXT as *const c_void;
        }

        unsafe { Ok(entry.create_instance(&instance_create_info, None)?) } //基本的に本家で返り値がVkResultなものはResult型で値が包まれて返ってくるので引数も減る
    }

    fn pick_physical_device(instance: &Instance) -> PhysicalDevice {
        let physical_devices = unsafe {
            instance
                .enumerate_physical_devices()
                .expect("物理デバイスが取得できませんでした")
        };

        let physical_device = physical_devices
            .into_iter()
            .find(|devices| QueueFamilyIndices::is_device_suitable(instance, *devices))
            .expect("最適なPhysical Deviceが存在しません");

        let props = unsafe { instance.get_physical_device_properties(physical_device) };

        log::info!("Selected physical device: {:?}", unsafe {
            CStr::from_ptr(props.device_name.as_ptr())
        });

        physical_device
    }

    //論理デバイスを取得
    fn pick_device_and_queue(
        instance: &Instance,
        physical_device: PhysicalDevice,
    ) -> (ash::Device, Queue) {
        let indices = QueueFamilyIndices::find_queue_families(instance, physical_device);

        //倫理デバイスが対応しているキューを取得する
        let queue_create_info = [vk::DeviceQueueCreateInfo::builder()
            .queue_family_index(indices.graphics_family.expect("値が存在しません"))
            .queue_priorities(&[1.0f32])
            .build()];

        //queue_family.rsで検索したgeometry shaderのような機能を使用できるかどうかを検索する時に使用する
        let device_features = vk::PhysicalDeviceFeatures::builder().build();

        let mut create_info = vk::DeviceCreateInfo::builder()
            .queue_create_infos(&queue_create_info)
            .enabled_features(&device_features);

        let layer_names = REQUIRED_LAYERS
            .iter()
            .map(|name| CString::new(*name).expect("Failed to build CString"))
            .collect::<Vec<_>>();
        let layer_names_ptrs = layer_names
            .iter()
            .map(|name| name.as_ptr())
            .collect::<Vec<_>>();

        if ENABLE_VALIDATION_LAYERS {
            create_info = create_info.enabled_layer_names(&layer_names_ptrs);
        }

        //存在しなかったりサポートされていない機能を有効にしようとするとエラーが出る
        let device =
            unsafe { instance.create_device(physical_device, &create_info, None) }.unwrap();

        //論理デバイスからキューを作成、
        //引数は必要なキューのキューファミリーの番号とキューインデックス
        //キューインデックスは複数存在するキューのインデックス
        let queue = unsafe { device.get_device_queue(indices.graphics_family.unwrap(), 0) };

        (device, queue)
    }
}

impl Drop for VulkanApp {
    fn drop(&mut self) {
        log::debug!("Dropping application.");
        unsafe {
            self.device.destroy_device(None);

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
