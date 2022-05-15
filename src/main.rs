use std::env;

mod debug;
mod khr_util;
mod queue_family;
mod vulkan_app;

fn main() {
    env::set_var("RUST_LOG", "info");
    //env::set_var("RUST_LOG", "DEBUG");
    env_logger::init();
    match vulkan_app::VulkanApp::new() {
        Ok(mut app) => app.run(),
        Err(error) => log::error!("Failed to create application. Cause: {}", error),
    }
}
