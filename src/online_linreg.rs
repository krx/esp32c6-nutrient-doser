use serde::{Deserialize, Serialize};

// References:
// - https://www3.stat.sinica.edu.tw/statistica/oldpdf/A5n124.pdf
// - https://scaron.info/blog/simple-linear-regression-with-online-updates.html

#[derive(Debug, Serialize, Deserialize, Default, Clone, Copy)]
pub struct OnlineLinearRegression {
    n: usize,
    sum_x: f64,
    sum_y: f64,
    sum_xx: f64,
    sum_xy: f64,
    intercept: f64,
    slope: f64,
}

impl OnlineLinearRegression {
    pub fn new() -> Self {
        Self {
            // From some manual testing, these should be pretty close to start with
            // once calibration data is available, these defaults will be overwritten
            intercept: 4000.0,
            slope: 312.5,
            ..Default::default()
        }
    }

    // Update the model with a new data point
    // x: mL dispensed
    // y: steps taken
    pub fn update(&mut self, x: f64, y: f64) {
        self.n += 1;
        self.sum_x += x;
        self.sum_y += y;
        self.sum_xx += x * x;
        self.sum_xy += x * y;

        let det = (self.n as f64) * self.sum_xx - self.sum_x * self.sum_x;
        if det != 0.0 {
            self.intercept = (self.sum_xx * self.sum_y - self.sum_xy * self.sum_x) / det;
            self.slope = ((self.n as f64) * self.sum_xy - self.sum_x * self.sum_y) / det;
        }
    }

    pub fn predict(&self, x: f64) -> f64 {
        self.intercept + self.slope * x
    }

    pub fn slope(&self) -> f64 {
        self.slope
    }

    pub fn intercept(&self) -> f64 {
        self.intercept
    }
}
