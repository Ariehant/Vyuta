//! Live MAVLink telemetry source.
//!
//! Connects to a MAVLink endpoint (UDP/TCP, via a `mavlink` connection string
//! such as `udpin:0.0.0.0:14550`) on a dedicated blocking thread, decodes the
//! messages the dashboard cares about, and folds them into the shared state.
//! The connection auto-reconnects on I/O errors.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mavlink::common::{MavMessage, MavModeFlag};
use mavlink::error::MessageReadError;

use crate::params::{decode_id, ParamStore, SharedConn};
use crate::px4;
use crate::telemetry::TelemetryState;

/// Spawn the MAVLink reader thread.
///
/// `conn_slot` is published once the link is up so the parameter service can
/// send `PARAM_REQUEST_LIST`/`PARAM_SET` over the same connection.
pub fn spawn(
    url: String,
    state: Arc<Mutex<TelemetryState>>,
    params: Arc<Mutex<ParamStore>>,
    conn_slot: SharedConn,
) {
    std::thread::Builder::new()
        .name("mavlink-rx".into())
        .spawn(move || run(url, state, params, conn_slot))
        .expect("failed to spawn mavlink thread");
}

fn run(
    url: String,
    state: Arc<Mutex<TelemetryState>>,
    params: Arc<Mutex<ParamStore>>,
    conn_slot: SharedConn,
) {
    loop {
        tracing::info!(%url, "connecting to MAVLink endpoint");
        match mavlink::connect::<MavMessage>(&url) {
            Ok(conn) => {
                tracing::info!(%url, "MAVLink connected");
                // Share the connection for the parameter write path.
                let conn: Arc<dyn mavlink::MavConnection<MavMessage> + Send + Sync> =
                    Arc::from(conn);
                *conn_slot.lock().expect("conn slot poisoned") = Some(conn.clone());
                loop {
                    match conn.recv() {
                        Ok((header, MavMessage::PARAM_VALUE(d))) => {
                            params.lock().expect("param store poisoned").upsert(
                                decode_id(&d.param_id),
                                d.param_value,
                                d.param_type as u8,
                                d.param_index,
                                d.param_count,
                            );
                            let _ = header;
                        }
                        Ok((header, msg)) => {
                            if matches!(msg, MavMessage::HEARTBEAT(_)) {
                                params
                                    .lock()
                                    .expect("param store poisoned")
                                    .set_target(header.system_id, header.component_id);
                            }
                            let mut s = state.lock().expect("telemetry state mutex poisoned");
                            apply(&mut s, msg);
                        }
                        Err(MessageReadError::Io(e)) => {
                            tracing::warn!(error = %e, "MAVLink I/O error; reconnecting");
                            break;
                        }
                        // Parse errors (CRC mismatch, unknown id, partial) are
                        // expected on a noisy link — skip the frame.
                        Err(_) => continue,
                    }
                }
                *conn_slot.lock().expect("conn slot poisoned") = None;
            }
            Err(e) => {
                tracing::warn!(%url, error = %e, "MAVLink connect failed; retrying in 2s");
            }
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

/// Fold a decoded MAVLink message into the telemetry state.
fn apply(s: &mut TelemetryState, msg: MavMessage) {
    let now = Instant::now();
    s.last_update = Some(now);

    match msg {
        MavMessage::HEARTBEAT(d) => {
            s.last_heartbeat = Some(now);
            s.armed = d
                .base_mode
                .contains(MavModeFlag::MAV_MODE_FLAG_SAFETY_ARMED);
            if d.base_mode
                .contains(MavModeFlag::MAV_MODE_FLAG_CUSTOM_MODE_ENABLED)
            {
                s.mode = px4::mode_string(d.custom_mode);
            } else {
                s.mode = format!("{:?}", d.base_mode);
            }
            s.system_status = format!("{:?}", d.system_status);
        }
        MavMessage::ATTITUDE(d) => {
            s.roll = d.roll as f64;
            s.pitch = d.pitch as f64;
            s.yaw = d.yaw as f64;
        }
        MavMessage::GLOBAL_POSITION_INT(d) => {
            s.lat = Some(d.lat as f64 / 1e7);
            s.lon = Some(d.lon as f64 / 1e7);
            s.alt_m = Some(d.alt as f64 / 1000.0);
            s.rel_alt_m = Some(d.relative_alt as f64 / 1000.0);
            if d.hdg != u16::MAX {
                s.heading_deg = Some(d.hdg as f64 / 100.0);
            }
        }
        MavMessage::SYS_STATUS(d) => {
            if d.voltage_battery != u16::MAX {
                s.battery_v = Some(d.voltage_battery as f64 / 1000.0);
            }
            if d.current_battery != -1 {
                s.current_a = Some(d.current_battery as f64 / 100.0);
            }
            if d.battery_remaining != -1 {
                s.battery_pct = Some(d.battery_remaining as f64);
            }
        }
        MavMessage::BATTERY_STATUS(d) => {
            let pack_mv: u32 = d
                .voltages
                .iter()
                .filter(|&&cell| cell != u16::MAX)
                .map(|&cell| cell as u32)
                .sum();
            if pack_mv > 0 {
                s.battery_v = Some(pack_mv as f64 / 1000.0);
            }
            if d.current_battery != -1 {
                s.current_a = Some(d.current_battery as f64 / 100.0);
            }
            if d.battery_remaining != -1 {
                s.battery_pct = Some(d.battery_remaining as f64);
            }
        }
        MavMessage::VFR_HUD(d) => {
            s.airspeed_mps = Some(d.airspeed as f64);
            s.groundspeed_mps = Some(d.groundspeed as f64);
            s.climb_mps = Some(d.climb as f64);
            s.throttle_pct = Some(d.throttle as f64);
            if s.heading_deg.is_none() {
                s.heading_deg = Some(((d.heading as f64) + 360.0) % 360.0);
            }
        }
        _ => {}
    }
}
