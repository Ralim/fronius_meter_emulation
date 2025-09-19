const WINDOW_SIZE: usize = 10;

/// A rolling average calculator that maintains a fixed-size window of f32 values.
#[derive(Debug, Clone)]
pub struct RollingAverage {
    buffer: [f32; WINDOW_SIZE],
    index: usize,
    count: usize,
    sum: f32,
}

impl RollingAverage {
    /// Creates a new RollingAverage with all values initialized to 0.0
    pub fn new() -> Self {
        Self {
            buffer: [0.0; WINDOW_SIZE],
            index: 0,
            count: 0,
            sum: 0.0,
        }
    }

    /// Adds a new value to the rolling average window.
    /// If the window is full, the oldest value is replaced.
    /// Returns the current average after adding the value.
    pub fn add(&mut self, value: f32) -> f32 {
        // Remove the old value from sum if buffer is full
        if self.count == WINDOW_SIZE {
            self.sum -= self.buffer[self.index];
        } else {
            self.count += 1;
        }

        // Add new value
        self.buffer[self.index] = value;
        self.sum += value;

        // Advance index in circular fashion
        self.index = (self.index + 1) % WINDOW_SIZE;

        // Return current average
        self.average()
    }

    /// Returns the current average without adding a new value.
    /// Returns 0.0 if no values have been added yet.
    pub fn average(&self) -> f32 {
        if self.count != WINDOW_SIZE {
            0.0
        } else {
            self.sum / self.count as f32
        }
    }
}

impl Default for RollingAverage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_rolling_average() {
        let avg = RollingAverage::new();
        assert_eq!(avg.average(), 0.0);
    }

    #[test]
    fn test_add_single_value() {
        let mut avg = RollingAverage::new();
        // Before window is full, average returns 0.0
        let result = avg.add(5.0);
        assert_eq!(result, 0.0);
        assert_eq!(avg.average(), 0.0);

        // Fill the window completely with 5.0s
        for _ in 1..WINDOW_SIZE {
            avg.add(5.0);
        }
        assert_eq!(avg.average(), 5.0);
    }

    #[test]
    fn test_add_multiple_values_within_window() {
        let mut avg = RollingAverage::new();
        avg.add(1.0);
        avg.add(2.0);
        let result = avg.add(3.0);
        // Window not full yet, so average returns 0.0
        assert_eq!(result, 0.0);
        assert_eq!(avg.average(), 0.0);
    }

    #[test]
    fn test_fill_window() {
        let mut avg = RollingAverage::new();
        for i in 1..=WINDOW_SIZE {
            avg.add(i as f32);
        }
        // Sum of 1 to WINDOW_SIZE = WINDOW_SIZE * (WINDOW_SIZE + 1) / 2
        let expected = (WINDOW_SIZE * (WINDOW_SIZE + 1) / 2) as f32 / WINDOW_SIZE as f32;
        assert_eq!(avg.average(), expected);
    }

    #[test]
    fn test_rolling_behavior() {
        let mut avg = RollingAverage::new();

        // Fill the window with 1.0s
        for _ in 0..WINDOW_SIZE {
            avg.add(1.0);
        }
        assert_eq!(avg.average(), 1.0);

        // Add one more value, should replace the oldest
        let result = avg.add(2.0);
        // Now we have (WINDOW_SIZE-1) values of 1.0 and 1 value of 2.0
        let expected = ((WINDOW_SIZE - 1) as f32 + 2.0) / WINDOW_SIZE as f32;
        assert_eq!(result, expected);
        assert_eq!(avg.average(), expected);
    }

    #[test]
    fn test_partial_window_returns_zero() {
        let mut avg = RollingAverage::new();
        avg.add(1.0);
        avg.add(2.0);
        avg.add(3.0);

        // Window not full, so average returns 0.0
        assert_eq!(avg.average(), 0.0);
    }

    #[test]
    fn test_precision_with_floating_point() {
        let mut avg = RollingAverage::new();
        // Fill window with pattern of values
        avg.add(1.1);
        avg.add(2.2);
        avg.add(3.3);
        avg.add(1.1);
        avg.add(2.2);
        avg.add(3.3);
        avg.add(1.1);
        avg.add(2.2);
        avg.add(3.3);
        avg.add(0.0);

        let expected = (1.1 + 2.2 + 3.3 + 1.1 + 2.2 + 3.3 + 1.1 + 2.2 + 3.3 + 0.0) / 10.0;
        assert!((avg.average() - expected).abs() < f32::EPSILON);
    }

    #[test]
    fn test_negative_values() {
        let mut avg = RollingAverage::new();
        // Fill window completely with negative and positive values
        avg.add(-1.0);
        avg.add(-2.0);
        avg.add(3.0);
        avg.add(-1.0);
        avg.add(-2.0);
        avg.add(3.0);
        avg.add(-1.0);
        avg.add(-2.0);
        avg.add(3.0);
        avg.add(0.0);

        let expected = (-1.0 - 2.0 + 3.0 - 1.0 - 2.0 + 3.0 - 1.0 - 2.0 + 3.0 + 0.0) / 10.0;
        assert_eq!(avg.average(), expected);
    }

    #[test]
    fn test_large_values() {
        let mut avg = RollingAverage::new();
        avg.add(1000000.0);
        avg.add(2000000.0);
        avg.add(1000000.0);
        avg.add(2000000.0);
        avg.add(1000000.0);
        avg.add(2000000.0);
        avg.add(1000000.0);
        avg.add(2000000.0);
        avg.add(1000000.0);
        avg.add(2000000.0);

        let expected = 1500000.0;
        assert_eq!(avg.average(), expected);
    }

    #[test]
    fn test_zero_values() {
        let mut avg = RollingAverage::new();
        // Fill window with mix of zeros and fives
        avg.add(0.0);
        avg.add(0.0);
        avg.add(0.0);
        avg.add(0.0);
        avg.add(0.0);
        avg.add(5.0);
        avg.add(5.0);
        avg.add(5.0);
        avg.add(5.0);
        avg.add(5.0);

        let expected = (0.0 + 0.0 + 0.0 + 0.0 + 0.0 + 5.0 + 5.0 + 5.0 + 5.0 + 5.0) / 10.0;
        assert_eq!(avg.average(), expected);
    }
}
