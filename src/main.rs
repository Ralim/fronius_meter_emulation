use data_fetcher::DataFetcher;
use smart_meter_emulator::SmartMeterEmulator;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio_modbus::server::tcp::{accept_tcp_connection, Server};
mod data_fetcher;
mod home_assistant;
mod rolling_average;
mod shelly_3em_client;
mod smart_meter_emulator;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("Starting Fronius modbus bridge");
    let socket_addr = "0.0.0.0:5502".parse().unwrap();

    let (emulated_meter, meter_update_handle) = SmartMeterEmulator::new();
    let _data_fetcher = DataFetcher::new(meter_update_handle);

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
