use crate::window_handlers::WindowHandlers;

use ash::vk::RefreshCycleDurationGOOGLEBuilder;
use log::info;
use std::env;

mod debug;
mod khr_util;
mod queue_family;
mod required_names;
mod swap_chain_utils;
mod vulkan_app;
mod window_handlers;

fn main() {
    env::set_var("RUST_LOG", "info");
    //env::set_var("RUST_LOG", "DEBUG");
    env_logger::init();

    //シェーダー確認
    const SHADER_PATH: &str = env!("rust_shader.spv");
    const SHADER: &[u8] = include_bytes!(env!("rust_shader.spv"));

    info!("Shader Path: {}", SHADER_PATH);
    info!("Shader Length: {}", SHADER.len());

    let window_handlers = WindowHandlers::new();

    match vulkan_app::VulkanApp::new(&window_handlers.window) {
        Ok(mut app) => app.run(window_handlers.event_loop),
        Err(error) => log::error!("Failed to create application. Cause: {}", error),
    }
}
