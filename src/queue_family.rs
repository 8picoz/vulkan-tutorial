use ash::vk::QueueFlags;
use ash::{vk, Instance};

pub struct QueueFamilyIndices {
    pub graphics_family: Option<u32>,
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
        device: vk::PhysicalDevice,
    ) -> QueueFamilyIndices {
        let queue_families =
            unsafe { instance.get_physical_device_queue_family_properties(device) };

        let mut queue_family_indices = Self::new();

        for (i, queue) in queue_families.iter().enumerate() {
            if queue.queue_flags.contains(QueueFlags::GRAPHICS) {
                queue_family_indices.graphics_family = Some(i as u32);
            }

            if queue_family_indices.is_complete() {
                break;
            }
        }

        queue_family_indices
    }

    //実行したい動作に対して適しているかどうかを判定
    #[allow(dead_code)]
    pub fn is_device_suitable(instance: &Instance, device: vk::PhysicalDevice) -> bool {
        let indices = Self::find_queue_families(instance, device);

        indices.is_complete()
    }

    //is_device_suitableの採点版
    #[allow(dead_code)]
    pub fn rate_device_suitability(instance: &Instance, device: vk::PhysicalDevice) -> usize {
        let mut score = 0;

        let device_properties = unsafe { instance.get_physical_device_properties(device) };
        let device_features = unsafe { instance.get_physical_device_features(device) };

        if device_properties.device_type == vk::PhysicalDeviceType::DISCRETE_GPU {
            score += 1000;
        }

        score += device_properties.limits.max_image_dimension2_d as usize;

        if device_features.geometry_shader == 0 {
            return 0;
        }

        score
    }

    fn is_complete(&self) -> bool {
        self.graphics_family.is_some()
    }
}
