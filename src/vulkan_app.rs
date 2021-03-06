use crate::queue_family::QueueFamilyIndices;
use crate::required_names::get_required_device_extensions;
use crate::swap_chain_utils::SwapChainSupportDetails;
use crate::{debug, khr_util, WindowHandlers};
use ash::extensions::khr::{Surface, Swapchain};
use ash::vk::{
    CommandPool, DebugUtilsMessengerCreateInfoEXT, DebugUtilsMessengerEXT, Format, PhysicalDevice,
    Pipeline, Queue, SharingMode, SurfaceKHR, SwapchainKHR,
};
use ash::{extensions::ext::DebugUtils, vk, Device, Entry, Instance};
use log::{debug, info};
use std::{
    error::Error,
    ffi::{c_void, CStr, CString},
    result::Result,
};
use winit::dpi::LogicalSize;
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

//同時にレンダリングできるフレーム数を指定
//2という数字を選んだのはCPUがGPUに対して選考しすぎないようにするため
//ここらへんの設定やFenceなどが垂直同期に対して関わってくるのだと思う
pub const MAX_FRAMES_IN_FLIGHT: u32 = 2;

//初期サイズ
const WIDTH: u32 = 800;
const HEIGHT: u32 = 800;

//メンバをOption<>地獄にしないためにはnewに関する関数をメソッドではなく関連関数にすることで回避する
pub struct VulkanApp {
    entry: Entry,
    instance: Instance,
    debug_utils: Option<DebugUtils>,
    debug_utils_messenger_ext: Option<DebugUtilsMessengerEXT>,
    //物理デバイス
    physical_device: PhysicalDevice,
    //倫理デバイス
    device: Device,
    graphics_queue: Queue,
    present_queue: Queue,
    //SurfaceKHRはハンドラ本体でSurfaceはラッパー？
    surface: Surface,
    surface_khr: SurfaceKHR,
    swap_chain: Swapchain,
    swap_chain_khr: SwapchainKHR,
    swap_chain_images: Vec<vk::Image>,
    swap_chain_image_format: Format,
    swap_chain_extent: vk::Extent2D,
    swap_chain_image_views: Vec<vk::ImageView>,
    render_pass: vk::RenderPass,
    pipeline_layout: vk::PipelineLayout,
    pipeline: Pipeline,
    swap_chain_frame_buffers: Vec<vk::Framebuffer>,
    command_pool: CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,
    current_frame: usize,
    resize: Option<(u32, u32)>,

    //これ移行がVecになっているのは複数のフレームを同時にレンダリングするときに複数必要になるため
    //swapchainからimageを取得してレンダリングの準備ができたことを知らせるSemaphore
    image_available_semaphores: Vec<vk::Semaphore>,
    //レンダリングが終了してPresentationの準備ができたことを知らせるSemaphore
    render_finished_semaphores: Vec<vk::Semaphore>,
    //一度に1フレームしかレンダリングしないようにCPU側で止めるためのFence
    in_flight_fences: Vec<vk::Fence>,
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
            Self::create_swap_chain(
                &instance,
                &device,
                physical_device,
                &surface,
                surface_khr,
                (WIDTH, HEIGHT),
            );

        //imageのLifetimeはswapchainに紐づいているので明示的にDestoryする必要はない
        let swap_chain_images = Self::get_swap_chain_images(&swap_chain, swap_chain_khr);

        let swap_chain_image_views =
            Self::create_image_views(&device, &swap_chain_images, swap_chain_image_format);

        let render_pass = Self::create_render_pass(&device, swap_chain_image_format);

        let (pipeline, pipeline_layout) =
            Self::create_graphics_pipeline(&device, swap_chain_extent, render_pass);

        let swap_chain_frame_buffers = Self::create_frame_buffers(
            &device,
            render_pass,
            //Cloneして大丈夫？
            swap_chain_image_views.clone(),
            swap_chain_extent,
        );

        let command_pool =
            Self::create_command_pool(&instance, &surface, surface_khr, physical_device, &device);

        let command_buffers =
            Self::create_command_buffers(&device, command_pool, MAX_FRAMES_IN_FLIGHT);

        let (image_available_semaphores, render_finished_semaphores, in_flight_fences) =
            Self::create_sync_objects(&device, MAX_FRAMES_IN_FLIGHT);

        Ok(Self {
            entry,
            instance,
            debug_utils,
            debug_utils_messenger_ext,
            physical_device,
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
            render_pass,
            pipeline_layout,
            pipeline,
            swap_chain_frame_buffers,
            command_pool,
            command_buffers,
            current_frame: 0,
            resize: None,
            image_available_semaphores,
            render_finished_semaphores,
            in_flight_fences,
        })
    }

    fn draw_frame(&mut self, frame_size: usize) {
        //フレームに対して書き込むために使用するCommandBufferやSemaphoreやFenceを取得する
        let command_buffer = *self.command_buffers.get(self.current_frame).unwrap();
        let image_available_semaphore = *self
            .image_available_semaphores
            .get(self.current_frame)
            .unwrap();
        let render_finished_semaphore = *self
            .render_finished_semaphores
            .get(self.current_frame)
            .unwrap();
        let in_flight_fence = *self.in_flight_fences.get(self.current_frame).unwrap();

        unsafe {
            //Fenceの待機
            //第二引数は配列で受け取った全てのFenceを待つかどうか
            self.device
                .wait_for_fences(&[in_flight_fence], true, u64::MAX)
                .unwrap();

            //swapchainからImageを取得する
            //.0はswap_chain_imagesの配列のIndexが帰ってくる
            //.1はVK_SUBOPTIMAL_KHRかどうかが帰ってくる
            //https://www.khronos.org/registry/vulkan/specs/1.3-extensions/man/html/VkResult.html
            let result = self.swap_chain.acquire_next_image(
                self.swap_chain_khr,
                //画像が利用可能になるまでの待機時間のタイムアウトをナノ秒で指定
                //MAXを入れるとタイムアウトを無効にできる
                u64::MAX,
                //このセマフォはシグナルが送られる
                image_available_semaphore,
                vk::Fence::null(),
            );

            let image_index = match result {
                Ok((image_index, _)) => image_index,
                //ERROR_OUT_OF_DATE_KHR
                //swapchainとsurfaceの互換がなくなった時に呼ばれる、ウィンドウのリサイズ時など
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    self.recreate_swap_chain();
                    return;
                }
                Err(error) => {
                    panic!("{}", error);
                }
            };

            //リセットをこの位置に置くことでrecreate_swap_chainのタイミングでreturnすることによるデッドロックを回避することが出来る
            //リセットしてるのにsignalを送る人がいないという状況を回避する
            self.device.reset_fences(&[in_flight_fence]).unwrap();

            //コマンドバッファをリセットする
            self.device
                .reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty())
                .unwrap();

            //コマンドバッファを記録する
            self.record_command_buffer(image_index as usize);

            //キューをGPUにSubmitする
            let submit_info = vk::SubmitInfo::builder()
                //どのセマフォを使用して待機するか
                .wait_semaphores(&[image_available_semaphore])
                //どのステージで待機するかを指定
                //今回は画像が利用可能になるまで待ちたいのでCOLOR_ATTACHMENT_OUTPUTを使用
                //この配列はインデックスで上記のsemaphoreの配列と対応する
                //ここのセマフォを設定せずに行うと理論的には画像が利用可能でない状態でバーテックスシェーダを使用することなどが可能
                .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
                //実行するコマンドバッファを指定
                .command_buffers(&[command_buffer])
                //ここで指定したセマフォに対してこのsubmitが終了した時にシグナルを送る
                .signal_semaphores(&[render_finished_semaphore])
                .build();

            //graphics_queueをsubmitする
            //in_flight_fenceに対してシグナルを送るように
            self.device
                //queueへのsubmitは非常に処理として重たいので複数のsubmit_infoを一回で渡せるようになっている
                .queue_submit(self.graphics_queue, &[submit_info], in_flight_fence)
                .unwrap();

            //Presentation

            let present_info = vk::PresentInfoKHR::builder()
                //待機するセマフォを指定
                .wait_semaphores(&[render_finished_semaphore])
                .swapchains(&[self.swap_chain_khr])
                //swapchainに対するimageを指定
                .image_indices(&[image_index])
                //このメソッドはPresentationが成功したかどうかを受け取れる
                //引数が配列になっているのは各swapchainに対してそれぞれResultが返ってくるため
                //今回はswapchainが１つしか存在しないのでpresent用の関数の戻り値を参照すれば良い
                //swapchainが複数存在するとき用？
                //.results()
                .build();

            let result = self
                .swap_chain
                .queue_present(self.present_queue, &present_info);

            match result {
                Ok(is_suboptimal) if is_suboptimal => {
                    self.recreate_swap_chain();
                }
                Ok(_) => {}
                //SUBOPTIMAL_KHR
                //swapchainはsurfaceに正常にpresentすることは出来るが、プロパティは完全に一致していない
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    self.recreate_swap_chain();
                }
                Err(error) => {
                    panic!("{}", error);
                }
            }

            //二回recreate_swap_chainが呼ばれることになりそう
            if self.resize.is_some() {
                self.recreate_swap_chain();
            }
        }

        self.current_frame = (self.current_frame + 1) % frame_size;
    }

    pub fn run(mut self, window_handlers: WindowHandlers) {
        info!("Running application");

        window_handlers
            .event_loop
            .run(move |event, _, control_flow| {
                *control_flow = ControlFlow::Poll;
                self.resize = None;

                if let Event::WindowEvent { event, .. } = event {
                    match event {
                        WindowEvent::CloseRequested => {
                            unsafe { self.device.device_wait_idle().unwrap() };
                            *control_flow = ControlFlow::Exit;
                        }
                        WindowEvent::Resized(physical_size) => {
                            self.resize = Some((physical_size.width, physical_size.height));
                        }
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
                    }
                }

                self.draw_frame(MAX_FRAMES_IN_FLIGHT as usize);
            });
    }

    pub fn recreate_swap_chain(&mut self) {
        //最小化対応
        //最小化時にここで待機させることによって対応させる
        //今の構成だと出来ない気もする
        // if let Some(size) = self.resize {
        //     while size.0 == 0 || size.1 == 0 {
        //         self.run();
        //     }
        // }

        //swapchainが使用されている時に触るのは良くないのでdeviceがidle状態になるのを待つ
        unsafe { self.device.device_wait_idle().unwrap() };

        self.cleanup_swap_chain();

        let (width, height) = self
            .resize
            .unwrap_or((self.swap_chain_extent.width, self.swap_chain_extent.height));

        info!("width: {}, height: {}", width, height);

        let (swap_chain, swap_chain_khr, swap_chain_image_format, swap_chain_extent) =
            Self::create_swap_chain(
                &self.instance,
                &self.device,
                self.physical_device,
                &self.surface,
                self.surface_khr,
                (width, height),
            );

        self.swap_chain = swap_chain;
        self.swap_chain_khr = swap_chain_khr;
        self.swap_chain_image_format = swap_chain_image_format;
        self.swap_chain_extent = swap_chain_extent;

        self.swap_chain_images = Self::get_swap_chain_images(&self.swap_chain, self.swap_chain_khr);

        //image_viewはswapchainに紐づいているので再作成しなければいけない
        self.swap_chain_image_views = Self::create_image_views(
            &self.device,
            &self.swap_chain_images,
            self.swap_chain_image_format,
        );

        //swapchain imageのformatに依存するため再作成
        self.render_pass = Self::create_render_pass(&self.device, self.swap_chain_image_format);

        //viewportとscissor rectがpipelineの作成時に指定されるので再作成
        //ただし再作成をしなくてもdynamic stateを使用すれば良い
        let (pipeline, pipeline_layout) =
            Self::create_graphics_pipeline(&self.device, self.swap_chain_extent, self.render_pass);

        self.pipeline = pipeline;
        self.pipeline_layout = pipeline_layout;

        //swapchainに依存するので再作成
        self.swap_chain_frame_buffers = Self::create_frame_buffers(
            &self.device,
            self.render_pass,
            self.swap_chain_image_views.clone(),
            self.swap_chain_extent,
        );
    }

    //swapchainをcleanupする
    fn cleanup_swap_chain(&mut self) {
        unsafe {
            for framebuffer in self.swap_chain_frame_buffers.clone() {
                self.device.destroy_framebuffer(framebuffer, None);
            }

            self.device.destroy_pipeline(self.pipeline, None);
            self.device
                .destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.destroy_render_pass(self.render_pass, None);

            for image_view in self.swap_chain_image_views.clone() {
                self.device.destroy_image_view(image_view, None);
            }

            self.swap_chain.destroy_swapchain(self.swap_chain_khr, None);
        }
    }

    fn create_instance(entry: &Entry) -> Result<Instance, Box<dyn Error>> {
        let app_info = vk::ApplicationInfo::builder()
            .application_name(CString::new("vulkan app")?.as_c_str())
            .application_version(0)
            .engine_name(CString::new("No Engine")?.as_c_str()) //エンジン名を入力するとそれが既知なエンジンだったらそれように最適化をする
            .engine_version(0)
            .api_version(vk::make_api_version(0, 1, 3, 0)) //Vulkan自体のバージョン
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
        window_size: (u32, u32),
    ) -> (Swapchain, SwapchainKHR, vk::Format, vk::Extent2D) {
        let swap_chain_support =
            SwapChainSupportDetails::new(physical_device, surface, surface_khr);

        let surface_format = swap_chain_support.choose_swap_surface_format();
        let present_mode = swap_chain_support.choose_swap_present_mode();
        let extent = swap_chain_support.choose_swap_extent(window_size.0, window_size.1);

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

    fn get_swap_chain_images(
        swap_chain: &Swapchain,
        swap_chain_khr: SwapchainKHR,
    ) -> Vec<vk::Image> {
        //swapchainで保持している画像のハンドルを取得する
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
                        .level_count(1)
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
        render_pass: vk::RenderPass,
    ) -> (vk::Pipeline, vk::PipelineLayout) {
        //プログラマブルステージの設定

        //Create Shader Module

        //ここの環境変数はrust-gpu側が設定をしてくれる
        const SHADER_PATH: &str = env!("rust_shader.spv");
        const SHADER_CODE: &[u8] = include_bytes!(env!("rust_shader.spv"));

        info!("Shader Path: {}", SHADER_PATH);
        info!("Shader Length: {}", SHADER_CODE.len());

        let shader_module = Self::create_shader_module(device, SHADER_CODE);

        //Lifetimeを確保するために一度変数にしている
        let main_vs = CString::new("main_vs").unwrap();
        let main_fs = CString::new("main_fs").unwrap();

        let vert_shader_stage_info = vk::PipelineShaderStageCreateInfo::builder()
            //fragmentやvertexまたgeometryなどのどこのシェーダーステージの物なのかを指定する
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(shader_module)
            .name(main_vs.as_c_str())
            //これはシェーダ内で定数を設定する時に外部から設定できるのでそのときに使用するもの
            //.specialization_info()
            .build();

        let frag_shader_stage_info = vk::PipelineShaderStageCreateInfo::builder()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(shader_module)
            .name(main_fs.as_c_str())
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
        //あとあと使う？
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

        //dynamic_stateは今後の章で扱うので今回は作るだけ作っておいて実際に設定する部分にはnullを入れておく
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

        //Pipeline

        let pipeline_info = vk::GraphicsPipelineCreateInfo::builder()
            .stages(&shader_stages)
            .vertex_input_state(&vertex_input_info)
            .input_assembly_state(&input_assembly_info)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterizer)
            .multisample_state(&multisampling)
            //.depth_stencil_state()
            .color_blend_state(&color_blend)
            //.dynamic_state()
            .layout(pipeline_layout)
            .render_pass(render_pass)
            .subpass(0)
            //パイプラインの派生をする時に使用する
            //パイプラインの派生とは既存のパイプラインと多くの機能が共通している場合に設定にコストをかけずに素早く切り替えることができる機能
            //Handleで既存のパイプラインを指定するか
            .base_pipeline_handle(vk::Pipeline::null())
            //パイプラインのIndexで指定するかのどちらか
            .base_pipeline_index(-1)
            .build();

        let pipeline = unsafe {
            device
                //第一引数のPipelineCacheはcreate_graphics_pipelinesを複数回呼び出しするときやキャッシュがファイルに保存されている時にパイプラインに関するデータを再利用することができる
                //第二引数は一気にpipelineを作成できるようにするために引数は配列を受け取れるようになっている
                .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
                .unwrap()
                //ここのpopは帰ってくる配列の要素が１つであることがわかっているため
                .pop()
                .unwrap()
        };

        unsafe {
            //パイプラインの作成が終了したらモジュールはすぐに破棄して良い
            device.destroy_shader_module(shader_module, None);
        }

        (pipeline, pipeline_layout)
    }

    fn create_shader_module(device: &Device, spirv_code: &[u8]) -> vk::ShaderModule {
        info!("create shader module");

        let create_info = vk::ShaderModuleCreateInfo {
            s_type: vk::StructureType::SHADER_MODULE_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: vk::ShaderModuleCreateFlags::empty(),
            code_size: spirv_code.len(),
            p_code: spirv_code.as_ptr() as *const u32,
        };

        unsafe { device.create_shader_module(&create_info, None).unwrap() }
    }

    fn create_render_pass(device: &Device, format: Format) -> vk::RenderPass {
        info!("create render pass");

        //Subpass周り諸々

        //subpass同士でやり取りするデータをAttachmentと呼ぶ
        let color_attachment = vk::AttachmentDescription::builder()
            //swapchainのフォーマットと同じものを使用
            .format(format)
            //マルチサンプリングの設定
            .samples(vk::SampleCountFlags::TYPE_1)
            //loadOpとstoreOpはレンダリング前と後のデータをどうするか決める
            //load
            //CLEARは開始時に定数で値をクリアする
            .load_op(vk::AttachmentLoadOp::CLEAR)
            //レンダリングされたコンテンツをメモリ上に保存する
            .store_op(vk::AttachmentStoreOp::STORE)
            //上記２つのStencil版
            //現在は使用していないので特に考慮する必要がないというDONT_CAREを割り当てる
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            //画像の特定のピクセルフォーマットはVkImageで保持されるがそのピクセルごとのメモリレイアウトの設定はここで行われる
            //レイアウトはそれぞれその画像が何をするための物なのかを示すもの
            //initialLayoutはレンダリングパスが始まる前に画像が持つレイアウトを指定する
            //UNDEFINEDは画像のレイアウト
            .initial_layout(vk::ImageLayout::UNDEFINED)
            //PRESENT_SRC_KHRはスワップチェーンで提示される画像となる
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)
            .build();

        //Subpass用の設定構造体
        let color_attachment_ref = vk::AttachmentReference::builder()
            //Subpassは複数のAttachmentを持つことがあるためこうなっている
            //参照するVkAttachmentDescriptionのインデックスを指定する
            .attachment(0)
            //attachmentのレイアウトを指定
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .build();

        let subpass = vk::SubpassDescription::builder()
            //Vulkanは将来的にCompute系のsubpassもサポートする可能性が存在するためGRAPHICSを指定してあげる
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            //ここでindexを0番に設定したためフラグメントシェーダーから`layout(location = 0) out vec4 outColor`で参照できる
            .color_attachments(&[color_attachment_ref])
            .build();

        //Render passのSubpass Dependencyはdraw_frameのImageが利用可能にならないと(セマフォでいうとimage_available_semaphore)設定できないので待機する
        //今回の方法はRender passを途中でVK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BITまで待機させることで可能にしているが
        //imageAvailableSemaphoreのwaitStagesをVK_PIPELINE_STAGE_TOP_OF_PIPE_BITに変更してRender pass自体を開始しないようにすることもできる

        //srcとdstの２つのsubpassを指定して紐づける
        let dependency = vk::SubpassDependency::builder()
            //subpassの依存関係を記述
            //SUBPAS_EXTERNALはdst_subpassがどう指定されているかに応じてレンダーパスの前後の暗黙のsubpassを参照する
            .src_subpass(vk::SUBPASS_EXTERNAL)
            //subpassのindexを指定
            .dst_subpass(0)
            //次の２つは待機する操作とその操作が発生するステージを指定
            //ステージ指定
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            //待機操作
            .src_access_mask(vk::AccessFlags::empty())
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .build();

        //RenderPass

        let render_pass_info = vk::RenderPassCreateInfo::builder()
            .attachments(&[color_attachment])
            .subpasses(&[subpass])
            .dependencies(&[dependency])
            .build();

        unsafe { device.create_render_pass(&render_pass_info, None).unwrap() }
    }

    fn create_frame_buffers(
        device: &Device,
        render_pass: vk::RenderPass,
        swap_chain_image_views: Vec<vk::ImageView>,
        swap_chain_extent: vk::Extent2D,
    ) -> Vec<vk::Framebuffer> {
        let mut swap_chain_frame_buffers = vec![];

        //vkImagesに割り当てていく
        for image_view in swap_chain_image_views {
            let frame_buffer_info = vk::FramebufferCreateInfo::builder()
                //FrameBufferがどのRender passと互換性を持つかを指定
                //FrameBufferは互換性のあるレンダーパスでのみ使用できる
                .render_pass(render_pass)
                //RenderPassのpAttachment配列内のそれぞれのAttachmentに対してどのImageViewが紐づくべきかを指定
                .attachments(&[image_view])
                .width(swap_chain_extent.width)
                .height(swap_chain_extent.height)
                //画像配列のレイヤー数を指定
                .layers(1)
                .build();

            swap_chain_frame_buffers
                .push(unsafe { device.create_framebuffer(&frame_buffer_info, None).unwrap() });
        }

        swap_chain_frame_buffers
    }

    fn create_command_pool(
        instance: &Instance,
        surface: &Surface,
        surface_khr: SurfaceKHR,
        physical_device: PhysicalDevice,
        device: &Device,
    ) -> vk::CommandPool {
        let queue_family_indices = QueueFamilyIndices::find_queue_families(
            instance,
            surface,
            surface_khr,
            physical_device,
        );

        //Command Bufferのメモリ管理をするためのCommand Pool
        let pool_info = vk::CommandPoolCreateInfo::builder()
            //flagは二種類存在し、
            //VK_COMMAND_POOL_CREATE_TRANSIENT_BITはプールが割り当てたコマンドバッファが短命であることを指定
            //VK_COMMAND_POOL_CREATE_RESET_COMMAND_BUFFER_BITはそのコマンドバッファをコマンドを積む際にResetして使い回すことを指定
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            //Command Poolは単一のキューファミリータイプに対して作られる
            //今回はグラフィックスキューファミリーを選択
            .queue_family_index(queue_family_indices.graphics_family.unwrap())
            .build();

        unsafe { device.create_command_pool(&pool_info, None).unwrap() }
    }

    //Command Bufferは所属するCommand Poolが破棄されるタイミングで自動的に破棄される
    fn create_command_buffers(
        device: &Device,
        command_pool: CommandPool,
        size: u32,
    ) -> Vec<vk::CommandBuffer> {
        let alloc_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(command_pool)
            //コマンドバッファがプライマリなのかセカンダリなのを指定
            //PRIMARY: 直接キューに対してサブミットすることができる
            //SECONDARY: 直接キューに対してサブミットすることは出来ないがプライマリコマンドバッファから間接的に呼び出すことができる
            //SECONDARYは共通の操作をまとめて再利用したりする時に便利
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(size)
            .build();

        unsafe { device.allocate_command_buffers(&alloc_info).unwrap() }
    }

    fn record_command_buffer(&self, image_index: usize) {
        //コマンドバッファもフレームバッファもインデックスは同じものを紐づけてあげる
        //今回は１つだけ使うためfirst()
        let command_buffer = *self.command_buffers.first().unwrap();

        //swapchainにpresentするときにimage_indexを渡してあげているのでそれと同等のものを使用できるようにしてあげる
        let swap_chain_frame_buffer = self.swap_chain_frame_buffers[image_index];

        let begin_info = vk::CommandBufferBeginInfo::builder()
            //コマンドバッファの使用方法を指定
            //ONE_TIME_SUBMIT: コマンドバッファを一度ジック押したらまたすぐに再記録する
            //PASS_CONTINUE: 一回のレンダリングパスの中で完結するSECONDARYコマンドバッファ
            //SIMULTANEOUS_USE: コマンドバッファを実行または保留中に再度Submitすることができる
            .flags(vk::CommandBufferUsageFlags::empty())
            //この値はSECONDARYコマンドバッファに対してのみ適用される
            //これはPRIMARYなコマンドバッファからどのように状態を継承するかを指定する
            //.inheritance_info()
            .build();

        unsafe {
            self.device
                //コマンドバッファの記録を開始する
                //一度記録されたコマンドバッファに対してもう一度このメソッドを呼び出すと、リセットが暗黙的に走る
                .begin_command_buffer(command_buffer, &begin_info)
                .unwrap()
        };

        let clear_color = vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [0.0, 0.0, 0.0, 1.0],
            },
        };

        let render_pass_info = vk::RenderPassBeginInfo::builder()
            //レンダーパスとカラーアタッチメントとして登録されたframebufferを紐づけ
            .render_pass(self.render_pass)
            .framebuffer(swap_chain_frame_buffer)
            .render_area(
                //レンダリング領域の大きさを指定
                //レンダリング領域とはシェーダのロードとストアが行われる場所
                //この領域外のピクセルの値は未定義となる
                vk::Rect2D::builder()
                    .offset(vk::Offset2D::builder().x(0).y(0).build())
                    .extent(self.swap_chain_extent)
                    .build(),
            )
            //color_attachmentの定義時に指定したLOAD_OP_CLEARに使用するクリア値の設定
            .clear_values(&[clear_color])
            .build();

        //コマンドを積む
        unsafe {
            //コマンドを記録するすべての関数はprefixとしてcmd(本家だとvkCmd)がつく
            //基本的にこれらの関数の実行時にはコマンドを記録しているだけで実際に実行しているわけではないので、返り値がResultになっていない
            //個のコマンドを使用することで描画が始まる
            self.device.cmd_begin_render_pass(
                command_buffer,
                &render_pass_info,
                //render_pass内の描画コマンドをどのように提供するかを指定
                //INLINE: render_pass内のコマンドはPRIMARYなコマンドバッファ自体に埋め込まれSECODARYは実行されない
                //SECONDARY_COMMAND_BUFFER: render_pass内のコマンドはSECONDARYなコマンドバッファから実行される
                vk::SubpassContents::INLINE,
            );

            //Graphics Pipelineをコマンドバッファに対して紐づける
            self.device.cmd_bind_pipeline(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline,
            );

            //三角形を描画する処理を発行
            self.device.cmd_draw(
                command_buffer,
                //頂点バッファのサイズ設定
                3,
                //インスタンス数
                1,
                //頂点バッファのオフセット
                0,
                //インスタンスのオフセットでgl_InstanceIndexの最小値となる
                0,
            );

            //render_pass系コマンドの終わり
            self.device.cmd_end_render_pass(command_buffer);
        };

        unsafe { self.device.end_command_buffer(command_buffer).unwrap() };
    }

    fn create_sync_objects(
        device: &Device,
        size: u32,
    ) -> (Vec<vk::Semaphore>, Vec<vk::Semaphore>, Vec<vk::Fence>) {
        //SemaphoreCreateInfoは今のところsTypeは必須ではなく今後のバージョンによりflagsやpNextが追加される可能性がある
        let semaphore_info = vk::SemaphoreCreateInfo::builder().build();

        //こちらもSemaphoreCreateInfo同様
        let fence_info = vk::FenceCreateInfo::builder()
            //最初の位置フレーム目はFenceの待機がレンダリング前に入るのでシグナリングを行うものがいないのに待機してしまう
            //なので最初はシグナリングされた状態で作る
            .flags(vk::FenceCreateFlags::SIGNALED)
            .build();

        let mut image_available_semaphores = vec![];
        let mut render_finished_semaphores = vec![];
        let mut in_flight_fences = vec![];

        for _ in 0..size {
            image_available_semaphores
                .push(unsafe { device.create_semaphore(&semaphore_info, None).unwrap() });
            render_finished_semaphores
                .push(unsafe { device.create_semaphore(&semaphore_info, None).unwrap() });

            in_flight_fences.push(unsafe { device.create_fence(&fence_info, None).unwrap() });
        }

        (
            image_available_semaphores,
            render_finished_semaphores,
            in_flight_fences,
        )
    }
}

impl Drop for VulkanApp {
    fn drop(&mut self) {
        log::debug!("Dropping application.");
        unsafe {
            self.cleanup_swap_chain();

            self.device.destroy_command_pool(self.command_pool, None);

            for semaphore in self.image_available_semaphores.clone() {
                self.device.destroy_semaphore(semaphore, None);
            }

            for semaphore in self.render_finished_semaphores.clone() {
                self.device.destroy_semaphore(semaphore, None);
            }

            for fence in self.in_flight_fences.clone() {
                self.device.destroy_fence(fence, None);
            }

            if let Some(debug_utils) = &self.debug_utils {
                debug_utils.destroy_debug_utils_messenger(
                    self.debug_utils_messenger_ext
                        .expect("DebugUtilsMessengerEXTが存在しません"),
                    None,
                );
            }

            self.device.destroy_device(None);

            self.surface.destroy_surface(self.surface_khr, None);

            self.instance.destroy_instance(None); //ライフタイムが聞いてても呼ばないと駄目
        }
    }
}
