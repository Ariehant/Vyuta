//! Pre-flight safety checks (Phase 7).
//!
//! Evaluates a checklist against the live telemetry and the parameter store so
//! the safety panel can gate the **Arm** button. Each item carries a pass/fail
//! and a human detail; the overall result is the AND of all checks.

use crate::params::ParamStore;
use crate::telemetry::TelemetryState;

/// Battery percentage below which arming is blocked.
pub const MIN_BATTERY_PCT: f64 = 30.0;
/// Maximum level attitude (radians) to consider the vehicle "level".
pub const MAX_TILT_RAD: f64 = 0.52; // ~30°

#[derive(Debug, Clone)]
pub struct CheckItem {
    pub id: &'static str,
    pub label: &'static str,
    pub pass: bool,
    pub detail: String,
}

/// Run the pre-flight checklist. Returns `(all_ok, items)`.
pub fn evaluate(
    state: &TelemetryState,
    params: &ParamStore,
    link_ok: bool,
) -> (bool, Vec<CheckItem>) {
    let mut items = Vec::new();

    items.push(CheckItem {
        id: "link",
        label: "Telemetry link",
        pass: link_ok,
        detail: if link_ok {
            "heartbeat healthy".into()
        } else {
            "no recent heartbeat".into()
        },
    });

    let (batt_pass, batt_detail) = match state.battery_pct {
        Some(p) if p >= MIN_BATTERY_PCT => (true, format!("{p:.0}%")),
        Some(p) => (false, format!("{p:.0}% < {MIN_BATTERY_PCT:.0}% minimum")),
        None => (false, "no battery telemetry".into()),
    };
    items.push(CheckItem {
        id: "battery",
        label: "Battery",
        pass: batt_pass,
        detail: batt_detail,
    });

    let gps_pass = state.lat.is_some() && state.lon.is_some();
    items.push(CheckItem {
        id: "gps",
        label: "GPS / position",
        pass: gps_pass,
        detail: if gps_pass {
            format!("{:.5}, {:.5}", state.lat.unwrap(), state.lon.unwrap())
        } else {
            "no position fix".into()
        },
    });

    let tilt = state.roll.abs().max(state.pitch.abs());
    let level_pass = tilt <= MAX_TILT_RAD;
    items.push(CheckItem {
        id: "attitude",
        label: "Attitude level",
        pass: level_pass,
        detail: format!("tilt {:.0}°", tilt.to_degrees()),
    });

    let params_pass = params.received() > 0;
    items.push(CheckItem {
        id: "params",
        label: "Parameters synced",
        pass: params_pass,
        detail: format!("{} parameters", params.received()),
    });

    let not_armed = !state.manual_arm.unwrap_or(state.armed);
    items.push(CheckItem {
        id: "disarmed",
        label: "Currently disarmed",
        pass: not_armed,
        detail: if not_armed {
            "ready".into()
        } else {
            "already armed".into()
        },
    });

    let ok = items.iter().all(|i| i.pass);
    (ok, items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::{Source, TelemetryState};

    fn good_state() -> TelemetryState {
        let mut s = TelemetryState::new(Source::Synthetic);
        s.battery_pct = Some(80.0);
        s.lat = Some(47.4);
        s.lon = Some(8.5);
        s.roll = 0.0;
        s.pitch = 0.0;
        s.manual_arm = Some(false);
        s
    }

    #[test]
    fn passes_when_all_good() {
        let mut store = ParamStore::new();
        store.upsert("A".into(), 1.0, 9, 0, 1);
        let (ok, items) = evaluate(&good_state(), &store, true);
        assert!(ok, "items: {:?}", items);
        assert_eq!(items.len(), 6);
    }

    #[test]
    fn fails_on_low_battery_and_no_gps() {
        let mut s = good_state();
        s.battery_pct = Some(10.0);
        s.lat = None;
        s.lon = None;
        let store = ParamStore::new();
        let (ok, items) = evaluate(&s, &store, true);
        assert!(!ok);
        assert!(!items.iter().find(|i| i.id == "battery").unwrap().pass);
        assert!(!items.iter().find(|i| i.id == "gps").unwrap().pass);
        assert!(!items.iter().find(|i| i.id == "params").unwrap().pass);
    }

    #[test]
    fn fails_when_already_armed_or_link_down() {
        let mut s = good_state();
        s.manual_arm = Some(true);
        let mut store = ParamStore::new();
        store.upsert("A".into(), 1.0, 9, 0, 1);
        let (ok, items) = evaluate(&s, &store, false);
        assert!(!ok);
        assert!(!items.iter().find(|i| i.id == "disarmed").unwrap().pass);
        assert!(!items.iter().find(|i| i.id == "link").unwrap().pass);
    }
}
