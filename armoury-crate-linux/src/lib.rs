//! Armoury Crate Linux - Gaming-focused control center for ASUS ROG laptops
//!
//! This is the Linux equivalent of ASUS Armoury Crate, providing:
//! - System monitoring dashboard with real-time metrics
//! - Scenario profiles for per-application settings
//! - RGB keyboard control (Aura)
//! - AniMe Matrix display control
//! - Custom fan curves
//! - Platform profile management
//! - TDP and power control

pub mod config;
pub mod error;
pub mod monitoring;
pub mod types;
pub mod ui;

// Re-export slint for external use
pub use slint;

// Include the generated Slint code
slint::include_modules!();

/// Print version information
pub fn print_versions() {
    let self_version = env!("CARGO_PKG_VERSION");
    println!("Armoury Crate Linux v{}", self_version);
}
