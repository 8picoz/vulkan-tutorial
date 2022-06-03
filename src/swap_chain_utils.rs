use ash::vk;

pub struct SwapChainSupportDetails {
    //サポートされる機能一覧を取得できる
    pub capabilities: vk::SurfaceCapabilitiesKHR,
    //サーフェイスのサポートする画像フォーマットについて
    pub formats: Vec<vk::SurfaceFormatKHR>,
    pub present_modes: Vec<vk::PresentModeKHR>,
}

impl SwapChainSupportDetails {
    pub fn new(
        physical_device: vk::PhysicalDevice,
        surface: &ash::extensions::khr::Surface,
        surface_khr: vk::SurfaceKHR,
    ) -> Self {
        let capabilities = unsafe {
            surface
                //核となるような機能を取得するためdeviceとsurfaceが必要
                .get_physical_device_surface_capabilities(physical_device, surface_khr)
                .unwrap()
        };

        let formats = unsafe {
            surface
                .get_physical_device_surface_formats(physical_device, surface_khr)
                .unwrap()
        };

        let present_modes = unsafe {
            surface
                //プレゼンテーションモードについて
                .get_physical_device_surface_present_modes(physical_device, surface_khr)
                .unwrap()
        };

        Self {
            capabilities,
            formats,
            present_modes,
        }
    }

    pub fn choose_swap_surface_format(&self) -> vk::SurfaceFormatKHR {
        for available_format in self.formats.iter() {
            if available_format.format == vk::Format::R8G8B8A8_SRGB
                && available_format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            {
                return *available_format;
            }
        }

        *self.formats.first().unwrap()
    }

    pub fn choose_swap_present_mode(&self) -> vk::PresentModeKHR {
        for available_present_mode in self.present_modes.clone() {
            if available_present_mode == vk::PresentModeKHR::MAILBOX {
                return available_present_mode;
            }
        }

        vk::PresentModeKHR::FIFO
    }

    pub fn choose_swap_extent(&self, width: u32, height: u32) -> vk::Extent2D {
        if self.capabilities.current_extent.width != u32::MAX {
            return self.capabilities.current_extent;
        }

        let min = self.capabilities.min_image_extent;
        let max = self.capabilities.max_image_extent;
        let width = width.min(max.width).max(min.width);
        let height = height.min(max.height).max(min.height);

        vk::Extent2D { width, height }
    }
}
