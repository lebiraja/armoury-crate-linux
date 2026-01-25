use sysinfo::System;

pub struct SystemMonitor {
    sys: System,
}

impl SystemMonitor {
    pub fn new() -> Self {
        let sys = System::new_all();
        Self { sys }
    }

    pub fn update(&mut self) {
        self.sys.refresh_all();
    }

    pub fn get_cpu_usage(&self) -> f32 {
        self.sys.global_cpu_info().cpu_usage()
    }

    pub fn get_ram_usage(&self) -> f32 {
        let total = self.sys.total_memory() as f32;
        let used = self.sys.used_memory() as f32;
        if total == 0.0 {
            0.0
        } else {
            (used / total) * 100.0
        }
    }

    pub fn get_gpu_usage(&self) -> f32 {
        // GPU usage is tricky on Linux without specific drivers (NVML or AMDGPU stats).
        // For now, we'll try to read from common sysfs paths or return 0.0 if unavailable.
        // This is a placeholder for a more complex implementation (e.g., using nvml-wrapper).
        0.0
    }
}
