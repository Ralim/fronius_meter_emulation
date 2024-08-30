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

    // async fn worker(target_device: SocketAddr, output: Sender<Readings>) {
    //     // Loop reading the shelly and dumping it to the fake meter
    //     loop {
    //         // The Shelly samples the inputs at 1Hz
    //         // Starting at 1011 (Total Current) we read 3 floats for (Total Current,Active Power,Apparent Power)

    //         if let Ok(total_readings) = ctx.read_input_registers(1011, 2 * 3).await.unwrap() {
    //             // Convert the bytes of the totals into floats and send onwards
    //             let total_current = merge_u16_f32(total_readings[0], total_readings[1]);
    //             let total_active_power = merge_u16_f32(total_readings[2], total_readings[3]);
    //             let total_reactive_power = merge_u16_f32(total_readings[4], total_readings[5]);
    //             println!("Shelly Readings {total_current}A {total_active_power}VA {total_reactive_power}VAR");
    //             output
    //                 .send(Readings::TotalRealPower(total_active_power))
    //                 .await
    //                 .expect("Cant send readings to fake meter");
    //             output
    //                 .send(Readings::ReactivePower(total_reactive_power))
    //                 .await
    //                 .expect("Cant send readings to fake meter");
    //             output
    //                 .send(Readings::NetACCurrent(total_current))
    //                 .await
    //                 .expect("Cant send readings to fake meter");
    //         } else {
    //             panic!("Cant Read Shelly input regs for total power")
    //         }
    //         send_phase_a_readings(&mut ctx, &output)
    //             .await
    //             .expect("Cant send readings to fake meter");
    //         send_phase_b_readings(&mut ctx, &output)
    //             .await
    //             .expect("Cant send readings to fake meter");
    //         send_phase_c_readings(&mut ctx, &output)
    //             .await
    //             .expect("Cant send readings to fake meter");
    //         tokio::time::sleep(Duration::from_secs(1)).await;
    //     }
    // }
}
fn merge_u16_f32(a: u16, b: u16) -> f32 {
    let x: u32 = a as u32 | (b as u32) << 16;
    f32::from_bits(x)
}
