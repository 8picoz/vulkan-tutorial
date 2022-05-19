use crate::{debug, khr_util};

use crate::queue_family::QueueFamilyIndices;
use crate::required_names::get_required_device_extensions;
use ash::extensions::khr::Surface;
use ash::vk::{
    DebugUtilsMessengerCreateInfoEXT, DebugUtilsMessengerEXT, PhysicalDevice, Queue, SurfaceKHR,
};
use ash::{extensions::ext::DebugUtils, vk, Entry, Instance};
use log::{debug, info};
use std::{
    error::Error,
    ffi::{c_void, CStr, CString},
    result::Result,
};
use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::Window;

#[cfg(debug_assertions)]
const ENABLE_VALIDATION_LAYERS: bool = true;

#[cfg(not(debug_assertions))]
const ENABLE_VALIDATION_LAYERS: bool = false;

///Validation Layerで必要な機能一覧
///今のAshだともっと良いやり方がある、Swapchainのやり方はその一例
pub const REQUIRED_LAYERS: [&str; 1] = ["VK_LAYER_KHRONOS_validation"];

pub struct VulkanApp {
    entry: Entry,
    instance: Instance,
    debug_utils: Option<DebugUtils>,
    debug_utils_messenger_ext: Option<DebugUtilsMessengerEXT>,
    //倫理デバイス
    device: ash::Device,
    graphics_queue: Queue,
    present_queue: Queue,
    //SurfaceKHRはハンドラ本体でSurfaceはラッパー？
    surface: Surface,
    surface_khr: SurfaceKHR,
}

impl VulkanApp {
    pub fn new(window: &Window) -> Result<Self, Box<dyn Error>> {
        debug!("Creating application");

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

        let (surface, surface_khr) = Self::create_surface(&instance, &entry, window);

        let physical_device = Self::pick_physical_device(&instance, &surface, surface_khr);

        let (device, graphics_queue, present_queue) = Self::create_logical_device_and_queue(
            &instance,
            &surface,
            surface_khr,
            physical_device,
        );

        Ok(Self {
            entry,
            instance,
            debug_utils,
            debug_utils_messenger_ext,
            device,
            graphics_queue,
            present_queue,
            surface,
            surface_khr,
        })
    }

    pub fn run(&mut self, event_loop: EventLoop<()>) {
        info!("Running application");

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(VirtualKeyCode::Space),
                                state: ElementState::Released,
                                ..
                            },
                        ..
                    } => {
                        info!("Space!");
                    }
                    _ => (),
                },
                _ => (),
            }
        });
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

    fn pick_physical_device(
        instance: &Instance,
        surface: &Surface,
        surface_khr: SurfaceKHR,
    ) -> PhysicalDevice {
        let physical_devices = unsafe {
            instance
                .enumerate_physical_devices()
                .expect("物理デバイスが取得できませんでした")
        };

        let physical_device = physical_devices
            .into_iter()
            .find(|physical_device| {
                QueueFamilyIndices::is_device_suitable(
                    instance,
                    surface,
                    surface_khr,
                    *physical_device,
                )
            })
            .expect("最適なPhysical Deviceが存在しません");

        let props = unsafe { instance.get_physical_device_properties(physical_device) };

        info!("Selected physical device: {:?}", unsafe {
            CStr::from_ptr(props.device_name.as_ptr())
        });

        physical_device
    }

    //論理デバイスを取得
    fn create_logical_device_and_queue(
        instance: &Instance,
        surface: &Surface,
        surface_khr: SurfaceKHR,
        physical_device: PhysicalDevice,
    ) -> (ash::Device, Queue, Queue) {
        let indices = QueueFamilyIndices::find_queue_families(
            instance,
            surface,
            surface_khr,
            physical_device,
        );

        //倫理デバイスが対応しているキューを取得する
        let queue_create_info = [vk::DeviceQueueCreateInfo::builder()
            .queue_family_index(indices.graphics_family.expect("値が存在しません"))
            .queue_priorities(&[1.0f32])
            .build()];

        //queue_family.rsで検索したgeometry shaderのような機能を使用できるかどうかを検索する時に使用する
        let device_features = vk::PhysicalDeviceFeatures::builder().build();

        let extension_names_ptr = get_required_device_extensions()
            .iter()
            .map(|name| name.as_ptr())
            .collect::<Vec<_>>();

        let mut create_info = vk::DeviceCreateInfo::builder()
            .queue_create_infos(&queue_create_info)
            .enabled_extension_names(&extension_names_ptr)
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

        //グラフィックスファミリーキューインデックス
        let graphics_queue =
            unsafe { device.get_device_queue(indices.graphics_family.unwrap(), 0) };

        //
        let present_queue = unsafe { device.get_device_queue(indices.present_family.unwrap(), 0) };

        (device, graphics_queue, present_queue)
    }

    fn create_surface(
        instance: &Instance,
        entry: &Entry,
        window: &Window,
    ) -> (Surface, SurfaceKHR) {
        let surface = Surface::new(entry, instance);
        let surface_khr =
            unsafe { ash_window::create_surface(entry, instance, window, None).unwrap() };

        info!("{:?}", surface_khr);

        (surface, surface_khr)
    }
}

impl Drop for VulkanApp {
    fn drop(&mut self) {
        log::debug!("Dropping application.");
        unsafe {
            self.device.destroy_device(None);

            self.surface.destroy_surface(self.surface_khr, None);

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
