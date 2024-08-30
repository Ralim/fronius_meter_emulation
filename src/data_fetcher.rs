use std::{env, time::Duration};

use crate::{
    home_assistant::HomeAssistantAPI, shelly_3em_client::Shelly3EMClient,
    smart_meter_emulator::Readings,
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
        println!("Connecting to shelly `{shelly_modbus}`");
        let mut shelly_client = Shelly3EMClient::new(shelly_modbus.parse().unwrap()).await;
        let mut home_assistant_client = HomeAssistantAPI::new();

        let mut interval = time::interval(Duration::from_millis(500));
        loop {
            // Now we read the shelly, and also read the HA offset
            let shelly_net_power = shelly_client.read_total_power().await;
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
            let summed_power =
                shelly_net_power.expect("Didnt get shelly power") + ha_import - ha_export;
            println!(
                "Summed power {summed_power}W, shelly {:?}W, HA Import {}W Export {}W",
                shelly_net_power, ha_import, ha_export
            );
            Self::send_power(summed_power, &output).await;
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
