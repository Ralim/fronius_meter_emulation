use smart_meter_emulator::SmartMeterEmulator;
use std::net::SocketAddr;
use threaded_data_coordinator::ThreadedDataCoordinator;
use tokio::net::TcpListener;
use tokio_modbus::server::tcp::{accept_tcp_connection, Server};

mod home_assistant;
mod home_assistant_reader;
mod power_combiner;
mod rolling_average;
mod shelly_reader;
mod smart_meter_emulator;
mod threaded_data_coordinator;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("Starting Fronius modbus bridge");
    let socket_addr = "0.0.0.0:5502".parse().unwrap();

    let (emulated_meter, meter_update_handle) = SmartMeterEmulator::new();

    // Start the threaded data coordinator which manages isolated threads for:
    // 1. Shelly power meter reading (Modbus)
    // 2. Home Assistant offset reading (HTTP)
    // 3. Power data combination with minimal locking
    let _data_coordinator = ThreadedDataCoordinator::new(meter_update_handle);

    //Start fake meter
    server_context(socket_addr, emulated_meter)
        .await
        .expect("Should never exit fake meter");

    Ok(())
}

async fn server_context(
    socket_addr: SocketAddr,
    emulated_meter: SmartMeterEmulator,
) -> anyhow::Result<()> {
    println!("Starting up server on {socket_addr}");
    let listener = TcpListener::bind(socket_addr).await?;
    let server = Server::new(listener);
    let new_service = |_socket_addr| Ok(Some(emulated_meter.clone()));
    let on_connected = |stream, socket_addr| async move {
        accept_tcp_connection(stream, socket_addr, new_service)
    };
    let on_process_error = |err| {
        eprintln!("{err}");
    };
    server.serve(&on_connected, on_process_error).await?;
    Ok(())
}
