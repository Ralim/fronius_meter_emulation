use crate::home_assistant::HomeAssistantAPI;
use crate::rolling_average::RollingAverage;
use std::env;
use std::time::Duration;
use tokio::sync::mpsc::Sender;
use tokio::time::{interval, sleep};

/// Isolated thread for reading offset values from Home Assistant
pub struct HomeAssistantReader {
    import_sensor: String,
    export_sensor: String,
    should_smooth: bool,
    update_sender: Sender<f32>,
    ha_client: HomeAssistantAPI,
    filtered_offset: RollingAverage,
}

impl HomeAssistantReader {
    /// Creates a new Home Assistant reader that will send offset updates via the provided channel
    pub fn new(update_sender: Sender<f32>) -> Self {
        let import_sensor = env::var("HA_EXTRA_IMPORT").unwrap_or_default();
        let export_sensor = env::var("HA_EXTRA_EXPORT").unwrap_or_default();
        let should_smooth = parse_bool_safe(env::var("HA_SMOOTH").ok());

        println!("Home Assistant Reader Config:");
        println!(
            "  Import sensor: {}",
            if import_sensor.is_empty() {
                "none"
            } else {
                &import_sensor
            }
        );
        println!(
            "  Export sensor: {}",
            if export_sensor.is_empty() {
                "none"
            } else {
                &export_sensor
            }
        );
        println!("  Smoothing: {}", should_smooth);

        Self {
            import_sensor,
            export_sensor,
            should_smooth,
            update_sender,
            ha_client: HomeAssistantAPI::new(),
            filtered_offset: RollingAverage::default(),
        }
    }

    /// Spawns the Home Assistant reader in its own isolated thread
    pub fn spawn(mut self) {
        tokio::spawn(async move {
            self.run().await;
        });
    }

    /// Main execution loop for the Home Assistant reader thread
    async fn run(&mut self) {
        println!("Starting Home Assistant offset reader thread");

        // If no sensors are configured, send zero offset once and exit
        if self.import_sensor.is_empty() && self.export_sensor.is_empty() {
            println!("No Home Assistant sensors configured, sending zero offset");
            if let Err(e) = self.update_sender.send(0.0).await {
                println!("Failed to send zero offset: {}", e);
            }
            return;
        }

        let mut read_interval = interval(Duration::from_millis(1000)); // Read HA less frequently than Shelly

        loop {
            read_interval.tick().await;

            match self.read_offset_with_retry().await {
                Ok(offset) => {
                    // Send the offset to the meter emulator
                    if let Err(e) = self.update_sender.send(offset).await {
                        panic!(
                            "Failed to send Home Assistant offset update: {}. Shutting down HA reader.",
                            e
                        );
                    }
                }
                Err(e) => {
                    println!("Home Assistant read error: {}", e);

                    // On error, send 0W offset as fallback

                    if let Err(e) = self.update_sender.send(0.0).await {
                        panic!("Failed to send fallback offset: {}", e);
                    }
                }
            }
        }
    }

    /// Reads offset data with automatic retry
    async fn read_offset_with_retry(&mut self) -> Result<f32, String> {
        const MAX_RETRIES: u32 = 3;

        for attempt in 1..=MAX_RETRIES {
            match self.read_offset().await {
                Ok(offset) => return Ok(offset),
                Err(e) => {
                    if attempt < MAX_RETRIES {
                        println!("HA read attempt {} failed: {}. Retrying...", attempt, e);
                        sleep(Duration::from_millis(200 * attempt as u64)).await;
                    } else {
                        return Err(format!(
                            "All {} attempts failed. Last error: {}",
                            MAX_RETRIES, e
                        ));
                    }
                }
            }
        }

        unreachable!()
    }

    /// Reads offset from Home Assistant sensors
    async fn read_offset(&mut self) -> Result<f32, String> {
        let ha_import = self.read_sensor_value(&self.import_sensor.clone()).await?;
        let ha_export = self.read_sensor_value(&self.export_sensor.clone()).await?;

        let raw_offset = ha_import - ha_export;
        let final_offset = if self.should_smooth {
            self.filtered_offset.add(raw_offset)
        } else {
            raw_offset
        };

        println!(
            "HA offset: Import {}W, Export {}W, Raw offset {}W, Final offset {}W",
            ha_import, ha_export, raw_offset, final_offset
        );

        Ok(final_offset)
    }

    /// Reads a single sensor value from Home Assistant
    async fn read_sensor_value(&mut self, sensor_name: &str) -> Result<f32, String> {
        if sensor_name.is_empty() {
            return Ok(0.0);
        }

        match self.ha_client.read_sensor_value(sensor_name).await {
            Ok(sensor) => sensor.state.parse::<f32>().map_err(|e| {
                format!(
                    "Failed to parse sensor {} value '{}': {}",
                    sensor_name, sensor.state, e
                )
            }),
            Err(e) => Err(format!("Failed to read sensor {}: {}", sensor_name, e)),
        }
    }
}

/// Safely parses a boolean from an optional string, defaulting to false
fn parse_bool_safe(val: Option<String>) -> bool {
    val.unwrap_or_default()
        .to_ascii_lowercase()
        .parse()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bool_safe() {
        // Test None input
        assert_eq!(parse_bool_safe(None), false);

        // Test empty string
        assert_eq!(parse_bool_safe(Some("".to_string())), false);

        // Test "true" variations
        assert_eq!(parse_bool_safe(Some("true".to_string())), true);
        assert_eq!(parse_bool_safe(Some("True".to_string())), true);
        assert_eq!(parse_bool_safe(Some("TRUE".to_string())), true);
        assert_eq!(parse_bool_safe(Some("TrUe".to_string())), true);

        // Test "false" variations
        assert_eq!(parse_bool_safe(Some("false".to_string())), false);
        assert_eq!(parse_bool_safe(Some("False".to_string())), false);
        assert_eq!(parse_bool_safe(Some("FALSE".to_string())), false);
        assert_eq!(parse_bool_safe(Some("FaLsE".to_string())), false);

        // Test invalid strings (should default to false)
        assert_eq!(parse_bool_safe(Some("yes".to_string())), false);
        assert_eq!(parse_bool_safe(Some("no".to_string())), false);
        assert_eq!(parse_bool_safe(Some("1".to_string())), false);
        assert_eq!(parse_bool_safe(Some("0".to_string())), false);
        assert_eq!(parse_bool_safe(Some("invalid".to_string())), false);
        assert_eq!(parse_bool_safe(Some("random text".to_string())), false);
    }
}
