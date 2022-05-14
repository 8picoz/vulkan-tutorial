mod khr_util;
mod vulkan_app;

fn main() {
    env_logger::init();
    match vulkan_app::VulkanApp::new() {
        Ok(mut app) => app.run(),
        Err(error) => log::error!("Failed to create application. Cause: {}", error),
    }
}
