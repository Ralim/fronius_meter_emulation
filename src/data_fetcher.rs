use std::{env, time::Duration};

use crate::{
    home_assistant::HomeAssistantAPI, rolling_average::RollingAverage,
    shelly_3em_client::Shelly3EMClient, smart_meter_emulator::Readings,
};
use tokio::{sync::mpsc::Sender, time};

// Implements reading the Shelly unit and then adjusting power metrics

pub struct DataFetcher {}

impl DataFetcher {
    pub fn new(output: Sender<Readings>) -> Self {
        tokio::spawn(async move {
            Self::worker(output).await;
        });
        Self {}
    }

    async fn worker(output: Sender<Readings>) {
        // 1. Open link to read from Shelly Unit
        // 2. Open link to read from HA
        let home_assistant_extra_import_sensor = env::var("HA_EXTRA_IMPORT").unwrap_or_default();
        let home_assistant_extra_export_sensor = env::var("HA_EXTRA_EXPORT").unwrap_or_default();
        let shelly_modbus =
            env::var("SHELLY_MODBUS").expect("Required to add Shelly modbus connection info");
        let slow_meter_read_interval: Duration = Duration::from_secs(
            env::var("SLOW_METER_READ_INTERVAL_S")
                .unwrap_or("60".to_owned())
                .parse()
                .unwrap_or(60),
        );
        println!("Slow meter read interval: {slow_meter_read_interval:?}");

        println!("Connecting to shelly `{shelly_modbus}`");
        let mut shelly_client = Shelly3EMClient::new(shelly_modbus.parse().unwrap()).await;
        let mut home_assistant_client = HomeAssistantAPI::new();

        if !home_assistant_client.is_configured() {
            println!("Home assistant integration disabled")
        }

        println!("Running");
        let should_smooth = parse_bool_safe(env::var("HA_SMOOTH").ok());
        let mut filtered_ha_offset = RollingAverage::default();
        let mut interval = time::interval(Duration::from_millis(500));
        let mut last_slow_meter_read = time::Instant::now();
        loop {
            // Now we read the shelly, and also read the HA offset
            let shelly_net_power = shelly_client.read_total_power().await;
            let (ha_import, ha_export, ha_offset) = if home_assistant_client.is_configured() {
                let ha_import = Self::read_ha_sensor_or_null(
                    &home_assistant_extra_import_sensor,
                    &mut home_assistant_client,
                )
                .await;
                let ha_export = Self::read_ha_sensor_or_null(
                    &home_assistant_extra_export_sensor,
                    &mut home_assistant_client,
                )
                .await;
                let ha_offset = if should_smooth {
                    filtered_ha_offset.add(ha_import - ha_export)
                } else {
                    ha_import - ha_export
                };
                (ha_import, ha_export, ha_offset)
            } else {
                (0.0, 0.0, 0.0)
            };
            let summed_power = shelly_net_power.expect("Didn't get shelly power") + ha_offset;
            println!(
                "Summed power {summed_power}W, shelly {:?}W, HA Import {}W Export {}W",
                shelly_net_power, ha_import, ha_export
            );
            Self::send_power(summed_power, &output).await;
            if last_slow_meter_read.elapsed() > slow_meter_read_interval {
                // Periodically we read sensors we want to slowly update
                // This is the phase voltages, frequencies, etc
                let phase_abc_readings = shelly_client.read_phase_power_information().await;
                if let Some(phase_abc_readings) = phase_abc_readings {
                    // Send these off to the emulator
                    output
                        .send(Readings::PhaseAVoltage(phase_abc_readings[0].voltage))
                        .await
                        .expect("Cant send readings to fake meter");
                    output
                        .send(Readings::PhaseAPF(phase_abc_readings[0].power_factor))
                        .await
                        .expect("Cant send readings to fake meter");
                    output
                        .send(Readings::PhaseACurrent(phase_abc_readings[0].current))
                        .await
                        .expect("Cant send readings to fake meter");
                    output
                        .send(Readings::PhaseAWatts(phase_abc_readings[0].active_power))
                        .await
                        .expect("Cant send readings to fake meter");
                    output
                        .send(Readings::PhaseAVAR(phase_abc_readings[0].apparent_power))
                        .await
                        .expect("Cant send readings to fake meter");
                    // Phase B
                    output
                        .send(Readings::PhaseBVoltage(phase_abc_readings[1].voltage))
                        .await
                        .expect("Cant send readings to fake meter");
                    output
                        .send(Readings::PhaseBPF(phase_abc_readings[1].power_factor))
                        .await
                        .expect("Cant send readings to fake meter");
                    output
                        .send(Readings::PhaseBCurrent(phase_abc_readings[1].current))
                        .await
                        .expect("Cant send readings to fake meter");
                    output
                        .send(Readings::PhaseBWatts(phase_abc_readings[1].active_power))
                        .await
                        .expect("Cant send readings to fake meter");
                    output
                        .send(Readings::PhaseBVAR(phase_abc_readings[1].apparent_power))
                        .await
                        .expect("Cant send readings to fake meter");
                    // Phase C
                    output
                        .send(Readings::PhaseCVoltage(phase_abc_readings[2].voltage))
                        .await
                        .expect("Cant send readings to fake meter");
                    output
                        .send(Readings::PhaseCPF(phase_abc_readings[2].power_factor))
                        .await
                        .expect("Cant send readings to fake meter");
                    output
                        .send(Readings::PhaseCCurrent(phase_abc_readings[2].current))
                        .await
                        .expect("Cant send readings to fake meter");
                    output
                        .send(Readings::PhaseCWatts(phase_abc_readings[2].active_power))
                        .await
                        .expect("Cant send readings to fake meter");
                    output
                        .send(Readings::PhaseCVAR(phase_abc_readings[2].apparent_power))
                        .await
                        .expect("Cant send readings to fake meter");
                    // Others
                    output
                        .send(Readings::AveragePhaseVoltage(
                            (phase_abc_readings[0].voltage
                                + phase_abc_readings[1].voltage
                                + phase_abc_readings[2].voltage)
                                / 3.0,
                        ))
                        .await
                        .expect("Cant send readings to fake meter");
                    output
                        .send(Readings::NetACCurrent(
                            phase_abc_readings[0].current
                                + phase_abc_readings[1].current
                                + phase_abc_readings[2].current,
                        ))
                        .await
                        .expect("Cant send readings to fake meter");
                }
                last_slow_meter_read = time::Instant::now();
            }
            interval.tick().await; // Wait for next sample time
        }
    }
    async fn read_ha_sensor_or_null(
        sensor_name: &str,
        home_assistant_client: &mut HomeAssistantAPI,
    ) -> f32 {
        let home_assistant_offset_str = home_assistant_client.read_sensor_value(sensor_name).await;
        let ha_offset: f32 = match home_assistant_offset_str {
            Ok(res) => res.state.parse().unwrap_or_default(),
            Err(e) => {
                println!("Didn't read HA offset {e:?}");
                0.0
            }
        };
        ha_offset
    }
    async fn send_power(summed_power: f32, output: &Sender<Readings>) {
        output
            .send(Readings::TotalRealPower(summed_power))
            .await
            .expect("Cant send readings to fake meter");
        output
            .send(Readings::ReactivePower(summed_power))
            .await
            .expect("Cant send readings to fake meter");
        output
            .send(Readings::NetACCurrent(summed_power))
            .await
            .expect("Cant send readings to fake meter");
    }
}

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
