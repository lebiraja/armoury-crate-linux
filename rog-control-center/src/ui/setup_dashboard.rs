use slint::ComponentHandle;
use std::time::Duration;
use crate::{DashboardData, MainWindow};
use crate::monitor::SystemMonitor;

pub fn setup_dashboard_page(ui: &MainWindow) {
    let handle = ui.as_weak();
    
    // Spawn a thread to update stats
    std::thread::spawn(move || {
        let mut monitor = SystemMonitor::new();
        loop {
            monitor.update();
            let cpu = monitor.get_cpu_usage();
            let ram = monitor.get_ram_usage();
            let gpu = monitor.get_gpu_usage();
            
            let handle_copy = handle.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = handle_copy.upgrade() {
                    let data = ui.global::<DashboardData>();
                    data.set_cpu_usage(cpu);
                    data.set_ram_usage(ram);
                    data.set_gpu_usage(gpu);
                }
            });
            
            std::thread::sleep(Duration::from_secs(1));
        }
    });
}
