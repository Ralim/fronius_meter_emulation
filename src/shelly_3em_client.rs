use std::net::SocketAddr;

use client::Context;
use tokio_modbus::prelude::*;

pub struct Shelly3EMClient {
    connection: Context,
}
// Registers are documented here
// https://shelly-api-docs.shelly.cloud/gen2/ComponentsAndServices/EM/#modbus-registers
// https://shelly-api-docs.shelly.cloud/gen2/ComponentsAndServices/EMData/#modbus-registers

impl Shelly3EMClient {
    pub async fn new(target_device: SocketAddr) -> Self {
        let connection = tcp::connect(target_device)
            .await
            .expect("Cant Connect to Shelly 3EM");

        Self { connection }
    }
    pub async fn read_total_power(&mut self) -> Option<f32> {
        if let Ok(total_readings) = self
            .connection
            .read_input_registers(ShellyRegister::TotalPower as u16, 2)
            .await
            .unwrap()
        {
            // Convert the bytes of the totals into floats and send onwards
            let total_active_power = merge_u16_f32(total_readings[0], total_readings[1]);
            Some(total_active_power)
        } else {
            None
        }
    }
    pub async fn read_import_export_totals(&mut self) -> Option<(f32, f32)> {
        if let Ok(import_readings) = self
            .connection
            .read_input_registers(ShellyRegister::TotalWhImport as u16, 2)
            .await
            .unwrap()
        {
            let import_total = merge_u16_f32(import_readings[0], import_readings[1]);
            if let Ok(export_readings) = self
                .connection
                .read_input_registers(ShellyRegister::TotalWhExport as u16, 2)
                .await
                .unwrap()
            {
                let export_total = merge_u16_f32(export_readings[0], export_readings[1]);
                Some((import_total, export_total))
            } else {
                None
            }
        } else {
            None
        }
    }
    pub async fn read_phase_power_information(&mut self) -> Option<[PhaseMeasurements; 3]> {
        let phase_a = self
            .read_registers_f32(ShellyRegister::PhaseAVoltage, 6)
            .await?;
        let phase_b = self
            .read_registers_f32(ShellyRegister::PhaseBVoltage, 6)
            .await?;
        let phase_c = self
            .read_registers_f32(ShellyRegister::PhaseCVoltage, 6)
            .await?;

        Some([
            PhaseMeasurements {
                voltage: phase_a[0],
                current: phase_a[1],
                active_power: phase_a[2],
                apparent_power: phase_a[3],
                power_factor: phase_a[4],
                frequency: phase_a[5],
            },
            PhaseMeasurements {
                voltage: phase_b[0],
                current: phase_b[1],
                active_power: phase_b[2],
                apparent_power: phase_b[3],
                power_factor: phase_b[4],
                frequency: phase_b[5],
            },
            PhaseMeasurements {
                voltage: phase_c[0],
                current: phase_c[1],
                active_power: phase_c[2],
                apparent_power: phase_c[3],
                power_factor: phase_c[4],
                frequency: phase_c[5],
            },
        ])
    }
    async fn read_registers_f32(
        &mut self,
        base_register: ShellyRegister,
        count: u16,
    ) -> Option<Vec<f32>> {
        if let Ok(reading) = self
            .connection
            .read_input_registers(base_register as u16, 2 * count)
            .await
            .unwrap()
        {
            let result = reading
                .chunks_exact(2)
                .map(|chunk| merge_u16_f32(chunk[0], chunk[1]));
            Some(result.collect())
        } else {
            None
        }
    }
}
fn merge_u16_f32(a: u16, b: u16) -> f32 {
    let x: u32 = a as u32 | (b as u32) << 16;
    f32::from_bits(x)
}
#[allow(dead_code)] // We list all for reference when doing multi-reg reads
pub struct PhaseMeasurements {
    pub voltage: f32,
    pub current: f32,
    pub active_power: f32,
    pub apparent_power: f32,
    pub power_factor: f32,
    pub frequency: f32,
}
#[repr(u16)]
#[allow(dead_code)] // We list all for reference when doing multi-reg reads
enum ShellyRegister {
    TotalPower = 1013,
    // Phase A Readings
    PhaseAVoltage = 31020,
    PhaseACurrent = 31022,
    PhaseAActivePower = 31024,
    PhaseAApparentPower = 31026,
    PhaseAPowerFactor = 31028,
    PhaseAFrequency = 31033,
    // Phase B Readings
    PhaseBVoltage = 31040,
    PhaseBCurrent = 31042,
    PhaseBActivePower = 31044,
    PhaseBApparentPower = 31046,
    PhaseBPowerFactor = 31048,
    PhaseBFrequency = 31053,
    // Phase C Readings
    PhaseCVoltage = 31060,
    PhaseCCurrent = 31062,
    PhaseCActivePower = 31064,
    PhaseCApparentPower = 31066,
    PhaseCPowerFactor = 31068,
    PhaseCFrequency = 31073,
    // Total Readings
    TotalCurrent = 31011,
    TotalActivePower = 31013,
    TotalApparentPower = 31015,
    // Note: These total across phases without summing phases first, so if you import on one phase and export on another both will increment
    TotalWhImport = 31162,
    TotalWhExport = 31164,
}
