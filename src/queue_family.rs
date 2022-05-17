use ash::extensions::khr::Surface;
use ash::vk::{QueueFlags, SurfaceKHR};
use ash::{vk, Instance};
use log::debug;

pub struct QueueFamilyIndices {
    //キューファミリーは番号で管理されている
    //描画コマンドに対応しているかどうか
    pub graphics_family: Option<u32>,
    //プレゼンテーションに対応しているかどうか
    pub present_family: Option<u32>,
}

impl QueueFamilyIndices {
    fn new() -> Self {
        Self {
            graphics_family: None,
        }
    }

    //デバイスがVK_QUEUE_GRAPHICS_BITのQueueFamilyに対応してるか探す関数
    pub fn find_queue_families(
        instance: &Instance,
        surface: &Surface,
        surface_khr: vk::SurfaceKHR,
        physical_device: vk::PhysicalDevice,
    ) -> QueueFamilyIndices {
        let queue_families =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

        let mut queue_family_indices = Self::new();

        //複数個のグラフィックスファミリーキューとプレゼンテーションファミリーキューの組み合わせが存在する？
        for (i, queue) in queue_families.iter().enumerate() {
            //グラフィックスキューファミリーの確認
            if queue.queue_flags.contains(QueueFlags::GRAPHICS) {
                queue_family_indices.graphics_family = Some(i as u32);
            }

            let present_support = unsafe {
                surface.get_physical_device_surface_support(physical_device, i as u32, surface_khr)
            };

            //プレゼンテーションキューファミリーの確認
            if present_support.unwrap() {
                queue_family_indices.present_family = Some(i as u32);
            }

            if queue_family_indices.is_complete() {
                break;
            }
        }

        queue_family_indices
    }

    //実行したい動作に対して適しているかどうかを判定
    #[allow(dead_code)]
    pub fn is_device_suitable(instance: &Instance, physical_device: vk::PhysicalDevice) -> bool {
        let indices = Self::find_queue_families(instance, physical_device);

        indices.is_complete()
    }

    //is_device_suitableの採点版
    #[allow(dead_code)]
    pub fn rate_device_suitability(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
    ) -> usize {
        let mut score = 0;

        let device_properties = unsafe { instance.get_physical_device_properties(physical_device) };
        let device_features = unsafe { instance.get_physical_device_features(physical_device) };

        if device_properties.device_type == vk::PhysicalDeviceType::DISCRETE_GPU {
            score += 1000;
        }

        score += device_properties.limits.max_image_dimension2_d as usize;

        //geometry_shaderが使用できるかどうかの確認
        if device_features.geometry_shader == 0 {
            return 0;
        }

        score
    }

    //サポートするキューファミリの存在を確認できたかどうか
    fn is_complete(&self) -> bool {
        self.graphics_family.is_some() && self.present_family.is_some()
    }
}
