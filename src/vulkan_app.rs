use crate::{debug, khr_util};

use crate::queue_family::QueueFamilyIndices;
use crate::required_names::get_required_device_extensions;
use crate::swap_chain_utils::SwapChainSupportDetails;
use ash::extensions::khr::{Surface, Swapchain};
use ash::vk::{
    DebugUtilsMessengerCreateInfoEXT, DebugUtilsMessengerEXT, PhysicalDevice, Queue, SharingMode,
    SurfaceKHR, SwapchainKHR,
};
use ash::{extensions::ext::DebugUtils, vk, Device, Entry, Instance};
use log::{debug, info};
use std::mem::swap;
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

//メンバをOption<>地獄にしないためにはnewに関する関数をメソッドではなく関連関数にすることで回避する
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
    swap_chain: Swapchain,
    swap_chain_khr: SwapchainKHR,
    swap_chain_images: Vec<vk::Image>,
    swap_chain_image_format: vk::Format,
    swap_chain_extent: vk::Extent2D,
    swap_chain_image_views: Vec<vk::ImageView>,
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

        let (swap_chain, swap_chain_khr, swap_chain_image_format, swap_chain_extent) =
            Self::create_swap_chain(&instance, &device, physical_device, &surface, surface_khr);

        //imageのLifetimeはswapchainに紐づいているので明示的にDestoryする必要はない
        let swap_chain_images = Self::get_swap_chain_image(&swap_chain, swap_chain_khr);

        let swap_chain_image_views =
            Self::create_image_views(&device, &swap_chain_images, swap_chain_image_format);

        Self::create_graphics_pipeline(&device);

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
            swap_chain,
            swap_chain_khr,
            swap_chain_images,
            swap_chain_image_format,
            swap_chain_extent,
            swap_chain_image_views,
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

        info!("surface: {:?}", surface_khr);

        (surface, surface_khr)
    }

    fn create_swap_chain(
        instance: &Instance,
        device: &Device,
        physical_device: PhysicalDevice,
        surface: &Surface,
        surface_khr: SurfaceKHR,
    ) -> (Swapchain, SwapchainKHR, vk::Format, vk::Extent2D) {
        let swap_chain_support =
            SwapChainSupportDetails::new(physical_device, surface, surface_khr);

        let surface_format = swap_chain_support.choose_swap_surface_format();
        let present_mode = swap_chain_support.choose_swap_present_mode();
        let extent = swap_chain_support.choose_swap_extent();

        //swapchainに含められる画像の枚数を決める
        //少なすぎると空き容量がなくてレンダリングが止まってしまう
        let mut image_count = swap_chain_support.capabilities.min_image_count + 1;

        //max_image_countが0の場合は上限が存在しないという意味
        if swap_chain_support.capabilities.max_image_count > 0
            && image_count > swap_chain_support.capabilities.max_image_count
        {
            image_count = swap_chain_support.capabilities.max_image_count;
        }

        let mut create_info = vk::SwapchainCreateInfoKHR::builder()
            .surface(surface_khr)
            .min_image_count(image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(extent)
            //各画像が持つレイヤの数
            //ステレオコピックアプリケーションなどを作成する時に使用
            //スタン時の演出とかにも使える？
            .image_array_layers(1)
            //Swapchain内の画像をどのように扱うかを指定
            //今回は直接レンダリングするのでCOLOR_ATTACHMENTを採用
            //別の場所に画像をレンダリングしてあとからメモリ操作などで送信するTRANSFER_DSTなどもある
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT);

        let indices = QueueFamilyIndices::find_queue_families(
            instance,
            surface,
            surface_khr,
            physical_device,
        );

        //キューファミリーのindexを配列に
        let queue_family_indices = [
            indices.graphics_family.unwrap(),
            indices.present_family.unwrap(),
        ];

        //SwapChainが扱う画像が複数の種類のキューファミリーがまたがって使用するかどうかの設定
        //今回の場合はグラフィックスファミリーとプレゼンテーションファミリーが同一のキューかどうかを調べてそれぞれ設定を確認する
        if indices.graphics_family.unwrap() != indices.present_family.unwrap() {
            //CONCURRENTは画像の所有権の移動なしに複数のキューファミリーをまたがって使用することができる
            create_info = create_info
                .image_sharing_mode(SharingMode::CONCURRENT)
                //CONCURRENTではどのキューファミリー間で所有権を共有するかを事前にしているする必要がある
                .queue_family_indices(&queue_family_indices);
        } else {
            //EXCLUSIVEは１つのキューファミリが所有権を持ち、複数のキューファミリーをまたがって使用する場合は明示的に所有権を移動しなければならない
            //パフォーマンス的には最高
            create_info = create_info.image_sharing_mode(SharingMode::EXCLUSIVE);
        }

        let create_info = create_info
            //swapchain内の画像に対して90度時計回りなどのtransformの変換を指定できる
            //今回の場合は何もしない
            .pre_transform(swap_chain_support.capabilities.current_transform)
            //ほかウィンドウとのブレンドをどうするか指定
            //OPAQUEはアルファを無視
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            //他ウィンドウに隠れたピクセルをクリップするかどうか
            .clipped(true)
            //Vulkanではアプリケーションの実行中にスワップチェンが無効または最適化されなくなる可能性がある
            //その場合はスワップチェーンを0から再度作らなければいけないため、その場合の古いSwapChainの参照を渡す
            //今回はSwapChainは一つしか作らないことを仮定
            .old_swapchain(vk::SwapchainKHR::null())
            .build();

        let swap_chain = Swapchain::new(instance, device);
        let swap_chain_khr = unsafe { swap_chain.create_swapchain(&create_info, None).unwrap() };

        info!("swapchain: {:?}", swap_chain_khr);

        (swap_chain, swap_chain_khr, surface_format.format, extent)
    }

    fn get_swap_chain_image(
        swap_chain: &Swapchain,
        swap_chain_khr: SwapchainKHR,
    ) -> Vec<vk::Image> {
        unsafe { swap_chain.get_swapchain_images(swap_chain_khr) }.unwrap()
    }

    fn create_image_views(
        device: &Device,
        swap_chain_images: &Vec<vk::Image>,
        swap_chain_image_format: vk::Format,
    ) -> Vec<vk::ImageView> {
        let mut swap_chain_image_views = vec![];

        for image in swap_chain_images {
            let create_info = vk::ImageViewCreateInfo::builder()
                .image(*image)
                //画像を1Dテクスチャ、2Dテクスチャ、3Dテクスチャ、キューマップとして扱うことができる
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(swap_chain_image_format)
                .components(
                    //swizzleなマッピングをすることができる
                    //例えばモノクロなテクスチャを全て赤色に割り当てて出力したりなど
                    //今回はすべてデフォルトで行う
                    vk::ComponentMapping::builder()
                        .r(vk::ComponentSwizzle::IDENTITY)
                        .g(vk::ComponentSwizzle::IDENTITY)
                        .b(vk::ComponentSwizzle::IDENTITY)
                        .a(vk::ComponentSwizzle::IDENTITY)
                        .build(),
                )
                .subresource_range(
                    //画像自体の目的が何であるか
                    //画像の土の部分にアクセスすべきかを書くことができる
                    //今回はミップマップレベルやマルチレイヤーは無しで設定
                    vk::ImageSubresourceRange::builder()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .base_mip_level(0)
                        .level_count(0)
                        .base_array_layer(0)
                        .layer_count(1)
                        .build(),
                )
                .build();

            swap_chain_image_views
                .push(unsafe { device.create_image_view(&create_info, None).unwrap() });
        }

        info!("Create SwapChain Image View");

        //テクスチャとして使う分には準備できているが、レンダーターゲットとしてはまだ設定が必要
        //その設定とはフレームバッファと呼ばれるもう一段回のインダイレクトが必要だがこれを用意するのにまずグラフィックスパイプラインを設定する必要がある
        swap_chain_image_views
    }

    fn create_graphics_pipeline(device: &Device) {
        //ここの環境変数はrust-gpu側が設定をしてくれる
        const SHADER_PATH: &str = env!("rust_shader.spv");
        const SHADER_CODE: &[u8] = include_bytes!(env!("rust_shader.spv"));

        info!("Shader Path: {}", SHADER_PATH);
        info!("Shader Length: {}", SHADER_CODE.len());

        let shader_module = Self::create_shader_module(device, SHADER_CODE);

        //graphics pipeline...

        unsafe {
            //パイプラインの作成が終了したらモジュールはすぐに破棄して良い
            device.destroy_shader_module(shader_module, None);
        }
    }

    fn create_shader_module(device: &Device, spirv_code: &[u8]) -> vk::ShaderModule {
        //[u8]から[u32]に変換
        //アライメントはSPIR-Vのコンパイラが保証しているものとする
        let spirv_code = unsafe { std::mem::transmute::<&[u8], &[u32]>(spirv_code) };

        let create_info = vk::ShaderModuleCreateInfo::builder()
            .code(spirv_code)
            .build();

        unsafe { device.create_shader_module(&create_info, None).unwrap() }
    }
}

impl Drop for VulkanApp {
    fn drop(&mut self) {
        log::debug!("Dropping application.");
        unsafe {
            for image_view in self.swap_chain_image_views.clone() {
                self.device.destroy_image_view(image_view, None);
            }

            self.device.destroy_device(None);

            self.surface.destroy_surface(self.surface_khr, None);

            self.swap_chain.destroy_swapchain(self.swap_chain_khr, None);

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
