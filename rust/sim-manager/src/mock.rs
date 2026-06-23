//! Built-in mock flight model.
//!
//! When no real PX4 + Gazebo toolchain is detected (or the panel asks for it),
//! the sidecar flies a small, deterministic-ish autopilot so the control panel
//! and 3D viewport work out of the box — the same philosophy as the synthetic
//! telemetry source in `maestros`. A tiny mission REPL (`arm`, `takeoff`,
//! `goto`, `land`, `rtl`, `orbit`, …) steers it, and injected wind visibly
//! pushes it downwind.
//!
//! Frame: local ENU, metres, origin at home. `yaw` is rotation about +Z with
//! 0 pointing along +X (east), increasing counter-clockwise.

use std::f64::consts::{PI, TAU};

use crate::protocol::{PoseFrame, WindFrame};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlightMode {
    Idle,
    Takeoff,
    Hold,
    Goto,
    Land,
    Rtl,
    Orbit,
}

impl FlightMode {
    pub fn as_str(self) -> &'static str {
        match self {
            FlightMode::Idle => "IDLE",
            FlightMode::Takeoff => "TAKEOFF",
            FlightMode::Hold => "HOLD",
            FlightMode::Goto => "GOTO",
            FlightMode::Land => "LAND",
            FlightMode::Rtl => "RTL",
            FlightMode::Orbit => "ORBIT",
        }
    }
}

// Tuning constants.
const V_MAX: f64 = 7.0; // horizontal speed clamp (m/s)
const KP_XY: f64 = 1.1; // position P gain
const KD_XY: f64 = 1.9; // velocity D gain
const KP_Z: f64 = 1.4;
const KD_Z: f64 = 2.2;
const VZ_MAX: f64 = 3.0;
const WIND_GAIN: f64 = 0.18; // how strongly wind disturbs the vehicle
const ATT_GAIN: f64 = 0.10; // radians of tilt per m/s² of demand
const ATT_MAX: f64 = 0.5; // clamp tilt (~29°)
const YAW_RATE: f64 = 1.6; // rad/s max yaw slew
const ARRIVE: f64 = 0.6; // arrival radius (m)
const ORBIT_R: f64 = 8.0; // default orbit radius (m)
const ORBIT_W: f64 = 0.25; // orbit angular rate (rad/s)

/// The mock vehicle and its little autopilot.
#[derive(Debug, Clone)]
pub struct MockPilot {
    pos: [f64; 3],
    vel: [f64; 3],
    yaw: f64,
    target: [f64; 3],
    mode: FlightMode,
    armed: bool,
    ground: bool, // rover-style vehicle that stays at z=0
    orbit_alt: f64,
    orbit_r: f64,
    phase: f64, // orbit angle accumulator
    roll: f64,
    pitch: f64,
    elapsed: f64,
}

impl MockPilot {
    pub fn new(ground: bool) -> Self {
        Self {
            pos: [0.0; 3],
            vel: [0.0; 3],
            yaw: 0.0,
            target: [0.0; 3],
            mode: FlightMode::Idle,
            armed: false,
            ground,
            orbit_alt: if ground { 0.0 } else { 6.0 },
            orbit_r: ORBIT_R,
            phase: 0.0,
            roll: 0.0,
            pitch: 0.0,
            elapsed: 0.0,
        }
    }

    pub fn mode(&self) -> FlightMode {
        self.mode
    }

    pub fn armed(&self) -> bool {
        self.armed
    }

    pub fn airborne(&self) -> bool {
        self.pos[2] > 0.2
    }

    /// Reset to home and begin an automatic takeoff-then-orbit, so the scene is
    /// lively the instant a sim starts.
    pub fn begin(&mut self) {
        *self = Self::new(self.ground);
        self.armed = true;
        self.mode = FlightMode::Orbit;
    }

    /// Stop and settle on the ground.
    pub fn halt(&mut self) {
        self.mode = FlightMode::Idle;
        self.armed = false;
    }

    /// Advance the simulation by `dt` seconds under the given `wind`.
    pub fn step(&mut self, dt: f64, wind: WindFrame) {
        if dt <= 0.0 {
            return;
        }
        self.elapsed += dt;

        // Resolve the current position target from the active mode.
        match self.mode {
            FlightMode::Orbit => {
                self.phase = (self.phase + ORBIT_W * dt) % TAU;
                self.target = [
                    self.orbit_r * self.phase.cos(),
                    self.orbit_r * self.phase.sin(),
                    self.orbit_alt,
                ];
            }
            FlightMode::Land => self.target[2] = 0.0,
            FlightMode::Rtl => {
                self.target[0] = 0.0;
                self.target[1] = 0.0;
            }
            _ => {}
        }

        // Wind as a disturbance acceleration (blowing toward `direction_deg`,
        // ENU compass: 0° = +Y/north, 90° = +X/east), plus a gust oscillation.
        let dir = wind.direction_deg.to_radians();
        let gust = 1.0 + wind.gust * (self.elapsed * 2.3).sin();
        let wind_acc = [
            wind.speed_mps * dir.sin() * WIND_GAIN * gust,
            wind.speed_mps * dir.cos() * WIND_GAIN * gust,
            0.0,
        ];

        // Horizontal P-D position control toward target.
        let mut acc = [
            KP_XY * (self.target[0] - self.pos[0]) - KD_XY * self.vel[0] + wind_acc[0],
            KP_XY * (self.target[1] - self.pos[1]) - KD_XY * self.vel[1] + wind_acc[1],
            0.0,
        ];

        // Vertical control: only climb when armed; otherwise sink to ground.
        let tz = if self.armed { self.target[2] } else { 0.0 };
        acc[2] = KP_Z * (tz - self.pos[2]) - KD_Z * self.vel[2];
        if self.ground {
            acc[2] = -KP_Z * self.pos[2] - KD_Z * self.vel[2]; // pinned to ground
        }

        // Integrate velocity, clamp, integrate position.
        for (v, a) in self.vel.iter_mut().zip(acc.iter()) {
            *v += *a * dt;
        }
        let hspeed = (self.vel[0].powi(2) + self.vel[1].powi(2)).sqrt();
        if hspeed > V_MAX {
            let k = V_MAX / hspeed;
            self.vel[0] *= k;
            self.vel[1] *= k;
        }
        self.vel[2] = self.vel[2].clamp(-VZ_MAX, VZ_MAX);
        for (p, v) in self.pos.iter_mut().zip(self.vel.iter()) {
            *p += *v * dt;
        }
        if self.pos[2] < 0.0 {
            self.pos[2] = 0.0;
            if self.vel[2] < 0.0 {
                self.vel[2] = 0.0;
            }
        }

        // Attitude: bank/pitch from horizontal acceleration in body axes.
        let (s, c) = self.yaw.sin_cos();
        let acc_fwd = acc[0] * c + acc[1] * s; // along heading
        let acc_lat = -acc[0] * s + acc[1] * c; // to the left
        self.pitch = (-acc_fwd * ATT_GAIN).clamp(-ATT_MAX, ATT_MAX);
        self.roll = (acc_lat * ATT_GAIN).clamp(-ATT_MAX, ATT_MAX);
        if self.ground {
            self.pitch = 0.0;
            self.roll = 0.0;
        }

        // Yaw: face direction of travel when moving meaningfully.
        if hspeed > 0.5 {
            let want = self.vel[1].atan2(self.vel[0]);
            self.yaw = slew_angle(self.yaw, want, YAW_RATE * dt);
        }

        self.update_mode_transitions();
    }

    fn update_mode_transitions(&mut self) {
        let dxy = self.horizontal_dist_to(self.target);
        match self.mode {
            FlightMode::Takeoff => {
                if (self.pos[2] - self.target[2]).abs() < 0.4 {
                    self.mode = FlightMode::Hold;
                }
            }
            FlightMode::Goto => {
                if dxy < ARRIVE && (self.pos[2] - self.target[2]).abs() < 0.5 {
                    self.mode = FlightMode::Hold;
                }
            }
            FlightMode::Rtl => {
                let home_dist = (self.pos[0].powi(2) + self.pos[1].powi(2)).sqrt();
                if home_dist < ARRIVE {
                    self.mode = FlightMode::Land;
                }
            }
            FlightMode::Land => {
                if self.pos[2] < 0.15 {
                    self.pos[2] = 0.0;
                    self.armed = false;
                    self.mode = FlightMode::Idle;
                }
            }
            _ => {}
        }
    }

    fn horizontal_dist_to(&self, p: [f64; 3]) -> f64 {
        ((p[0] - self.pos[0]).powi(2) + (p[1] - self.pos[1]).powi(2)).sqrt()
    }

    /// Snapshot the current pose for the wire frame.
    pub fn pose_frame(&self, t_ms: u128) -> PoseFrame {
        PoseFrame {
            t_ms,
            x: self.pos[0],
            y: self.pos[1],
            z: self.pos[2],
            roll: self.roll,
            pitch: self.pitch,
            yaw: self.yaw,
            vx: self.vel[0],
            vy: self.vel[1],
            vz: self.vel[2],
            airborne: self.airborne(),
            armed: self.armed,
            flight_mode: self.mode.as_str().to_string(),
        }
    }

    /// Interpret a mission REPL line; returns a human-readable acknowledgement.
    pub fn handle_command(&mut self, text: &str) -> String {
        let lower = text.trim().to_lowercase();
        let mut parts = lower.split_whitespace();
        let Some(verb) = parts.next() else {
            return "empty command".to_string();
        };
        let nums: Vec<f64> = parts.filter_map(|p| p.parse::<f64>().ok()).collect();

        match verb {
            "arm" => {
                self.armed = true;
                "armed".to_string()
            }
            "disarm" => {
                self.armed = false;
                self.mode = FlightMode::Idle;
                "disarmed".to_string()
            }
            "takeoff" | "to" => {
                let alt = nums.first().copied().unwrap_or(5.0).max(0.5);
                self.armed = true;
                self.mode = if self.ground {
                    FlightMode::Hold
                } else {
                    FlightMode::Takeoff
                };
                self.target = [
                    self.pos[0],
                    self.pos[1],
                    if self.ground { 0.0 } else { alt },
                ];
                format!("takeoff to {alt:.1} m")
            }
            "hold" | "loiter" | "pause" => {
                self.mode = FlightMode::Hold;
                self.target = self.pos;
                "holding position".to_string()
            }
            "goto" | "go" => {
                if nums.len() < 2 {
                    return "usage: goto <x> <y> [z]".to_string();
                }
                self.armed = true;
                self.mode = FlightMode::Goto;
                let z = nums.get(2).copied().unwrap_or(self.pos[2]).max(0.0);
                self.target = [nums[0], nums[1], if self.ground { 0.0 } else { z }];
                format!(
                    "goto x={:.1} y={:.1} z={:.1}",
                    self.target[0], self.target[1], self.target[2]
                )
            }
            "orbit" | "auto" | "mission" => {
                self.armed = true;
                self.mode = FlightMode::Orbit;
                if let Some(r) = nums.first() {
                    self.orbit_r = r.max(2.0);
                }
                if let Some(a) = nums.get(1) {
                    self.orbit_alt = if self.ground { 0.0 } else { a.max(1.0) };
                }
                format!("orbit r={:.1} m alt={:.1} m", self.orbit_r, self.orbit_alt)
            }
            "land" => {
                self.mode = FlightMode::Land;
                self.target = [self.pos[0], self.pos[1], 0.0];
                "landing".to_string()
            }
            "rtl" | "home" => {
                self.armed = true;
                self.mode = FlightMode::Rtl;
                "return to launch".to_string()
            }
            other => format!("unknown command: {other}"),
        }
    }
}

/// Slew `from` toward `to` (both radians) by at most `max_step`, wrapping.
fn slew_angle(from: f64, to: f64, max_step: f64) -> f64 {
    let mut diff = (to - from) % TAU;
    if diff > PI {
        diff -= TAU;
    } else if diff < -PI {
        diff += TAU;
    }
    let step = diff.clamp(-max_step, max_step);
    let mut out = from + step;
    if out > PI {
        out -= TAU;
    } else if out < -PI {
        out += TAU;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calm() -> WindFrame {
        WindFrame {
            speed_mps: 0.0,
            direction_deg: 0.0,
            gust: 0.0,
        }
    }

    fn run(p: &mut MockPilot, secs: f64, wind: WindFrame) {
        let dt = 0.02;
        let steps = (secs / dt) as usize;
        for _ in 0..steps {
            p.step(dt, wind);
        }
    }

    #[test]
    fn takeoff_gains_altitude() {
        let mut p = MockPilot::new(false);
        assert!(!p.airborne());
        p.handle_command("takeoff 5");
        run(&mut p, 12.0, calm());
        let pose = p.pose_frame(0);
        assert!(pose.z > 3.5, "expected climb, got z={}", pose.z);
        assert!(p.airborne());
    }

    #[test]
    fn goto_moves_horizontally_then_holds() {
        let mut p = MockPilot::new(false);
        p.handle_command("takeoff 5");
        run(&mut p, 8.0, calm());
        p.handle_command("goto 20 0 5");
        run(&mut p, 20.0, calm());
        let pose = p.pose_frame(0);
        assert!((pose.x - 20.0).abs() < 1.5, "x={}", pose.x);
        assert_eq!(p.mode(), FlightMode::Hold);
    }

    #[test]
    fn land_disarms_on_ground() {
        let mut p = MockPilot::new(false);
        p.handle_command("takeoff 6");
        run(&mut p, 10.0, calm());
        p.handle_command("land");
        run(&mut p, 20.0, calm());
        let pose = p.pose_frame(0);
        assert!(pose.z < 0.2, "z={}", pose.z);
        assert!(!p.armed());
        assert_eq!(p.mode(), FlightMode::Idle);
    }

    #[test]
    fn wind_pushes_downwind() {
        // Hold at a point, then compare steady-state offset with/without wind.
        let mut calm_p = MockPilot::new(false);
        calm_p.handle_command("takeoff 5");
        run(&mut calm_p, 10.0, calm());
        calm_p.handle_command("hold");
        run(&mut calm_p, 10.0, calm());

        let mut windy = MockPilot::new(false);
        windy.handle_command("takeoff 5");
        run(&mut windy, 10.0, calm());
        windy.handle_command("hold");
        let east_wind = WindFrame {
            speed_mps: 10.0,
            direction_deg: 90.0, // toward +X (east)
            gust: 0.0,
        };
        run(&mut windy, 12.0, east_wind);

        assert!(
            windy.pose_frame(0).x > calm_p.pose_frame(0).x + 0.5,
            "expected eastward drift: windy.x={} calm.x={}",
            windy.pose_frame(0).x,
            calm_p.pose_frame(0).x
        );
    }

    #[test]
    fn rover_stays_on_ground() {
        let mut p = MockPilot::new(true);
        p.handle_command("takeoff 5");
        p.handle_command("goto 10 0");
        run(&mut p, 15.0, calm());
        let pose = p.pose_frame(0);
        assert!(pose.z.abs() < 0.05, "rover left ground: z={}", pose.z);
        assert!((pose.x - 10.0).abs() < 2.0, "x={}", pose.x);
    }

    #[test]
    fn unknown_command_is_reported() {
        let mut p = MockPilot::new(false);
        assert!(p.handle_command("wiggle").starts_with("unknown command"));
    }
}
