//! Decoded flight-log model and plotting-oriented downsampling.

use std::collections::BTreeMap;

/// A single named time series (seconds since log start → value).
#[derive(Debug, Clone, Default)]
pub struct Series {
    pub t: Vec<f64>,
    pub v: Vec<f64>,
}

impl Series {
    pub fn push(&mut self, t: f64, v: f64) {
        self.t.push(t);
        self.v.push(v);
    }

    pub fn len(&self) -> usize {
        self.t.len()
    }

    pub fn is_empty(&self) -> bool {
        self.t.is_empty()
    }

    /// Min / max of the values (ignoring non-finite), or `(0, 0)` if empty.
    pub fn min_max(&self) -> (f64, f64) {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for &x in &self.v {
            if x.is_finite() {
                lo = lo.min(x);
                hi = hi.max(x);
            }
        }
        if lo.is_finite() {
            (lo, hi)
        } else {
            (0.0, 0.0)
        }
    }

    /// Downsample to roughly `max_points` while preserving extremes (min/max
    /// per bucket) so vibration spikes survive. Returns `(t, v)`.
    pub fn downsample(&self, max_points: usize) -> (Vec<f64>, Vec<f64>) {
        let n = self.len();
        if n <= max_points || max_points < 4 {
            return (self.t.clone(), self.v.clone());
        }
        let buckets = max_points / 2;
        let mut t_out = Vec::with_capacity(buckets * 2);
        let mut v_out = Vec::with_capacity(buckets * 2);
        for b in 0..buckets {
            let start = b * n / buckets;
            let end = ((b + 1) * n / buckets).max(start + 1).min(n);
            let mut imin = start;
            let mut imax = start;
            for i in start..end {
                if self.v[i] < self.v[imin] {
                    imin = i;
                }
                if self.v[i] > self.v[imax] {
                    imax = i;
                }
            }
            // Emit in time order to keep the line coherent.
            let (a, c) = if imin <= imax {
                (imin, imax)
            } else {
                (imax, imin)
            };
            t_out.push(self.t[a]);
            v_out.push(self.v[a]);
            if c != a {
                t_out.push(self.t[c]);
                v_out.push(self.v[c]);
            }
        }
        (t_out, v_out)
    }
}

/// A span during which the vehicle was in one flight mode.
#[derive(Debug, Clone)]
pub struct ModeSpan {
    pub mode: String,
    pub t0: f64,
    pub t1: f64,
}

/// A logged text message (ULog 'L').
#[derive(Debug, Clone)]
pub struct LogMessage {
    pub t: f64,
    pub level: u8,
    pub text: String,
}

/// A fully decoded flight log.
pub struct Log {
    pub source: String,
    pub name: String,
    pub series: BTreeMap<String, Series>,
    pub modes: Vec<ModeSpan>,
    pub messages: Vec<LogMessage>,
    pub info: BTreeMap<String, String>,
    pub duration_s: f64,
}

impl Log {
    /// Per-series summary used by the browser's field picker.
    pub fn summaries(&self) -> Vec<SeriesSummary> {
        self.series
            .iter()
            .map(|(name, s)| {
                let (min, max) = s.min_max();
                SeriesSummary {
                    name: name.clone(),
                    count: s.len(),
                    min,
                    max,
                }
            })
            .collect()
    }
}

pub struct SeriesSummary {
    pub name: String,
    pub count: usize,
    pub min: f64,
    pub max: f64,
}
