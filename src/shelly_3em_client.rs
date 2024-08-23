use std::{net::SocketAddr, time::Duration};

use crate::smart_meter_emulator::Readings;
use client::Context;
use tokio::sync::mpsc::{error::SendError, Sender};
use tokio_modbus::prelude::*;

pub struct Shelly3EMClient {}
// Registers are documened here
// https://shelly-api-docs.shelly.cloud/gen2/ComponentsAndServices/EM/#modbus-registers

impl Shelly3EMClient {
    pub fn new(target_device: SocketAddr, output: Sender<Readings>) -> Self {
        tokio::spawn(async move {
            Self::worker(target_device, output).await;
        });
        Self {}
    }

    async fn worker(target_device: SocketAddr, output: Sender<Readings>) {
        let mut ctx = tcp::connect(target_device)
            .await
            .expect("Cant Connect to Shelly 3EM");

        // Loop reading the shelly and dumping it to the fake meter
        loop {
            // The Shelly samples the inputs at 1Hz
            // Starting at 1011 (Total Current) we read 3 floats for (Total Current,Active Power,Apparent Power)

            if let Ok(total_readings) = ctx.read_input_registers(1011, 2 * 3).await.unwrap() {
                // Convert the bytes of the totals into floats and send onwards
                let total_current = merge_u16_f32(total_readings[0], total_readings[1]);
                let total_active_power = merge_u16_f32(total_readings[2], total_readings[3]);
                let total_reactive_power = merge_u16_f32(total_readings[4], total_readings[5]);
                println!("Shelly Readings {total_current}A {total_active_power}VA {total_reactive_power}VAR");
                output
                    .send(Readings::TotalRealPower(total_active_power))
                    .await
                    .expect("Cant send readings to fake meter");
                output
                    .send(Readings::ReactivePower(total_reactive_power))
                    .await
                    .expect("Cant send readings to fake meter");
                output
                    .send(Readings::NetACCurrent(total_current))
                    .await
                    .expect("Cant send readings to fake meter");
            } else {
                panic!("Cant Read Shelly input regs for total power")
            }
            send_phase_a_readings(&mut ctx, &output)
                .await
                .expect("Cant send readings to fake meter");
            send_phase_b_readings(&mut ctx, &output)
                .await
                .expect("Cant send readings to fake meter");
            send_phase_c_readings(&mut ctx, &output)
                .await
                .expect("Cant send readings to fake meter");
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}
fn merge_u16_f32(a: u16, b: u16) -> f32 {
    let x: u32 = a as u32 | (b as u32) << 16;
    f32::from_bits(x)
}

async fn send_phase_a_readings(
    ctx: &mut Context,
    output: &Sender<Readings>,
) -> Result<(), SendError<Readings>> {
    let (voltage, current, active_power, apparent_power, power_factor) =
        read_shelly_phase_readings(1020, ctx).await;
    output.send(Readings::PhaseAVoltage(voltage)).await?;
    output.send(Readings::PhaseACurrent(current)).await?;
    output.send(Readings::PhaseAVA(active_power)).await?;
    output.send(Readings::PhaseAVAR(apparent_power)).await?;
    output.send(Readings::PhaseAPF(power_factor)).await?;
    Ok(())
}
async fn send_phase_b_readings(
    ctx: &mut Context,
    output: &Sender<Readings>,
) -> Result<(), SendError<Readings>> {
    let (voltage, current, active_power, apparent_power, power_factor) =
        read_shelly_phase_readings(1040, ctx).await;
    output.send(Readings::PhaseBVoltage(voltage)).await?;
    output.send(Readings::PhaseBCurrent(current)).await?;
    output.send(Readings::PhaseBVA(active_power)).await?;
    output.send(Readings::PhaseBVAR(apparent_power)).await?;
    output.send(Readings::PhaseBPF(power_factor)).await?;
    Ok(())
}

async fn send_phase_c_readings(
    ctx: &mut Context,
    output: &Sender<Readings>,
) -> Result<(), SendError<Readings>> {
    let (voltage, current, active_power, apparent_power, power_factor) =
        read_shelly_phase_readings(1060, ctx).await;
    output.send(Readings::PhaseCVoltage(voltage)).await?;
    output.send(Readings::PhaseCCurrent(current)).await?;
    output.send(Readings::PhaseCVA(active_power)).await?;
    output.send(Readings::PhaseCVAR(apparent_power)).await?;
    output.send(Readings::PhaseCPF(power_factor)).await?;
    Ok(())
}

async fn read_shelly_phase_readings(
    base_addr: u16,
    ctx: &mut Context,
) -> (f32, f32, f32, f32, f32) {
    //Read 5 floats from the shelly
    if let Ok(resp) = ctx.read_input_registers(base_addr, 2 * 5).await.unwrap() {
        // Convert the bytes of the totals into floats and send onwards
        let voltage = merge_u16_f32(resp[0], resp[1]);
        let current = merge_u16_f32(resp[2], resp[3]);
        let active_power = merge_u16_f32(resp[4], resp[5]);
        let apparent_power = merge_u16_f32(resp[6], resp[7]);
        let power_factor = merge_u16_f32(resp[8], resp[9]);
        // println!(
        //     "Shelly Readings for phase {base_addr} -> {voltage}V {current}A {active_power}VA {apparent_power}VAR {power_factor}pf"
        // );
        (voltage, current, active_power, apparent_power, power_factor)
    } else {
        panic!("Cant Read Shelly input regs for phase {base_addr}")
    }
}
