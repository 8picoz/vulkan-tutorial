use winit::event_loop::EventLoop;
use winit::window::{Window, WindowBuilder};

pub struct WindowHandlers {
    /// event_loop.runするには所有権を消費しなければいけないが
    /// VulkanAppにEventLoopを持たせてしまうと
    /// Dropが存在するのでrunのタイミングでVulkanAppの所有権を消費することが出来ない
    /// (VulkanAppのフィールドが消費されてしまったらDropで呼べない)
    pub event_loop: EventLoop<()>,
    pub window: Window,
}

impl WindowHandlers {
    pub fn new() -> Self {
        let event_loop = winit::event_loop::EventLoop::new();

        let window = WindowBuilder::new()
            .with_title("vulkan_tutorial")
            .with_inner_size(winit::dpi::LogicalSize::new(800.0f32, 800.0f32))
            .with_resizable(true)
            .build(&event_loop)
            .unwrap();

        Self { event_loop, window }
    }
}
