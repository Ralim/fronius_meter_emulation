use data_fetcher::DataFetcher;
use smart_meter_emulator::SmartMeterEmulator;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio_modbus::server::tcp::{accept_tcp_connection, Server};
mod data_fetcher;
mod home_assistant;
mod shelly_3em_client;
mod smart_meter_emulator;
use axum::{routing::get, Router};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("Starting Fronius modbus bridge");
    let socket_addr = "0.0.0.0:5502".parse().unwrap();

    let (emulated_meter, meter_update_handle) = SmartMeterEmulator::new();
    let _data_fetcher = DataFetcher::new(meter_update_handle);
    // Spawn health-check server
    tokio::spawn(async move {
        healthcheck_server().await;
    });
    //Start fake meter
    server_context(socket_addr, emulated_meter)
        .await
        .expect("Should never exit fake meter");

    Ok(())
}

async fn healthcheck_server() {
    let app = Router::new().route("/", get(healthcheck));

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
async fn healthcheck() -> &'static str {
    "OK"
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
