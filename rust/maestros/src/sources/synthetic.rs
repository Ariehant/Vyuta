//! Synthetic telemetry generator.
//!
//! Active when no MAVLink link is configured, so the dashboard has lively data
//! to render out of the box (and for UI development without a vehicle/SITL).

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::time::interval;

use crate::px4;
use crate::telemetry::TelemetryState;

/// Spawn a task that animates the shared state at ~50 Hz.
pub fn spawn(state: Arc<Mutex<TelemetryState>>) {
    tokio::spawn(async move {
        let start = Instant::now();
        let mut ticker = interval(Duration::from_millis(20));
        loop {
            ticker.tick().await;
            let t = start.elapsed().as_secs_f64();
            let now = Instant::now();

            let mut s = state.lock().expect("telemetry state mutex poisoned");
            s.roll = 0.35 * (t * 0.7).sin();
            s.pitch = 0.20 * (t * 0.5).cos();
            s.yaw = (t * 0.2) % std::f64::consts::TAU;
            s.heading_deg = Some((s.yaw.to_degrees() + 360.0) % 360.0);
            s.lat = Some(47.397_742 + 0.0008 * (t * 0.15).sin());
            s.lon = Some(8.545_594 + 0.0008 * (t * 0.15).cos());
            s.alt_m = Some(488.0 + 2.0 * (t * 0.4).sin());
            s.rel_alt_m = Some(10.0 + 2.0 * (t * 0.4).sin());
            s.battery_v = Some(16.8 - (t * 0.01).min(4.0));
            s.battery_pct = Some((100.0 - t * 0.5).max(0.0));
            s.current_a = Some(12.0 + 3.0 * (t * 0.9).sin());
            s.groundspeed_mps = Some(5.0 + 2.0 * (t * 0.3).sin().abs());
            s.airspeed_mps = Some(5.5 + 2.0 * (t * 0.3).sin().abs());
            s.climb_mps = Some(2.0 * 0.4 * (t * 0.4).cos());
            s.throttle_pct = Some(55.0 + 10.0 * (t * 0.6).sin());
            // Respect an operator arm/disarm override (Phase 7 safety panel);
            // otherwise animate arming for a lively demo.
            if s.manual_arm.is_none() {
                // Start disarmed so the safety panel's pre-flight passes at t=0.
                s.armed = !(t as u64 / 5).is_multiple_of(2);
            }
            s.mode = px4::mode_string(px4::custom_mode(3, 0)); // POSCTL
            s.system_status = "MAV_STATE_ACTIVE".to_string();
            s.last_heartbeat = Some(now);
            s.last_update = Some(now);
        }
    });
}
