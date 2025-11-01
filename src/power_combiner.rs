use crate::smart_meter_emulator::Readings;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;

/// Thread-safe power data combiner that receives updates from multiple sources
/// and combines them with minimal locking
pub struct PowerCombiner {
    // Atomic storage for power values (using bits representation for f32)
    shelly_power: Arc<AtomicU64>,
    ha_offset: Arc<AtomicU64>,

    // Channel to send combined readings to the meter emulator
    meter_sender: Sender<Readings>,

    // Track if we have received initial values from both sources
    has_shelly_data: Arc<Mutex<bool>>,
    has_ha_data: Arc<Mutex<bool>>,
}

impl PowerCombiner {
    /// Creates a new PowerCombiner
    pub fn new(meter_sender: Sender<Readings>) -> Self {
        Self {
            shelly_power: Arc::new(AtomicU64::new(0.0f32.to_bits() as u64)),
            ha_offset: Arc::new(AtomicU64::new(0.0f32.to_bits() as u64)),
            meter_sender,
            has_shelly_data: Arc::new(Mutex::new(false)),
            has_ha_data: Arc::new(Mutex::new(false)),
        }
    }

    /// Spawns the Shelly power data receiver thread
    pub fn spawn_shelly_receiver(self: Arc<Self>, mut shelly_receiver: Receiver<f32>) {
        let combiner = Arc::clone(&self);
        tokio::spawn(async move {
            println!("Starting Shelly power receiver thread");

            while let Some(power) = shelly_receiver.recv().await {
                combiner.update_shelly_power(power).await;
            }

            println!("Shelly power receiver thread exiting");
        });
    }

    /// Spawns the Home Assistant offset receiver thread
    pub fn spawn_ha_receiver(self: Arc<Self>, mut ha_receiver: Receiver<f32>) {
        let combiner = Arc::clone(&self);
        tokio::spawn(async move {
            println!("Starting Home Assistant offset receiver thread");

            while let Some(offset) = ha_receiver.recv().await {
                combiner.update_ha_offset(offset).await;
            }

            println!("Home Assistant offset receiver thread exiting");
        });
    }

    /// Updates the Shelly power value and triggers recalculation
    async fn update_shelly_power(&self, power: f32) {
        // Store the new power value atomically
        self.shelly_power
            .store(power.to_bits() as u64, Ordering::Relaxed);

        // Mark that we have Shelly data
        {
            let mut has_data = self.has_shelly_data.lock().await;
            *has_data = true;
        }

        // Recalculate and send combined power
        self.send_combined_power().await;
    }

    /// Updates the Home Assistant offset value and triggers recalculation
    async fn update_ha_offset(&self, offset: f32) {
        // Store the new offset value atomically
        self.ha_offset
            .store(offset.to_bits() as u64, Ordering::Relaxed);

        // Mark that we have HA data
        {
            let mut has_data = self.has_ha_data.lock().await;
            *has_data = true;
        }

        // Recalculate and send combined power
        self.send_combined_power().await;
    }

    /// Calculates combined power and sends it to the meter emulator
    async fn send_combined_power(&self) {
        // Check if we have data from both sources
        let has_shelly = *self.has_shelly_data.lock().await;
        let has_ha = *self.has_ha_data.lock().await;

        if !has_shelly || !has_ha {
            // Wait until we have data from both sources
            return;
        }

        // Load current values atomically
        let shelly_bits = self.shelly_power.load(Ordering::Relaxed);
        let ha_bits = self.ha_offset.load(Ordering::Relaxed);

        let shelly_power = f32::from_bits(shelly_bits as u32);
        let ha_offset = f32::from_bits(ha_bits as u32);

        let combined_power = shelly_power + ha_offset;

        println!(
            "Power combination: Shelly {}W + HA offset {}W = {}W",
            shelly_power, ha_offset, combined_power
        );

        // Send the combined readings to the meter emulator
        if let Err(e) = self.send_readings(combined_power).await {
            println!("Failed to send combined power readings: {}", e);
        }
    }

    /// Sends power readings to the meter emulator
    async fn send_readings(
        &self,
        power: f32,
    ) -> Result<(), tokio::sync::mpsc::error::SendError<Readings>> {
        // Send multiple readings as the original code did
        self.meter_sender
            .send(Readings::TotalRealPower(power))
            .await?;
        self.meter_sender
            .send(Readings::ReactivePower(power))
            .await?;
        self.meter_sender
            .send(Readings::NetACCurrent(power))
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_power_combiner_basic_functionality() {
        let (meter_tx, mut meter_rx) = mpsc::channel(32);
        let (shelly_tx, shelly_rx) = mpsc::channel(32);
        let (ha_tx, ha_rx) = mpsc::channel(32);

        let combiner = Arc::new(PowerCombiner::new(meter_tx));

        // Spawn receiver threads
        combiner.clone().spawn_shelly_receiver(shelly_rx);
        combiner.clone().spawn_ha_receiver(ha_rx);

        // Send test data
        shelly_tx.send(100.0).await.unwrap();
        ha_tx.send(25.0).await.unwrap();

        // Should receive combined readings
        let reading1 = tokio::time::timeout(Duration::from_millis(100), meter_rx.recv())
            .await
            .expect("Should receive reading")
            .unwrap();

        match reading1 {
            Readings::TotalRealPower(power) => assert_eq!(power, 125.0),
            _ => panic!("Expected TotalRealPower reading"),
        }
    }

    #[tokio::test]
    async fn test_power_combiner_waits_for_both_sources() {
        let (meter_tx, mut meter_rx) = mpsc::channel(32);
        let (shelly_tx, shelly_rx) = mpsc::channel(32);
        let (ha_tx, ha_rx) = mpsc::channel(32);

        let combiner = Arc::new(PowerCombiner::new(meter_tx));

        combiner.clone().spawn_shelly_receiver(shelly_rx);
        combiner.clone().spawn_ha_receiver(ha_rx);

        // Send only Shelly data
        shelly_tx.send(100.0).await.unwrap();

        // Should not receive any readings yet
        let result = tokio::time::timeout(Duration::from_millis(50), meter_rx.recv()).await;
        assert!(
            result.is_err(),
            "Should not receive readings with only one source"
        );

        // Now send HA data
        ha_tx.send(25.0).await.unwrap();

        // Now should receive readings
        let reading = tokio::time::timeout(Duration::from_millis(100), meter_rx.recv())
            .await
            .expect("Should receive reading after both sources")
            .unwrap();

        match reading {
            Readings::TotalRealPower(power) => assert_eq!(power, 125.0),
            _ => panic!("Expected TotalRealPower reading"),
        }
    }

    #[tokio::test]
    async fn test_power_combiner_negative_values() {
        let (meter_tx, mut meter_rx) = mpsc::channel(32);
        let (shelly_tx, shelly_rx) = mpsc::channel(32);
        let (ha_tx, ha_rx) = mpsc::channel(32);

        let combiner = Arc::new(PowerCombiner::new(meter_tx));

        combiner.clone().spawn_shelly_receiver(shelly_rx);
        combiner.clone().spawn_ha_receiver(ha_rx);

        // Test with negative values
        shelly_tx.send(-50.0).await.unwrap();
        ha_tx.send(75.0).await.unwrap();

        let reading = tokio::time::timeout(Duration::from_millis(100), meter_rx.recv())
            .await
            .unwrap()
            .unwrap();

        match reading {
            Readings::TotalRealPower(power) => assert_eq!(power, 25.0),
            _ => panic!("Expected TotalRealPower reading"),
        }
    }

    #[tokio::test]
    async fn test_atomic_f32_storage() {
        // Test that our atomic f32 storage works correctly
        let atomic = AtomicU64::new(0);

        let test_value = 123.456f32;
        atomic.store(test_value.to_bits() as u64, Ordering::Relaxed);

        let stored_bits = atomic.load(Ordering::Relaxed);
        let retrieved_value = f32::from_bits(stored_bits as u32);

        assert_eq!(retrieved_value, test_value);
    }
}
