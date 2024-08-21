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

    tokio::select! {
        _ = server_context(socket_addr) => unreachable!(),
        // _ = client_context(socket_addr) => println!("Exiting"),
    }

    Ok(())
}

async fn server_context(socket_addr: SocketAddr) -> anyhow::Result<()> {
    println!("Starting up server on {socket_addr}");
    let listener = TcpListener::bind(socket_addr).await?;
    let server = Server::new(listener);
    let (emulated_meter, meter_update_handle) = SmartMeterEmulator::new();
    meter_update_handle
        .send(Readings::AveragePhaseVoltage(230.0))
        .await;
    meter_update_handle
        .send(Readings::AveragePhaseVoltage(230.0))
        .await;
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

// async fn client_context(socket_addr: SocketAddr) {
//     tokio::join!(
//         async {
//             // Give the server some time for starting up
//             tokio::time::sleep(Duration::from_secs(1)).await;

//             println!("CLIENT: Connecting client...");
//             let mut ctx = tcp::connect(socket_addr).await.unwrap();

//             println!("CLIENT: Reading 2 input registers...");
//             let response = ctx.read_input_registers(0x00, 2).await.unwrap();
//             println!("CLIENT: The result is '{response:?}'");
//             assert_eq!(response.unwrap(), vec![1234, 5678]);

//             println!("CLIENT: Writing 2 holding registers...");
//             ctx.write_multiple_registers(0x01, &[7777, 8888])
//                 .await
//                 .unwrap()
//                 .unwrap();

//             // Read back a block including the two registers we wrote.
//             println!("CLIENT: Reading 4 holding registers...");
//             let response = ctx.read_holding_registers(0x00, 4).await.unwrap();
//             println!("CLIENT: The result is '{response:?}'");
//             assert_eq!(response.unwrap(), vec![10, 7777, 8888, 40]);

//             // Now we try to read with an invalid register address.
//             // This should return a Modbus exception response with the code
//             // IllegalDataAddress.
//             println!("CLIENT: Reading nonexistent holding register address... (should return IllegalDataAddress)");
//             let response = ctx.read_holding_registers(0x100, 1).await.unwrap();
//             println!("CLIENT: The result is '{response:?}'");
//             assert!(matches!(response, Err(Exception::IllegalDataAddress)));

//             println!("CLIENT: Done.")
//         },
//         tokio::time::sleep(Duration::from_secs(5))
//     );
// }
