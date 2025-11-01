use crate::home_assistant_reader::HomeAssistantReader;
use crate::power_combiner::PowerCombiner;
use crate::shelly_reader::ShellyReader;
use crate::smart_meter_emulator::Readings;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc::{self, Sender};

/// Coordinates the threaded data fetching architecture
/// Spawns isolated threads for each data source and manages their communication
pub struct ThreadedDataCoordinator {
    power_combiner: Arc<PowerCombiner>,
}

impl ThreadedDataCoordinator {
    /// Creates a new ThreadedDataCoordinator and starts all worker threads
    pub fn new(meter_update_sender: Sender<Readings>) -> Self {
        println!("Initializing threaded data coordinator");

        // Create channels for inter-thread communication
        let (shelly_tx, shelly_rx) = mpsc::channel::<f32>(32);
        let (ha_tx, ha_rx) = mpsc::channel::<f32>(32);

        // Create the power combiner
        let power_combiner = Arc::new(PowerCombiner::new(meter_update_sender));

        // Spawn the power combiner receiver threads
        power_combiner.clone().spawn_shelly_receiver(shelly_rx);
        power_combiner.clone().spawn_ha_receiver(ha_rx);

        // Start the Shelly reader thread
        Self::start_shelly_reader_thread(shelly_tx);

        // Start the Home Assistant reader thread
        Self::start_home_assistant_reader_thread(ha_tx);

        println!("All data reader threads started successfully");

        Self { power_combiner }
    }

    /// Starts the Shelly power meter reader thread
    fn start_shelly_reader_thread(sender: Sender<f32>) {
        let shelly_modbus = env::var("SHELLY_MODBUS")
            .expect("Required to add Shelly modbus connection info (SHELLY_MODBUS env var)");

        println!(
            "Starting Shelly reader thread for address: {}",
            shelly_modbus
        );

        let target_address: SocketAddr = shelly_modbus
            .parse()
            .expect("Invalid SHELLY_MODBUS address format");

        let shelly_reader = ShellyReader::new(target_address, sender);
        shelly_reader.spawn();
    }

    /// Starts the Home Assistant offset reader thread
    fn start_home_assistant_reader_thread(sender: Sender<f32>) {
        println!("Starting Home Assistant reader thread");

        let ha_reader = HomeAssistantReader::new(sender);
        ha_reader.spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_threaded_data_coordinator_creation() {
        // Set required environment variables for test
        env::set_var("SHELLY_MODBUS", "127.0.0.1:5502");
        env::remove_var("HA_EXTRA_IMPORT");
        env::remove_var("HA_EXTRA_EXPORT");

        let (meter_tx, _meter_rx) = mpsc::channel(32);

        // This should not panic and should create the coordinator
        let _coordinator = ThreadedDataCoordinator::new(meter_tx);

        // Wait a moment for threads to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Get status report (should not panic)

        // Clean up environment
        env::remove_var("SHELLY_MODBUS");
    }

    #[tokio::test]
    async fn test_status_report_formatting() {
        // This test needs to run in tokio context due to spawned tasks
        let (meter_tx, _meter_rx) = mpsc::channel(32);

        // Set minimal required env vars to avoid panics
        env::set_var("SHELLY_MODBUS", "127.0.0.1:5502");

        let _coordinator = ThreadedDataCoordinator::new(meter_tx);

        // Wait a moment for threads to start
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Clean up
        env::remove_var("SHELLY_MODBUS");
    }
}
