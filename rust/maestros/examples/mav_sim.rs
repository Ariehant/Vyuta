//! Minimal MAVLink telemetry simulator for testing `maestros` without PX4.
//!
//! Sends HEARTBEAT / ATTITUDE / GLOBAL_POSITION_INT / SYS_STATUS / VFR_HUD at
//! ~20 Hz to a MAVLink endpoint. Pair it with maestros listening on UDP:
//!
//!   VYUTA_MAVLINK_URL=udpin:127.0.0.1:14550 cargo run --bin maestros
//!   cargo run --example mav_sim -- udpout:127.0.0.1:14550
//!
//! The default target is `udpout:127.0.0.1:14550`.

use std::time::{Duration, Instant};

use mavlink::common::{
    MavAutopilot, MavMessage, MavModeFlag, MavState, MavType, ATTITUDE_DATA,
    GLOBAL_POSITION_INT_DATA, HEARTBEAT_DATA, SYS_STATUS_DATA, VFR_HUD_DATA,
};

fn px4_custom_mode(main: u8, sub: u8) -> u32 {
    ((main as u32) << 16) | ((sub as u32) << 24)
}

fn main() {
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "udpout:127.0.0.1:14550".to_string());

    let conn = mavlink::connect::<MavMessage>(&url).expect("failed to open MAVLink connection");
    let header = mavlink::MavHeader {
        system_id: 1,
        component_id: 1,
        sequence: 0,
    };

    eprintln!("mav_sim: streaming telemetry to {url}");
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
            if let Err(e) = conn.send(&header, &msg) {
                eprintln!("mav_sim: send error: {e}");
            }
        }

        std::thread::sleep(Duration::from_millis(50));
    }
}
