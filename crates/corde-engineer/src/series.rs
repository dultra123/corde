//! In-memory samples of one lap: named `f64` columns (NaN where the game gave
//! nothing), plus the lap record. This is what every metric works on, and what
//! the unit tests build by hand.

use corde_protocol::{LapRecord, VehicleClass, Weather};
use std::collections::HashMap;

/// Suffixes of the per-wheel channels, protocol order (FL, FR, RL, RR).
pub const WHEEL_SUFFIXES: [&str; 4] = ["_fl", "_fr", "_rl", "_rr"];

/// Name of a per-wheel channel (`brake_temp_c` + wheel 3 → `brake_temp_c_rr`).
pub fn wheel_channel(base: &str, wheel: usize) -> String {
    format!("{base}{}", WHEEL_SUFFIXES[wheel])
}

/// Session-level facts the analysis needs besides the samples.
#[derive(Debug, Clone, Default)]
pub struct SessionMeta {
    pub session_id: String,
    pub track_name: String,
    pub track_length_m: f32,
    pub vehicle_name: String,
    pub vehicle_class: VehicleClass,
    /// Rate of the samples on disk, Hz.
    pub record_rate_hz: f32,
    /// A setup (REST / `.svm`) was captured with the session.
    pub setup_known: bool,
    /// Weather at the start of the session.
    pub weather: Weather,
    /// Every lap of the session (valid or not), for the lap statistics.
    pub all_laps: Vec<LapRecord>,
    /// French notes collected while loading (skipped laps…), copied into
    /// `DataQuality::notes`.
    pub notes: Vec<String>,
}

/// Samples of one lap.
#[derive(Debug, Clone, Default)]
pub struct LapSeries {
    pub lap_number: i32,
    pub record: LapRecord,
    /// Number of samples (length of every column).
    pub n: usize,
    cols: HashMap<String, Vec<f64>>,
}

impl LapSeries {
    pub fn new(lap_number: i32, record: LapRecord) -> Self {
        Self {
            lap_number,
            record,
            n: 0,
            cols: HashMap::new(),
        }
    }

    /// Add (or replace) a column. Every column must have the same length; the
    /// first one fixes `n`, shorter / longer ones are padded with NaN /
    /// truncated so that a malformed file cannot panic the analysis.
    pub fn set(&mut self, name: impl Into<String>, mut values: Vec<f64>) -> &mut Self {
        if self.cols.is_empty() {
            self.n = values.len();
        } else {
            values.resize(self.n, f64::NAN);
        }
        self.cols.insert(name.into(), values);
        self
    }

    /// Replace every column at once with columns of length `n` (resampling).
    pub fn replace_columns(&mut self, n: usize, cols: Vec<(String, Vec<f64>)>) {
        self.cols.clear();
        self.n = n;
        for (name, mut values) in cols {
            values.resize(n, f64::NAN);
            self.cols.insert(name, values);
        }
    }

    /// Column by name, `None` when absent.
    pub fn col(&self, name: &str) -> Option<&[f64]> {
        self.cols.get(name).map(Vec::as_slice)
    }

    /// Per-wheel column.
    pub fn wheel(&self, base: &str, wheel: usize) -> Option<&[f64]> {
        self.col(&wheel_channel(base, wheel))
    }

    pub fn has(&self, name: &str) -> bool {
        self.cols.contains_key(name)
    }

    /// `true` when the column exists and holds at least one finite non-zero value.
    pub fn is_live(&self, name: &str) -> bool {
        self.col(name)
            .map(|c| c.iter().any(|v| v.is_finite() && *v != 0.0))
            .unwrap_or(false)
    }

    /// Lap time axis.
    pub fn t(&self) -> &[f64] {
        self.col("t_s").unwrap_or(&[])
    }

    /// Distance axis.
    pub fn dist(&self) -> &[f64] {
        self.col("lap_dist_m").unwrap_or(&[])
    }

    /// Median sample interval, seconds (0.02 s when it cannot be measured).
    pub fn dt(&self) -> f64 {
        let t = self.t();
        let mut d: Vec<f64> = t
            .windows(2)
            .map(|w| w[1] - w[0])
            .filter(|d| d.is_finite() && *d > 0.0)
            .collect();
        if d.is_empty() {
            return 0.02;
        }
        d.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        d[d.len() / 2]
    }

    /// Column names.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.cols.keys().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_are_padded_to_the_first_length() {
        let mut s = LapSeries::new(1, LapRecord::default());
        s.set("t_s", vec![0.0, 0.1, 0.2]);
        s.set("x", vec![1.0]);
        s.set("y", vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(s.n, 3);
        assert_eq!(s.col("x").unwrap().len(), 3);
        assert!(s.col("x").unwrap()[2].is_nan());
        assert_eq!(s.col("y").unwrap(), &[1.0, 2.0, 3.0]);
        assert!((s.dt() - 0.1).abs() < 1e-9);
        assert!(s.is_live("y"));
        assert!(!s.is_live("nope"));
        assert_eq!(wheel_channel("brake_temp_c", 3), "brake_temp_c_rr");
    }
}
