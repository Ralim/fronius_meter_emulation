use axum::{extract::Path, http::StatusCode, response::Json, routing::get, Router};
use serde_json::json;
use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{
    net::TcpListener,
    sync::oneshot,
    time::{sleep, timeout},
};
use tokio_modbus::{
    prelude::*,
    server::{
        tcp::{accept_tcp_connection, Server},
        Service,
    },
};

// Global mutex to serialize tests and prevent environment variable conflicts
static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

// Import the application modules
use fronius_meter_emulation::{
    smart_meter_emulator::SmartMeterEmulator, threaded_data_coordinator::ThreadedDataCoordinator,
};

/// Mock Shelly 3EM Modbus server that simulates power readings
#[derive(Clone)]
struct MockShellyServer {
    power_value: Arc<AtomicU32>, // Store f32 as u32 bits for atomic access
    read_count: Arc<AtomicU32>,
    should_fail: Arc<AtomicBool>,
}

impl MockShellyServer {
    fn new() -> Self {
        Self {
            power_value: Arc::new(AtomicU32::new(0.0f32.to_bits())),
            read_count: Arc::new(AtomicU32::new(0)),
            should_fail: Arc::new(AtomicBool::new(false)),
        }
    }

    fn set_power(&self, power: f32) {
        self.power_value.store(power.to_bits(), Ordering::Relaxed);
    }

    fn set_should_fail(&self, should_fail: bool) {
        self.should_fail.store(should_fail, Ordering::Relaxed);
    }

    fn get_read_count(&self) -> u32 {
        self.read_count.load(Ordering::Relaxed)
    }
}

impl Service for MockShellyServer {
    type Request = Request<'static>;
    type Response = Response;
    type Exception = ExceptionCode;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Exception>> + Send>,
    >;

    fn call(&self, req: Self::Request) -> Self::Future {
        let power_value = self.power_value.clone();
        let read_count = self.read_count.clone();
        let should_fail = self.should_fail.clone();

        Box::pin(async move {
            // Increment read counter
            read_count.fetch_add(1, Ordering::Relaxed);

            if should_fail.load(Ordering::Relaxed) {
                return Err(ExceptionCode::ServerDeviceFailure);
            }

            match req {
                Request::ReadInputRegisters(addr, cnt) if addr == 1013 && cnt == 2 => {
                    let power_bits = power_value.load(Ordering::Relaxed);
                    let power = f32::from_bits(power_bits);

                    // Convert f32 to two u16 values (little-endian format as expected by client)
                    let combined_bits = power.to_bits();
                    let low = (combined_bits & 0xFFFF) as u16;
                    let high = (combined_bits >> 16) as u16;

                    Ok(Response::ReadInputRegisters(vec![low, high]))
                }
                _ => Err(ExceptionCode::IllegalFunction),
            }
        })
    }
}

/// Mock Home Assistant HTTP server
struct MockHomeAssistantServer {
    import_value: Arc<Mutex<f32>>,
    export_value: Arc<Mutex<f32>>,
    request_count: Arc<AtomicU32>,
    should_fail: Arc<AtomicBool>,
}

impl MockHomeAssistantServer {
    fn new() -> Self {
        Self {
            import_value: Arc::new(Mutex::new(0.0)),
            export_value: Arc::new(Mutex::new(0.0)),
            request_count: Arc::new(AtomicU32::new(0)),
            should_fail: Arc::new(AtomicBool::new(false)),
        }
    }

    fn set_import_value(&self, value: f32) {
        *self.import_value.lock().unwrap() = value;
    }

    fn set_export_value(&self, value: f32) {
        *self.export_value.lock().unwrap() = value;
    }

    fn get_request_count(&self) -> u32 {
        self.request_count.load(Ordering::Relaxed)
    }

    fn create_router(self: Arc<Self>) -> Router {
        Router::new().route(
            "/api/states/:entity_id",
            get({
                let server = self.clone();
                move |path: Path<String>| async move {
                    server.request_count.fetch_add(1, Ordering::Relaxed);

                    if server.should_fail.load(Ordering::Relaxed) {
                        return Err(StatusCode::INTERNAL_SERVER_ERROR);
                    }

                    let entity_id = path.0;
                    let value = match entity_id.as_str() {
                        "sensor.test_import" => *server.import_value.lock().unwrap(),
                        "sensor.test_export" => *server.export_value.lock().unwrap(),
                        _ => return Err(StatusCode::NOT_FOUND),
                    };

                    Ok(Json(json!({
                        "entity_id": entity_id,
                        "state": value.to_string(),
                        "last_changed": "2023-01-01T12:00:00Z",
                        "last_reported": "2023-01-01T12:00:00Z",
                        "last_updated": "2023-01-01T12:00:00Z"
                    })))
                }
            }),
        )
    }
}

/// Start mock Shelly Modbus server
async fn start_mock_shelly_server() -> (MockShellyServer, SocketAddr, oneshot::Sender<()>) {
    let mock_server = MockShellyServer::new();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_clone = mock_server.clone();

    tokio::spawn(async move {
        let server = Server::new(listener);
        let new_service = |_socket_addr| Ok(Some(server_clone.clone()));
        let on_connected = |stream, socket_addr| async move {
            accept_tcp_connection(stream, socket_addr, new_service)
        };
        let on_process_error = |err| {
            eprintln!("Mock Shelly server error: {}", err);
        };

        // Run server until shutdown signal
        tokio::select! {
            _ = server.serve(&on_connected, on_process_error) => {},
            _ = shutdown_rx => {
                println!("Mock Shelly server shutting down");
            }
        }
    });

    (mock_server, addr, shutdown_tx)
}

/// Start mock Home Assistant HTTP server
async fn start_mock_ha_server() -> (
    Arc<MockHomeAssistantServer>,
    SocketAddr,
    oneshot::Sender<()>,
) {
    let mock_server = Arc::new(MockHomeAssistantServer::new());
    let app = mock_server.clone().create_router();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    tokio::spawn(async move {
        let server = axum::serve(listener, app);
        tokio::select! {
            _ = server => {},
            _ = shutdown_rx => {
                println!("Mock Home Assistant server shutting down");
            }
        }
    });

    (mock_server, addr, shutdown_tx)
}

/// Test client for reading from our Fronius meter emulator
async fn test_modbus_client(
    meter_addr: SocketAddr,
) -> Result<Vec<u16>, Box<dyn std::error::Error>> {
    let mut ctx = tcp::connect(meter_addr).await?;

    // Read the total real power register (40097-40098 in the SunSpec mapping)
    let result = ctx.read_holding_registers(40097, 2).await?;
    Ok(result?)
}

/// Convert two u16 values back to f32 (matches the application's encoding)
/// SmartMeterEmulator stores f32 as: [high_bits, low_bits]
fn u16_pair_to_f32(high: u16, low: u16) -> f32 {
    let combined: u32 = ((high as u32) << 16) | (low as u32);
    f32::from_bits(combined)
}

#[tokio::test]
async fn test_full_integration() {
    // Serialize tests to prevent environment variable conflicts
    let _guard = TEST_MUTEX.lock().unwrap();
    // Start mock servers
    let (mock_shelly, shelly_addr, _shelly_shutdown) = start_mock_shelly_server().await;
    let (mock_ha, ha_addr, _ha_shutdown) = start_mock_ha_server().await;

    // Set initial values
    mock_shelly.set_power(1000.0); // 1000W from Shelly
    mock_ha.set_import_value(500.0); // 500W import
    mock_ha.set_export_value(200.0); // 200W export
                                     // Expected combined power: 1000 + (500 - 200) = 1300W

    // Set environment variables for the application
    std::env::set_var("SHELLY_MODBUS", shelly_addr.to_string());
    std::env::set_var("HA_URL", format!("http://{}", ha_addr));
    std::env::set_var("HA_TOKEN", "test_token");
    std::env::set_var("HA_EXTRA_IMPORT", "sensor.test_import");
    std::env::set_var("HA_EXTRA_EXPORT", "sensor.test_export");
    std::env::set_var("HA_SMOOTH", "false");

    // Start the application
    let (emulated_meter, meter_update_handle) = SmartMeterEmulator::new();
    ThreadedDataCoordinator::start(meter_update_handle);

    // Start the meter's Modbus TCP server
    let meter_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let meter_addr = meter_listener.local_addr().unwrap();

    tokio::spawn(async move {
        let server = tokio_modbus::server::tcp::Server::new(meter_listener);
        let new_service = |_socket_addr| Ok(Some(emulated_meter.clone()));
        let on_connected = |stream, socket_addr| async move {
            accept_tcp_connection(stream, socket_addr, new_service)
        };
        let on_process_error = |err| {
            eprintln!("Meter server error: {}", err);
        };

        let _ = server.serve(&on_connected, on_process_error).await;
    });

    // Wait for the system to initialize and start reading data
    sleep(Duration::from_secs(2)).await;

    // Test 1: Basic functionality - read combined power value
    let result = timeout(Duration::from_secs(5), test_modbus_client(meter_addr))
        .await
        .expect("Timeout waiting for Modbus response")
        .expect("Failed to read from meter");

    assert_eq!(result.len(), 2, "Expected 2 registers for f32 value");
    let combined_power = u16_pair_to_f32(result[0], result[1]);
    assert!(
        (combined_power - 1300.0).abs() < 0.1,
        "Expected combined power ~1300W, got {}W",
        combined_power
    );

    println!(
        "✅ Test 1 passed: Basic power combination ({}W)",
        combined_power
    );

    // Test 2: Update values and verify changes propagate
    mock_shelly.set_power(2000.0); // Change Shelly to 2000W
    mock_ha.set_import_value(300.0); // Change import to 300W
    mock_ha.set_export_value(100.0); // Change export to 100W
                                     // Expected new combined power: 2000 + (300 - 100) = 2200W

    sleep(Duration::from_secs(2)).await; // Wait for updates to propagate

    let result = timeout(Duration::from_secs(5), test_modbus_client(meter_addr))
        .await
        .expect("Timeout waiting for Modbus response")
        .expect("Failed to read updated values from meter");

    let updated_power = u16_pair_to_f32(result[0], result[1]);
    assert!(
        (updated_power - 2200.0).abs() < 0.1,
        "Expected updated power ~2200W, got {}W",
        updated_power
    );

    println!(
        "✅ Test 2 passed: Value updates propagate ({}W)",
        updated_power
    );

    // Test 3: Negative power values (export > import scenario)
    mock_shelly.set_power(-500.0); // Negative power from Shelly (exporting)
    mock_ha.set_import_value(100.0);
    mock_ha.set_export_value(800.0); // Export more than import
                                     // Expected: -500 + (100 - 800) = -1200W

    sleep(Duration::from_secs(2)).await;

    let result = timeout(Duration::from_secs(5), test_modbus_client(meter_addr))
        .await
        .expect("Timeout waiting for Modbus response")
        .expect("Failed to read negative values from meter");

    let negative_power = u16_pair_to_f32(result[0], result[1]);
    assert!(
        (negative_power - (-1200.0)).abs() < 0.1,
        "Expected negative power ~-1200W, got {}W",
        negative_power
    );

    println!(
        "✅ Test 3 passed: Negative power values ({}W)",
        negative_power
    );

    // Test 4: Verify both servers are being accessed
    let shelly_reads = mock_shelly.get_read_count();
    let ha_requests = mock_ha.get_request_count();

    assert!(
        shelly_reads > 5,
        "Expected multiple Shelly reads, got {}",
        shelly_reads
    );
    assert!(
        ha_requests > 2,
        "Expected multiple HA requests, got {}",
        ha_requests
    );

    println!(
        "✅ Test 4 passed: Both servers accessed (Shelly: {} reads, HA: {} requests)",
        shelly_reads, ha_requests
    );

    // Test 5: Test resilience to temporary failures
    mock_shelly.set_should_fail(true);
    sleep(Duration::from_secs(1)).await; // Let it fail a few times

    mock_shelly.set_should_fail(false);
    mock_shelly.set_power(3000.0);
    mock_ha.set_import_value(400.0);
    mock_ha.set_export_value(50.0);
    // Expected: 3000 + (400 - 50) = 3350W

    sleep(Duration::from_secs(3)).await; // Wait for recovery and updates

    let result = timeout(Duration::from_secs(5), test_modbus_client(meter_addr))
        .await
        .expect("Timeout waiting for Modbus response after failure")
        .expect("Failed to read from meter after recovery");

    let recovery_power = u16_pair_to_f32(result[0], result[1]);
    assert!(
        (recovery_power - 3350.0).abs() < 0.1,
        "Expected recovery power ~3350W, got {}W",
        recovery_power
    );

    println!(
        "✅ Test 5 passed: Recovery from failures ({}W)",
        recovery_power
    );

    // Clean up environment variables
    std::env::remove_var("SHELLY_MODBUS");
    std::env::remove_var("HA_URL");
    std::env::remove_var("HA_TOKEN");
    std::env::remove_var("HA_EXTRA_IMPORT");
    std::env::remove_var("HA_EXTRA_EXPORT");
    std::env::remove_var("HA_SMOOTH");

    println!("🎉 All integration tests passed!");
}

#[tokio::test]
async fn test_missing_home_assistant_config() {
    // Serialize tests to prevent environment variable conflicts
    let _guard = TEST_MUTEX.lock().unwrap();
    // Start only the mock Shelly server
    let (mock_shelly, shelly_addr, _shelly_shutdown) = start_mock_shelly_server().await;
    mock_shelly.set_power(1500.0);

    // Clean all environment variables first
    std::env::remove_var("HA_URL");
    std::env::remove_var("HA_TOKEN");
    std::env::remove_var("HA_EXTRA_IMPORT");
    std::env::remove_var("HA_EXTRA_EXPORT");
    std::env::remove_var("HA_SMOOTH");

    // Set only Shelly config, no HA config
    std::env::set_var("SHELLY_MODBUS", shelly_addr.to_string());

    let (emulated_meter, meter_update_handle) = SmartMeterEmulator::new();
    ThreadedDataCoordinator::start(meter_update_handle);

    let meter_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let meter_addr = meter_listener.local_addr().unwrap();

    tokio::spawn(async move {
        let server = tokio_modbus::server::tcp::Server::new(meter_listener);
        let new_service = |_socket_addr| Ok(Some(emulated_meter.clone()));
        let on_connected = |stream, socket_addr| async move {
            accept_tcp_connection(stream, socket_addr, new_service)
        };
        let on_process_error = |_err| {};
        let _ = server.serve(&on_connected, on_process_error).await;
    });

    sleep(Duration::from_secs(2)).await;

    let result = timeout(Duration::from_secs(5), test_modbus_client(meter_addr))
        .await
        .expect("Timeout waiting for Modbus response")
        .expect("Failed to read from meter");

    let power_only = u16_pair_to_f32(result[0], result[1]);
    // Should be just the Shelly power (1500W) + 0W offset = 1500W
    assert!(
        (power_only - 1500.0).abs() < 0.1,
        "Expected power-only reading ~1500W, got {}W (should have no HA offset)",
        power_only
    );

    println!(
        "✅ Test passed: Works with missing HA config ({}W)",
        power_only
    );

    std::env::remove_var("SHELLY_MODBUS");
}
