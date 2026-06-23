//! Automatic flight-log review.
//!
//! Runs a handful of heuristics over the decoded [`Log`] (vibration, failsafe
//! activations, battery, mode changes, logged warnings) and reports findings
//! with a severity so the panel can show a triage checklist.

use crate::model::{Log, Series};

#[derive(Debug, Clone)]
pub struct Finding {
    pub id: &'static str,
    pub title: String,
    pub severity: &'static str, // ok | info | warning | critical
    pub detail: String,
}

fn finding(
    id: &'static str,
    severity: &'static str,
    title: impl Into<String>,
    detail: impl Into<String>,
) -> Finding {
    Finding {
        id,
        severity,
        title: title.into(),
        detail: detail.into(),
    }
}

/// Find a series by exact key (first match by instance 0 convention).
fn get<'a>(log: &'a Log, key: &str) -> Option<&'a Series> {
    log.series.get(key)
}

/// Peak RMS of a zero-mean-ish series over `win_s`-second windows.
fn windowed_peak_rms(s: &Series, win_s: f64) -> (f64, f64) {
    if s.is_empty() || win_s <= 0.0 {
        return (0.0, 0.0);
    }
    let mut best_rms = 0.0;
    let mut best_t = 0.0;
    let mut bucket = (s.t[0] / win_s) as i64;
    let mut sumsq = 0.0;
    let mut count = 0u32;
    let mut bstart = s.t[0];
    let flush = |sumsq: f64, count: u32, bstart: f64, best_rms: &mut f64, best_t: &mut f64| {
        if count > 0 {
            let rms = (sumsq / count as f64).sqrt();
            if rms > *best_rms {
                *best_rms = rms;
                *best_t = bstart;
            }
        }
    };
    for i in 0..s.len() {
        let b = (s.t[i] / win_s) as i64;
        if b != bucket {
            flush(sumsq, count, bstart, &mut best_rms, &mut best_t);
            bucket = b;
            sumsq = 0.0;
            count = 0;
            bstart = s.t[i];
        }
        sumsq += s.v[i] * s.v[i];
        count += 1;
    }
    flush(sumsq, count, bstart, &mut best_rms, &mut best_t);
    (best_rms, best_t)
}

/// Spans where a 0/1 series is active; returns (count, total_seconds, first_t).
fn active_spans(s: &Series) -> (u32, f64, Option<f64>) {
    let mut count = 0u32;
    let mut total = 0.0;
    let mut first = None;
    let mut in_span = false;
    let mut span_start = 0.0;
    let mut prev_t = s.t.first().copied().unwrap_or(0.0);
    for i in 0..s.len() {
        let active = s.v[i] >= 0.5;
        if active && !in_span {
            in_span = true;
            span_start = s.t[i];
            count += 1;
            first.get_or_insert(s.t[i]);
        } else if !active && in_span {
            in_span = false;
            total += s.t[i] - span_start;
        }
        prev_t = s.t[i];
    }
    if in_span {
        total += prev_t - span_start;
    }
    (count, total, first)
}

pub fn review(log: &Log) -> Vec<Finding> {
    let mut out = Vec::new();

    // --- Duration / modes ---------------------------------------------------
    out.push(finding(
        "duration",
        "info",
        "Flight duration",
        format!(
            "{:.1} s, {} mode change(s)",
            log.duration_s,
            log.modes.len().saturating_sub(1)
        ),
    ));

    // --- Vibration ----------------------------------------------------------
    let ax = get(log, "sensor_combined[0].accelerometer_m_s2[0]");
    let ay = get(log, "sensor_combined[0].accelerometer_m_s2[1]");
    if let (Some(ax), Some(ay)) = (ax, ay) {
        let (rx, tx) = windowed_peak_rms(ax, 0.5);
        let (ry, _ty) = windowed_peak_rms(ay, 0.5);
        let peak = rx.max(ry);
        let (sev, note) = match peak {
            p if p >= 5.0 => ("critical", "excessive"),
            p if p >= 3.0 => ("warning", "elevated"),
            _ => ("ok", "nominal"),
        };
        out.push(finding(
            "vibration",
            sev,
            "Vibration (horizontal accel)",
            format!("peak RMS {peak:.2} m/s² ({note}) near t={tx:.1}s"),
        ));
    } else {
        out.push(finding(
            "vibration",
            "info",
            "Vibration",
            "no accelerometer data",
        ));
    }

    // --- Failsafe -----------------------------------------------------------
    if let Some(fs) = get(log, "vehicle_status[0].failsafe") {
        let (count, total, first) = active_spans(fs);
        if count > 0 {
            out.push(finding(
                "failsafe",
                "warning",
                "Failsafe activated",
                format!(
                    "{count} activation(s), {total:.1}s total, first at t={:.1}s",
                    first.unwrap_or(0.0)
                ),
            ));
        } else {
            out.push(finding(
                "failsafe",
                "ok",
                "Failsafe",
                "no failsafe activations",
            ));
        }
    }

    // --- Battery ------------------------------------------------------------
    if let Some(rem) = get(log, "battery_status[0].remaining") {
        let (min, _) = rem.min_max();
        let pct = min * 100.0;
        let sev = match pct {
            p if p < 15.0 => "critical",
            p if p < 25.0 => "warning",
            _ => "ok",
        };
        let mut detail = format!("min remaining {pct:.0}%");
        if let Some(volt) = get(log, "battery_status[0].voltage_v") {
            let (vmin, _) = volt.min_max();
            detail.push_str(&format!(", min voltage {vmin:.2} V"));
        }
        out.push(finding("battery", sev, "Battery", detail));
    }

    // --- Altitude -----------------------------------------------------------
    if let Some(z) = get(log, "vehicle_local_position[0].z") {
        let (min, _) = z.min_max(); // NED: most-negative z = highest altitude
        out.push(finding(
            "altitude",
            "info",
            "Max altitude",
            format!("{:.1} m", -min),
        ));
    }

    // --- Logged warnings ----------------------------------------------------
    let warns = log.messages.iter().filter(|m| m.level <= 4).count();
    if warns > 0 {
        out.push(finding(
            "log_messages",
            "info",
            "Logged warnings/errors",
            format!("{warns} message(s) at warning level or above"),
        ));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::synthetic_log;

    #[test]
    fn synthetic_review_flags_vibration_failsafe_and_battery() {
        let log = synthetic_log();
        let findings = review(&log);
        let by = |id: &str| findings.iter().find(|f| f.id == id);

        let vib = by("vibration").expect("vibration finding");
        assert!(
            vib.severity == "warning" || vib.severity == "critical",
            "vib sev {}",
            vib.severity
        );

        let fs = by("failsafe").expect("failsafe finding");
        assert_eq!(fs.severity, "warning");

        let bat = by("battery").expect("battery finding");
        assert_eq!(bat.severity, "warning"); // ends ~18%
    }
}
