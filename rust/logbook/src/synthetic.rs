//! Synthetic flight-log generator.
//!
//! Produces a small but *valid* ULog byte buffer for a ~30 s flight (takeoff →
//! POSCTL → a vibration burst → an RC-loss failsafe → RTL → land), then parses
//! it with the real [`crate::ulog`] parser. This gives the log browser
//! something to show with no `.ulg` on disk — the same out-of-the-box
//! philosophy as the synthetic telemetry source — and exercises the parser
//! itself on every run.

use crate::model::Log;
use crate::ulog;

const T0_US: u64 = 1_000_000;

// --- little-endian writers --------------------------------------------------

fn put_msg(out: &mut Vec<u8>, mtype: u8, payload: &[u8]) {
    out.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    out.push(mtype);
    out.extend_from_slice(payload);
}

fn put_format(out: &mut Vec<u8>, def: &str) {
    put_msg(out, b'F', def.as_bytes());
}

fn put_add(out: &mut Vec<u8>, multi_id: u8, msg_id: u16, name: &str) {
    let mut p = vec![multi_id];
    p.extend_from_slice(&msg_id.to_le_bytes());
    p.extend_from_slice(name.as_bytes());
    put_msg(out, b'A', &p);
}

fn put_logged(out: &mut Vec<u8>, level: u8, t_us: u64, text: &str) {
    let mut p = vec![level];
    p.extend_from_slice(&t_us.to_le_bytes());
    p.extend_from_slice(text.as_bytes());
    put_msg(out, b'L', &p);
}

struct Rec(Vec<u8>);
impl Rec {
    fn new(msg_id: u16, t_us: u64) -> Self {
        let mut v = Vec::new();
        v.extend_from_slice(&msg_id.to_le_bytes());
        v.extend_from_slice(&t_us.to_le_bytes());
        Rec(v)
    }
    fn f32(mut self, x: f32) -> Self {
        self.0.extend_from_slice(&x.to_le_bytes());
        self
    }
    fn u8(mut self, x: u8) -> Self {
        self.0.push(x);
        self
    }
    fn emit(self, out: &mut Vec<u8>) {
        put_msg(out, b'D', &self.0);
    }
}

/// nav_state for a given time (seconds).
fn nav_state(t: f64) -> u8 {
    match t {
        _ if t < 3.0 => 17, // AUTO_TAKEOFF
        _ if t < 12.0 => 2, // POSCTL
        _ if t < 22.0 => 5, // AUTO_RTL
        _ => 18,            // AUTO_LAND
    }
}

fn failsafe(t: f64) -> u8 {
    (12.0..13.5).contains(&t) as u8
}

/// Build the synthetic ULog byte buffer.
pub fn synthetic_ulog_bytes() -> Vec<u8> {
    let mut out = Vec::new();

    // Header: magic, version, timestamp.
    out.extend_from_slice(&[0x55, 0x4C, 0x6F, 0x67, 0x01, 0x12, 0x35]);
    out.push(0); // version
    out.extend_from_slice(&T0_US.to_le_bytes());

    // Flag bits ('B'): compat[8], incompat[8], appended_offsets[3*u64].
    put_msg(&mut out, b'B', &[0u8; 40]);

    // Info string: key = "char[N] sys_name", value = "PX4".
    let key = format!("char[{}] sys_name", "PX4".len());
    let mut ip = vec![key.len() as u8];
    ip.extend_from_slice(key.as_bytes());
    ip.extend_from_slice(b"PX4");
    put_msg(&mut out, b'I', &ip);

    // Formats.
    put_format(
        &mut out,
        "sensor_combined:uint64_t timestamp;float[3] accelerometer_m_s2;float[3] gyro_rad;",
    );
    put_format(&mut out, "vehicle_attitude:uint64_t timestamp;float[4] q;");
    put_format(
        &mut out,
        "vehicle_local_position:uint64_t timestamp;float x;float y;float z;float vx;float vy;float vz;",
    );
    put_format(
        &mut out,
        "battery_status:uint64_t timestamp;float voltage_v;float remaining;",
    );
    put_format(
        &mut out,
        "vehicle_status:uint64_t timestamp;uint8_t nav_state;uint8_t arming_state;uint8_t failsafe;",
    );

    // Subscriptions (instance 0).
    put_add(&mut out, 0, 0, "sensor_combined");
    put_add(&mut out, 0, 1, "vehicle_attitude");
    put_add(&mut out, 0, 2, "vehicle_local_position");
    put_add(&mut out, 0, 3, "battery_status");
    put_add(&mut out, 0, 4, "vehicle_status");

    put_logged(&mut out, 6, T0_US + 100_000, "[logger] logging started");

    let dur = 30.0_f64;
    let us = |t: f64| T0_US + (t * 1e6) as u64;

    // sensor_combined @ 50 Hz (with a vibration burst 14..16 s).
    let mut t = 0.0;
    while t < dur {
        let burst = if (14.0..16.0).contains(&t) { 5.0 } else { 0.0 };
        let nx = 0.15 * (t * 3.0).sin() + burst * (t * 220.0).sin();
        let ny = 0.15 * (t * 2.3).cos() + burst * (t * 205.0).cos();
        let nz = -9.81 + 0.1 * (t * 4.0).sin() + burst * 0.5 * (t * 230.0).sin();
        Rec::new(0, us(t))
            .f32(nx as f32)
            .f32(ny as f32)
            .f32(nz as f32)
            .f32((0.02 * (t * 1.5).sin()) as f32)
            .f32((0.02 * (t * 1.7).cos()) as f32)
            .f32((0.01 * (t * 0.9).sin()) as f32)
            .emit(&mut out);
        t += 0.02;
    }

    // vehicle_attitude @ 50 Hz (gentle wobble quaternion).
    let mut t = 0.0;
    while t < dur {
        let roll = 0.2 * (t * 0.7).sin();
        let (s, c) = (roll / 2.0).sin_cos();
        Rec::new(1, us(t))
            .f32(c as f32)
            .f32(s as f32)
            .f32(0.0)
            .f32(0.0)
            .emit(&mut out);
        t += 0.02;
    }

    // vehicle_local_position @ 20 Hz (climb, orbit, descend; NED z = -alt).
    let mut t = 0.0;
    while t < dur {
        let alt = if t < 3.0 {
            (t / 3.0) * 10.0
        } else if t < 22.0 {
            10.0
        } else {
            (10.0 * (1.0 - (t - 22.0) / 8.0)).max(0.0)
        };
        let x = 8.0 * (t * 0.2).cos();
        let y = 8.0 * (t * 0.2).sin();
        Rec::new(2, us(t))
            .f32(x as f32)
            .f32(y as f32)
            .f32((-alt) as f32)
            .f32((-1.6 * (t * 0.2).sin()) as f32)
            .f32((1.6 * (t * 0.2).cos()) as f32)
            .f32(0.0)
            .emit(&mut out);
        t += 0.05;
    }

    // battery_status @ 2 Hz (voltage + remaining decay).
    let mut t = 0.0;
    while t < dur {
        let frac = t / dur;
        let voltage = 16.8 - 2.2 * frac;
        let remaining = 1.0 - 0.82 * frac; // ends ~0.18
        Rec::new(3, us(t))
            .f32(voltage as f32)
            .f32(remaining as f32)
            .emit(&mut out);
        t += 0.5;
    }

    // vehicle_status @ 5 Hz (mode timeline + failsafe window).
    let mut t = 0.0;
    while t < dur {
        let arming = if (1.0..28.0).contains(&t) { 2 } else { 1 };
        Rec::new(4, us(t))
            .u8(nav_state(t))
            .u8(arming)
            .u8(failsafe(t))
            .emit(&mut out);
        t += 0.2;
    }

    put_logged(&mut out, 4, us(12.0), "Failsafe enabled: RC signal lost");
    put_logged(&mut out, 6, us(22.0), "Landing detected");

    out
}

/// Build a synthetic [`Log`] by parsing the generated bytes.
pub fn synthetic_log() -> Log {
    let bytes = synthetic_ulog_bytes();
    ulog::parse(&bytes, "synthetic", "synthetic-flight.ulg").expect("synthetic ULog must parse")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_parses_with_expected_series_and_modes() {
        let log = synthetic_log();
        assert!(log.duration_s > 28.0 && log.duration_s < 31.0);
        assert!(log
            .series
            .contains_key("sensor_combined[0].accelerometer_m_s2[0]"));
        assert!(log.series.contains_key("battery_status[0].remaining"));
        assert!(log.series.contains_key("vehicle_status[0].nav_state"));
        // Modes should include takeoff, posctl, rtl, land.
        let modes: Vec<_> = log.modes.iter().map(|m| m.mode.as_str()).collect();
        assert!(modes.contains(&"AUTO_TAKEOFF"));
        assert!(modes.contains(&"POSCTL"));
        assert!(modes.contains(&"AUTO_RTL"));
        assert!(modes.contains(&"AUTO_LAND"));
        // Logged strings captured.
        assert!(log.messages.iter().any(|m| m.text.contains("Failsafe")));
    }

    #[test]
    fn accel_array_is_expanded_to_three_axes() {
        let log = synthetic_log();
        for ax in 0..3 {
            assert!(log
                .series
                .contains_key(&format!("sensor_combined[0].accelerometer_m_s2[{ax}]")));
        }
    }
}
