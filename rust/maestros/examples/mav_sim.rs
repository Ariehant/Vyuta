//! Minimal MAVLink simulator for testing `maestros` without PX4.
//!
//! Streams HEARTBEAT / ATTITUDE / GLOBAL_POSITION_INT / SYS_STATUS / VFR_HUD at
//! ~20 Hz, and also acts as a tiny **parameter server** (Phase 4): it answers
//! PARAM_REQUEST_LIST / PARAM_REQUEST_READ with PARAM_VALUE and applies
//! PARAM_SET (echoing the new value). Pair it with maestros listening on UDP:
//!
//!   VYUTA_MAVLINK_URL=udpin:127.0.0.1:14550 cargo run --bin maestros
//!   cargo run --example mav_sim -- udpout:127.0.0.1:14550
//!
//! The default target is `udpout:127.0.0.1:14550`.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mavlink::common::{
    MavAutopilot, MavMessage, MavModeFlag, MavParamType, MavState, MavType, ATTITUDE_DATA,
    GLOBAL_POSITION_INT_DATA, HEARTBEAT_DATA, PARAM_VALUE_DATA, SYS_STATUS_DATA, VFR_HUD_DATA,
};
use mavlink::{MavConnection, MavHeader};

type Conn = Arc<dyn MavConnection<MavMessage> + Send + Sync>;

fn px4_custom_mode(main: u8, sub: u8) -> u32 {
    ((main as u32) << 16) | ((sub as u32) << 24)
}

fn encode_id(id: &str) -> [u8; 16] {
    let mut out = [0u8; 16];
    for (dst, src) in out.iter_mut().zip(id.bytes()) {
        *dst = src;
    }
    out
}

fn decode_id(raw: &[u8; 16]) -> String {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    String::from_utf8_lossy(&raw[..end]).into_owned()
}

const HEADER: MavHeader = MavHeader {
    system_id: 1,
    component_id: 1,
    sequence: 0,
};

fn param_value(table: &[(String, f32)], index: usize) -> MavMessage {
    let (id, value) = &table[index];
    MavMessage::PARAM_VALUE(PARAM_VALUE_DATA {
        param_value: *value,
        param_count: table.len() as u16,
        param_index: index as u16,
        param_id: encode_id(id),
        param_type: MavParamType::MAV_PARAM_TYPE_REAL32,
    })
}

/// Reader thread: serve parameter requests and apply sets.
fn run_param_server(conn: Conn) {
    let table = Arc::new(Mutex::new(vec![
        ("MC_ROLLRATE_P".to_string(), 0.15f32),
        ("MC_PITCHRATE_P".to_string(), 0.15),
        ("MPC_XY_P".to_string(), 0.95),
        ("MPC_Z_P".to_string(), 1.0),
        ("BAT1_N_CELLS".to_string(), 4.0),
        ("COM_RC_IN_MODE".to_string(), 0.0),
    ]));

    loop {
        match conn.recv() {
            Ok((_h, MavMessage::PARAM_REQUEST_LIST(_))) => {
                let t = table.lock().unwrap().clone();
                eprintln!("mav_sim: PARAM_REQUEST_LIST -> sending {} params", t.len());
                for i in 0..t.len() {
                    let _ = conn.send(&HEADER, &param_value(&t, i));
                }
            }
            Ok((_h, MavMessage::PARAM_REQUEST_READ(d))) => {
                let id = decode_id(&d.param_id);
                let t = table.lock().unwrap().clone();
                if let Some(i) = t.iter().position(|(k, _)| *k == id) {
                    let _ = conn.send(&HEADER, &param_value(&t, i));
                }
            }
            Ok((_h, MavMessage::PARAM_SET(d))) => {
                let id = decode_id(&d.param_id);
                let mut t = table.lock().unwrap();
                if let Some(i) = t.iter().position(|(k, _)| *k == id) {
                    t[i].1 = d.param_value;
                    eprintln!("mav_sim: PARAM_SET {id} = {}", d.param_value);
                    let msg = param_value(&t, i);
                    drop(t);
                    let _ = conn.send(&HEADER, &msg);
                }
            }
            Ok(_) => {}
            Err(_) => {} // parse/IO hiccups: keep serving
        }
    }
}

fn main() {
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "udpout:127.0.0.1:14550".to_string());

    let conn: Conn =
        Arc::from(mavlink::connect::<MavMessage>(&url).expect("failed to open MAVLink connection"));

    eprintln!("mav_sim: streaming telemetry + serving params to {url}");

    // Parameter server on its own thread (shares the connection).
    let param_conn = conn.clone();
    std::thread::spawn(move || run_param_server(param_conn));

    let start = Instant::now();
    loop {
        let t = start.elapsed().as_secs_f64();
        let t_ms = start.elapsed().as_millis() as u32;

        let heartbeat = MavMessage::HEARTBEAT(HEARTBEAT_DATA {
            custom_mode: px4_custom_mode(3, 0), // POSCTL
            mavtype: MavType::MAV_TYPE_QUADROTOR,
            autopilot: MavAutopilot::MAV_AUTOPILOT_PX4,
            base_mode: MavModeFlag::MAV_MODE_FLAG_SAFETY_ARMED
                | MavModeFlag::MAV_MODE_FLAG_CUSTOM_MODE_ENABLED,
            system_status: MavState::MAV_STATE_ACTIVE,
            mavlink_version: 3,
        });

        let attitude = MavMessage::ATTITUDE(ATTITUDE_DATA {
            time_boot_ms: t_ms,
            roll: (0.35 * (t * 0.7).sin()) as f32,
            pitch: (0.20 * (t * 0.5).cos()) as f32,
            yaw: ((t * 0.2) % std::f64::consts::TAU) as f32,
            ..Default::default()
        });

        let position = MavMessage::GLOBAL_POSITION_INT(GLOBAL_POSITION_INT_DATA {
            time_boot_ms: t_ms,
            lat: ((47.397_742 + 0.0008 * (t * 0.15).sin()) * 1e7) as i32,
            lon: ((8.545_594 + 0.0008 * (t * 0.15).cos()) * 1e7) as i32,
            alt: ((488.0 + 2.0 * (t * 0.4).sin()) * 1000.0) as i32,
            relative_alt: ((10.0 + 2.0 * (t * 0.4).sin()) * 1000.0) as i32,
            hdg: ((((t * 0.2).to_degrees()) % 360.0 + 360.0) % 360.0 * 100.0) as u16,
            ..Default::default()
        });

        let sys_status = MavMessage::SYS_STATUS(SYS_STATUS_DATA {
            voltage_battery: (16800.0 - (t * 10.0).min(4000.0)) as u16,
            current_battery: (1200 + (300.0 * (t * 0.9).sin()) as i16),
            battery_remaining: (100 - (t * 0.5) as i8).max(0),
            ..Default::default()
        });

        let vfr_hud = MavMessage::VFR_HUD(VFR_HUD_DATA {
            airspeed: (5.5 + 2.0 * (t * 0.3).sin().abs()) as f32,
            groundspeed: (5.0 + 2.0 * (t * 0.3).sin().abs()) as f32,
            heading: (((t * 0.2).to_degrees()) % 360.0) as i16,
            throttle: (55.0 + 10.0 * (t * 0.6).sin()) as u16,
            alt: (488.0 + 2.0 * (t * 0.4).sin()) as f32,
            climb: (0.8 * (t * 0.4).cos()) as f32,
        });

        for msg in [heartbeat, attitude, position, sys_status, vfr_hud] {
            if let Err(e) = conn.send(&HEADER, &msg) {
                eprintln!("mav_sim: send error: {e}");
            }
        }

        std::thread::sleep(Duration::from_millis(50));
    }
}
