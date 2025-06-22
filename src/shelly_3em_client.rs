use std::net::SocketAddr;

use client::Context;
use tokio_modbus::prelude::*;

pub struct Shelly3EMClient {
    connection: Context,
}
// Registers are documented here
// https://shelly-api-docs.shelly.cloud/gen2/ComponentsAndServices/EM/#modbus-registers

impl Shelly3EMClient {
    pub async fn new(target_device: SocketAddr) -> Self {
        let connection = tcp::connect(target_device)
            .await
            .expect("Cant Connect to Shelly 3EM");

        Self { connection }
    }
    pub async fn read_total_power(&mut self) -> Option<f32> {
        if let Ok(total_readings) = self.connection.read_input_registers(1013, 2).await.unwrap() {
            // Convert the bytes of the totals into floats and send onwards
            let total_active_power = merge_u16_f32(total_readings[0], total_readings[1]);
            Some(total_active_power)
        } else {
            None
        }
    }
}
fn merge_u16_f32(a: u16, b: u16) -> f32 {
    let x: u32 = a as u32 | (b as u32) << 16;
    f32::from_bits(x)
}
