use std::net::SocketAddr;
use std::time::Duration;
use tokio::sync::mpsc::Sender;
use tokio::time::{interval, sleep};
use tokio_modbus::prelude::*;

/// Isolated thread for reading power data from Shelly 3EM device
pub struct ShellyReader {
    target_address: SocketAddr,
    update_sender: Sender<f32>,
    connection: Option<client::Context>,
}

impl ShellyReader {
    /// Creates a new Shelly reader that will send power updates via the provided channel
    pub fn new(target_address: SocketAddr, update_sender: Sender<f32>) -> Self {
        Self {
            target_address,
            update_sender,
            connection: None,
        }
    }

    /// Spawns the Shelly reader in its own isolated thread
    pub fn spawn(self) {
        tokio::spawn(async move {
            let mut reader = self;
            reader.run().await;
        });
    }

    /// Main execution loop for the Shelly reader thread
    async fn run(&mut self) {
        println!(
            "Starting Shelly power meter reader thread for {}",
            self.target_address
        );

        let mut read_interval = interval(Duration::from_millis(500));
        let mut consecutive_errors = 0u32;
        const MAX_CONSECUTIVE_ERRORS: u32 = 10;

        loop {
            read_interval.tick().await;

            match self.read_power_with_retry().await {
                Ok(power) => {
                    consecutive_errors = 0;

                    // Send the power reading to the meter emulator
                    if let Err(e) = self.update_sender.send(power).await {
                        println!(
                            "Failed to send Shelly power update: {}. Shutting down Shelly reader.",
                            e
                        );
                        break;
                    }
                }
                Err(e) => {
                    consecutive_errors += 1;
                    println!(
                        "Shelly read error ({}/{}): {}",
                        consecutive_errors, MAX_CONSECUTIVE_ERRORS, e
                    );

                    if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                        println!(
                            "Too many consecutive Shelly read errors. Shutting down Shelly reader."
                        );
                        break;
                    }

                    // Reset connection on error to force reconnect
                    self.connection = None;
                }
            }
        }

        println!("Shelly reader thread exiting");
    }

    /// Reads power data with automatic retry and reconnection
    async fn read_power_with_retry(&mut self) -> Result<f32, String> {
        // Ensure we have a connection
        if self.connection.is_none() {
            self.connection = self.connect_with_retry().await;
        }

        // If still no connection, return error
        if self.connection.is_none() {
            return Err("No connection available".to_string());
        }

        // Try to read power data
        match self.read_total_power().await {
            Ok(Some(power)) => Ok(power),
            Ok(None) => Err("No power data received".to_string()),
            Err(e) => {
                // Connection failed, reset it for next attempt
                self.connection = None;
                Err(format!("Modbus read failed: {}", e))
            }
        }
    }

    /// Connects to Shelly device with retry logic
    async fn connect_with_retry(&mut self) -> Option<client::Context> {
        const MAX_RETRIES: u32 = 3;

        for attempt in 1..=MAX_RETRIES {
            println!(
                "Connecting to Shelly 3EM at {} (attempt {}/{})",
                self.target_address, attempt, MAX_RETRIES
            );

            match tcp::connect(self.target_address).await {
                Ok(connection) => {
                    println!("Successfully connected to Shelly 3EM");
                    return Some(connection);
                }
                Err(e) => {
                    println!("Connection attempt {} failed: {}", attempt, e);
                    if attempt < MAX_RETRIES {
                        sleep(Duration::from_secs(1 << (attempt - 1))).await; // Exponential backoff
                    }
                }
            }
        }

        None
    }

    /// Reads total power from Shelly 3EM
    /// Registers are documented at: https://shelly-api-docs.shelly.cloud/gen2/ComponentsAndServices/EM/#modbus-registers
    async fn read_total_power(&mut self) -> Result<Option<f32>, Box<dyn std::error::Error>> {
        let connection = self.connection.as_mut().ok_or("No connection available")?;

        // tokio-modbus returns Result<Result<Vec<u16>, ExceptionCode>, Error>
        match connection.read_input_registers(1013, 2).await {
            Ok(modbus_result) => match modbus_result {
                Ok(total_readings) => {
                    if total_readings.len() >= 2 {
                        let total_active_power =
                            merge_u16_f32(total_readings[0], total_readings[1]);
                        Ok(Some(total_active_power))
                    } else {
                        Ok(None)
                    }
                }
                Err(exception) => Err(format!("Modbus exception: {:?}", exception).into()),
            },
            Err(io_error) => Err(format!("IO error: {:?}", io_error).into()),
        }
    }
}

/// Converts two u16 values into a f32 (little-endian)
fn merge_u16_f32(low: u16, high: u16) -> f32 {
    let combined: u32 = (low as u32) | ((high as u32) << 16);
    f32::from_bits(combined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_u16_f32() {
        // Test with known float bit pattern
        let test_float = 123.456f32;
        let bits = test_float.to_bits();
        let low = (bits & 0xFFFF) as u16;
        let high = (bits >> 16) as u16;

        let result = merge_u16_f32(low, high);
        assert_eq!(result, test_float);
    }

    #[test]
    fn test_merge_u16_f32_zero() {
        let result = merge_u16_f32(0, 0);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_merge_u16_f32_negative() {
        let test_float = -456.789f32;
        let bits = test_float.to_bits();
        let low = (bits & 0xFFFF) as u16;
        let high = (bits >> 16) as u16;

        let result = merge_u16_f32(low, high);
        assert_eq!(result, test_float);
    }
}
