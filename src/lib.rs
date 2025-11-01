//! Fronius Meter Emulation Library
//!
//! This library provides a threaded architecture for emulating a Fronius smart meter
//! by combining power readings from a Shelly 3EM device and offset values from Home Assistant.

pub mod home_assistant;
pub mod home_assistant_reader;
pub mod power_combiner;
pub mod rolling_average;
pub mod shelly_reader;
pub mod smart_meter_emulator;
pub mod threaded_data_coordinator;

// Re-export commonly used types for easier access
pub use home_assistant_reader::HomeAssistantReader;
pub use power_combiner::PowerCombiner;
pub use shelly_reader::ShellyReader;
pub use smart_meter_emulator::{Readings, SmartMeterEmulator};
pub use threaded_data_coordinator::ThreadedDataCoordinator;
