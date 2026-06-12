//! Route modules for API v1

pub mod monitoring;

// Re-export route configuration functions
pub use monitoring::configure_monitoring;
