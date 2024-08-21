use std::{
    collections::HashMap,
    future,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio_modbus::{prelude::*, server::tcp::Server};

#[derive(Clone)]
pub struct SmartMeterEmulator {
    input_registers: Arc<Mutex<HashMap<u16, u16>>>,
    holding_registers: Arc<Mutex<HashMap<u16, u16>>>,
}
#[derive(Debug)]
pub enum Readings {
    NetACCurrent(f32),
    AveragePhaseVoltage(f32),
    AverageLLVoltage(f32),
    PhaseACurrent(f32),
    PhaseBCurrent(f32),
    PhaseCCurrent(f32),
    PhaseAVoltage(f32),
    PhaseBVoltage(f32),
    PhaseCVoltage(f32),
    PhaseAWatts(f32),
    PhaseBWatts(f32),
    PhaseCWatts(f32),
    PhaseABVoltage(f32),
    PhaseBCVoltage(f32),
    PhaseCAVoltage(f32),
    Frequency(f32),
    TotalRealPower(f32),
    ApparentPower(f32),
    PhaseAVA(f32),
    PhaseBVA(f32),
    PhaseCVA(f32),
    ReactivePower(f32),
    PhaseAVAR(f32),
    PhaseBVAR(f32),
    PhaseCVAR(f32),
    PowerFactorTotal(f32),
    PhaseAPF(f32),
    PhaseBPF(f32),
    PhaseCPF(f32),
}

impl tokio_modbus::server::Service for SmartMeterEmulator {
    type Request = Request<'static>;
    type Response = Response;
    type Exception = Exception;
    type Future = future::Ready<Result<Self::Response, Self::Exception>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let res = match req {
            Request::ReadInputRegisters(addr, cnt) => {
                println!("Register Read for {addr}/{cnt}");
                register_read(&self.input_registers.lock().unwrap(), addr, cnt)
                    .map(Response::ReadInputRegisters)
            }
            Request::ReadHoldingRegisters(addr, cnt) => {
                println!("Holding register Read for {addr}/{cnt}");
                register_read(&self.holding_registers.lock().unwrap(), addr, cnt)
                    .map(Response::ReadHoldingRegisters)
            }

            _ => {
                println!("SERVER: Exception::IllegalFunction - Unimplemented function code in request: {req:?}");
                Err(Exception::IllegalFunction)
            }
        };
        future::ready(res)
    }
}

impl SmartMeterEmulator {
    pub fn new() -> (Self, Sender<Readings>) {
        // Insert some test data as register values.
        let mut input_registers = HashMap::new();
        input_registers.insert(0, 1234);
        input_registers.insert(1, 5678);
        let mut holding_registers = HashMap::new();
        // Seed in all the constant values that are used for the device
        // Well-known value. Uniquely identifies this as a SunSpec Modbus Map
        let sun_spec_values: [u16; 101] = [
            0x5375, 0x6e53, // Sun Spec marker
            1, 65, // Num registers
            70, 114, 111, 110, 105, 117, 115, 0, 0, 0, 0, 0, 0, 0, 0, 0, 83, 109, 97, 114, 116, 32,
            77, 101, 116, 101, 114, 32, 54, 51, 65, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 48, 48, 48, 48, 48, 48, 48, 49, 0, 0, 0, 0, 0, 0, 0, 0,   //Block2
            240, // Modbus address
            213, // Y connected 3 phase (ABCN)
            124, //End of static values
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0,
        ];
        for index in 0..sun_spec_values.len() {
            holding_registers.insert(40000 + index as u16, sun_spec_values[index]);
        }
        // Second set of readings
        let sun_spec_values_2: [u16; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        for index in 0..sun_spec_values_2.len() {
            holding_registers.insert(40129 + index as u16, sun_spec_values_2[index]);
        }

        //Misc filler

        holding_registers.insert(40193, 0);
        holding_registers.insert(40194, 0);
        holding_registers.insert(40195, 0xFFFF); // Terminates the readings blocks
        holding_registers.insert(40196, 0);

        holding_registers.insert(0, 1); // Sunspec model common
        holding_registers.insert(1, 0); // Length of registers
        holding_registers.insert(11, 0);
        holding_registers.insert(12, 0);
        // Not SunSpec, so return 0 to mark us as SunSpec
        holding_registers.insert(768, 0);
        holding_registers.insert(1706, 0);
        holding_registers.insert(50000, 0);
        holding_registers.insert(50001, 0);

        // To handle incoming data updates, we use an MPSC channel for comms
        let (tx, rx) = mpsc::channel(128);
        let holding_registers = Arc::new(Mutex::new(holding_registers));
        let handler_holding_registers = holding_registers.clone();
        tokio::spawn(async move {
            Self::handle_incoming_register_events(rx, handler_holding_registers).await;
        });
        //Return server & channel for readings
        (
            Self {
                input_registers: Arc::new(Mutex::new(input_registers)),
                holding_registers,
            },
            tx,
        )
    }
    async fn handle_incoming_register_events(
        mut events: Receiver<Readings>,
        holding_registers: Arc<Mutex<HashMap<u16, u16>>>,
    ) {
        println!("Starting readinger updates handler task");
        while let Some(reading) = events.recv().await {
            println!("New Reading of {reading:?}");
            match reading {
                Readings::NetACCurrent(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40071, reading)
                }
                Readings::AveragePhaseVoltage(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40079, reading)
                }
                Readings::AverageLLVoltage(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40087, reading)
                }
                Readings::PhaseACurrent(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40073, reading)
                }
                Readings::PhaseBCurrent(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40075, reading)
                }
                Readings::PhaseCCurrent(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40077, reading)
                }
                Readings::PhaseAVoltage(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40081, reading)
                }
                Readings::PhaseBVoltage(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40083, reading)
                }
                Readings::PhaseCVoltage(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40085, reading)
                }
                Readings::PhaseAWatts(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40099, reading)
                }
                Readings::PhaseBWatts(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40101, reading)
                }
                Readings::PhaseCWatts(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40103, reading)
                }
                Readings::PhaseABVoltage(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40089, reading)
                }
                Readings::PhaseBCVoltage(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40091, reading)
                }
                Readings::PhaseCAVoltage(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40093, reading)
                }
                Readings::Frequency(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40095, reading)
                }
                Readings::TotalRealPower(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40097, reading)
                }
                Readings::ApparentPower(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40105, reading)
                }
                Readings::PhaseAVA(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40107, reading)
                }
                Readings::PhaseBVA(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40109, reading)
                }
                Readings::PhaseCVA(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 4011, reading)
                }
                Readings::ReactivePower(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40113, reading)
                }
                Readings::PhaseAVAR(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40115, reading)
                }
                Readings::PhaseBVAR(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40117, reading)
                }
                Readings::PhaseCVAR(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40119, reading)
                }
                Readings::PowerFactorTotal(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40121, reading)
                }
                Readings::PhaseAPF(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40123, reading)
                }
                Readings::PhaseBPF(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40125, reading)
                }
                Readings::PhaseCPF(reading) => {
                    Self::set_holding_reg_f32(&holding_registers, 40127, reading)
                }
            }
        }
        unreachable!();
    }
    fn set_holding_reg(
        holding_registers: &Arc<Mutex<HashMap<u16, u16>>>,
        register: u16,
        value: u16,
    ) {
        let mut regs = holding_registers.lock().expect("Shall unlock registers");
        regs.entry(register).and_modify(|entry| *entry = value);
    }
    fn set_holding_reg_f32(
        holding_registers: &Arc<Mutex<HashMap<u16, u16>>>,
        register_base_number: u16,
        value: f32,
    ) {
        let int_encoding: u32 = value.to_bits();
        Self::set_holding_reg(
            holding_registers,
            register_base_number,
            (int_encoding >> 16) as u16,
        );
        Self::set_holding_reg(
            holding_registers,
            register_base_number + 1,
            (int_encoding & 0xFFFF) as u16,
        );
    }
}

/// Helper function implementing reading registers from a HashMap.
fn register_read(
    registers: &HashMap<u16, u16>,
    addr: u16,
    cnt: u16,
) -> Result<Vec<u16>, Exception> {
    let mut response_values = vec![0; cnt.into()];
    for i in 0..cnt {
        let reg_addr = addr + i;
        if let Some(r) = registers.get(&reg_addr) {
            response_values[i as usize] = *r;
        } else {
            println!(
                "SERVER: Exception::IllegalDataAddress, can't handle read of register {reg_addr}/0x{reg_addr:X}"
            );
            return Err(Exception::IllegalDataAddress);
        }
    }
    println!("Register read for addr:{addr} count:{cnt} returns {response_values:?}");
    Ok(response_values)
}
