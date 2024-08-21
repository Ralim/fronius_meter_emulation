use shelly_3em_client::Shelly3EMClient;
use smart_meter_emulator::{Readings, SmartMeterEmulator};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio_modbus::server::tcp::{accept_tcp_connection, Server};
mod shelly_3em_client;
mod smart_meter_emulator;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    log::info!("Starting Fronius modbus bridge");
    let socket_addr = "0.0.0.0:5502".parse().unwrap();

    let (emulated_meter, meter_update_handle) = SmartMeterEmulator::new();
    let _shelly = Shelly3EMClient::new("192.168.0.223:502".parse().unwrap(), meter_update_handle);
    server_context(socket_addr, emulated_meter).await;

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
