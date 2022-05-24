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
    pipeline_layout: vk::PipelineLayout,
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

        let pipeline_layout = Self::create_graphics_pipeline(&device, swap_chain_extent);

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
            pipeline_layout,
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

    fn create_graphics_pipeline(
        device: &Device,
        swap_chain_extent: vk::Extent2D,
    ) -> vk::PipelineLayout {
        //プログラマブルステージの設定

        //Create Shader Module

        //ここの環境変数はrust-gpu側が設定をしてくれる
        const SHADER_PATH: &str = env!("rust_shader.spv");
        const SHADER_CODE: &[u8] = include_bytes!(env!("rust_shader.spv"));

        info!("Shader Path: {}", SHADER_PATH);
        info!("Shader Length: {}", SHADER_CODE.len());

        let shader_module = Self::create_shader_module(device, SHADER_CODE);

        let vert_shader_stage_info = vk::PipelineShaderStageCreateInfo::builder()
            //fragmentやvertexまたgeometryなどのどこのシェーダーステージの物なのかを指定する
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(shader_module)
            .name(CString::new("main_vs").unwrap().as_c_str())
            //これはシェーダ内で定数を設定する時に外部から設定できるのでそのときに使用するもの
            //.specialization_info()
            .build();

        let frag_shader_stage_info = vk::PipelineShaderStageCreateInfo::builder()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(shader_module)
            .name(CString::new("main_fs").unwrap().as_c_str())
            .build();

        let shader_stages = [vert_shader_stage_info, frag_shader_stage_info];

        //Vertex Input

        //頂点シェーダーに渡される頂点データの形式を指定
        //今回は三角形の頂点データがシェーダーにハードコードされているので何も設定しなくて良い
        let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::builder()
            //バインディングとはデータ感の間隔やデータが頂点ごとかインスタンスごとかの指定など
            //.vertex_binding_descriptions()
            //頂点シェーダーに渡される属性の指定またどのバインディングからロードするかやどのオフセットでロードするかなど
            //.vertex_attribute_description_count()
            .build();

        //固定機能ステージの設定

        //Input Assembly
        //入力された頂点からどのようなトポロジでプリミティブを作成するかを設定

        let input_assembly_info = vk::PipelineInputAssemblyStateCreateInfo::builder()
            //トポロジの設定
            //今回は3つずつ頂点を読み込んで描画
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            //トポロジの設定でSTRIP系の設定をしていると全てのプリミティブがつながってしまうので
            //trueにすることでそのつながり部分を一度断ち切るようなindex値を設定できる
            .primitive_restart_enable(false)
            .build();

        //Viewport

        let viewport = vk::Viewport::builder()
            //出力がレンダリングするフレームバッファの領域を指定
            //x, yはスタート位置
            .x(0.0)
            .y(0.0)
            //縦横のサイズ
            .width(swap_chain_extent.width as _)
            .height(swap_chain_extent.height as _)
            .min_depth(0.0)
            .max_depth(1.0)
            .build();

        //Scissor Rectangle

        //Viewportはレンダリングされた画像をフレームバッファに対してどの位置に描画をするのか設定するものに対して
        //Scissor Rectangleはレンダリングされた画像のどのピクセルを使用するかを指定
        //https://vulkan-tutorial.com/images/viewports_scissors.png
        let scissor = vk::Rect2D::builder()
            .offset(vk::Offset2D::builder().x(0).y(0).build())
            .extent(swap_chain_extent)
            .build();

        //viewportとscissor rectangleを統合
        let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
            //GPUによっては複数のviewportとscissor rectangleを使用することができる
            .viewports(&[viewport])
            .scissors(&[scissor])
            .build();

        let rasterizer = vk::PipelineRasterizationStateCreateInfo::builder()
            //trueを設定した場合nearとfarを超えたフラグメントはカリングされるのではなくclampされる
            //シャドウマップなどに有効
            //GPUの機能を有効にする必要あり
            .depth_clamp_enable(false)
            //trueを設定した場合ラスタライザステージをスキップする
            .rasterizer_discard_enable(false)
            //フラグメントの生成方法
            //input assemblyは実際に塗るかどうかの設定だが、これはフラグメントを作成するかどうかの判断(?)
            //例えばFILLの場合はポリゴンの領域をフラグメントで埋める
            //GPUの機能を有効にする必要あり
            .polygon_mode(vk::PolygonMode::FILL)
            //線の太さを設定
            //最大値はGPUに依存する
            //1.0以上を指定したい場合はwideLinesというGPUの機能を有効にする必要あり
            .line_width(1.0)
            //カリングの種類を指定
            .cull_mode(vk::CullModeFlags::BACK)
            //Vulkanは右回りが表面？
            .front_face(vk::FrontFace::CLOCKWISE)
            //深度値の設定
            //フラグメントの偏りに基づいてバイアスを掛けたりして深度地を変更することができる
            //これらはシャドウマッピングなどで使用される
            .depth_bias_enable(false)
            .depth_bias_constant_factor(0.0)
            .depth_bias_clamp(0.0)
            .depth_bias_slope_factor(0.0)
            .build();

        //Multisampling

        //マルチサンプリングはアンチエイリアスの方法の１つ
        //GPUの機能を有効にする必要がある
        let multisampling = vk::PipelineMultisampleStateCreateInfo::builder()
            //今は無効化
            .sample_shading_enable(false)
            .rasterization_samples(vk::SampleCountFlags::TYPE_1)
            .min_sample_shading(1.0)
            .alpha_to_coverage_enable(false)
            .alpha_to_one_enable(false)
            .build();

        //Depth Stencil
        //今はスキップ

        //Color blending

        //フレームバッファごとの設定
        //現在はフレームバッファは１つしか存在しない
        let color_blend_attachment = vk::PipelineColorBlendAttachmentState::builder()
            .color_write_mask(
                vk::ColorComponentFlags::R
                    | vk::ColorComponentFlags::G
                    | vk::ColorComponentFlags::B
                    | vk::ColorComponentFlags::A,
            )
            //新しい色と古い色を混ぜるかどうか
            //falseの場合はフラグメントシェーダーからの新しい色をそのまま使用する
            .blend_enable(false)
            //新しく来た色の寄与の割合(src_color_blend_factor * new_color的な感じ)
            .src_color_blend_factor(vk::BlendFactor::ONE)
            //もとから存在した色の寄与の割合(dst_color_blend_factor * old_color的な感じ)
            .dst_color_blend_factor(vk::BlendFactor::ZERO)
            //色を混ぜるときの演算子
            .color_blend_op(vk::BlendOp::ADD)
            //上記のalpha版
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
            .alpha_blend_op(vk::BlendOp::ADD)
            .build();

        //全てのフレームバッファ構造体の設定
        let color_blend = vk::PipelineColorBlendStateCreateInfo::builder()
            //2つ目のブレンド方法
            //ビット単位でのブレンドの演算を行うことができる
            //これを有効にするとVkPipelineColorBlendAttachmentStateで有効にしたblend設定は無効になってしまうので注意
            //vkPipelineColorBlendAttachmentStateで設定したcolor_write_maskは個々でも使用される
            .logic_op_enable(false)
            //ビット演算の演算子指定
            .logic_op(vk::LogicOp::COPY)
            .attachments(&[color_blend_attachment])
            .blend_constants([0.0, 0.0, 0.0, 0.0])
            .build();

        //Dynamic State

        //一度パイプラインの作成をしたあとに再作成をなしに変更できる値を設定
        //ここではビューポートのサイズと線の幅
        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::LINE_WIDTH];

        //dynamic_stateは今後の章で扱うので今回は作るだけ作っておいてnullを入れておく
        let dynamic_state = vk::PipelineDynamicStateCreateInfo::builder()
            .dynamic_states(&dynamic_states)
            .build();

        //Pipeline layout

        //この構造体はVertex Shaderに変換行列を渡したり、フラグメントシェーダーでテクスチャサンプラーを作成するために使用する
        //これによってシェーダーを一回一回ビルドしなくても定数を外部から変えることで柔軟性を持たせることができる
        let pipeline_layout_info = vk::PipelineLayoutCreateInfo::builder()
            //.set_layouts()
            //.push_constant_ranges()
            .build();

        let pipeline_layout = unsafe {
            device
                .create_pipeline_layout(&pipeline_layout_info, None)
                .unwrap()
        };

        unsafe {
            //パイプラインの作成が終了したらモジュールはすぐに破棄して良い
            device.destroy_shader_module(shader_module, None);
        }

        pipeline_layout
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

            self.device
                .destroy_pipeline_layout(self.pipeline_layout, None);

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
