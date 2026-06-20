//! PX4 flight-mode decoding.
//!
//! PX4 encodes its mode in the HEARTBEAT `custom_mode` field (only meaningful
//! when `MAV_MODE_FLAG_CUSTOM_MODE_ENABLED` is set):
//!   bits 16..24  -> main mode
//!   bits 24..32  -> sub mode (only used when main mode is AUTO)

/// Decode a PX4 `custom_mode` into a human-readable mode string.
pub fn mode_string(custom_mode: u32) -> String {
    let main = ((custom_mode >> 16) & 0xff) as u8;
    let sub = ((custom_mode >> 24) & 0xff) as u8;

    let s = match main {
        1 => "MANUAL",
        2 => "ALTCTL",
        3 => "POSCTL",
        4 => match sub {
            1 => "AUTO.READY",
            2 => "AUTO.TAKEOFF",
            3 => "AUTO.LOITER",
            4 => "AUTO.MISSION",
            5 => "AUTO.RTL",
            6 => "AUTO.LAND",
            7 => "AUTO.RTGS",
            8 => "AUTO.FOLLOW",
            9 => "AUTO.PRECLAND",
            10 => "AUTO.VTOL_TAKEOFF",
            _ => "AUTO",
        },
        5 => "ACRO",
        6 => "OFFBOARD",
        7 => "STABILIZED",
        8 => "RATTITUDE",
        _ => "UNKNOWN",
    };
    s.to_string()
}

/// Build a PX4 `custom_mode` from main/sub mode bytes (used by the simulator).
pub fn custom_mode(main: u8, sub: u8) -> u32 {
    ((main as u32) << 16) | ((sub as u32) << 24)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_simple_modes() {
        assert_eq!(mode_string(custom_mode(1, 0)), "MANUAL");
        assert_eq!(mode_string(custom_mode(3, 0)), "POSCTL");
        assert_eq!(mode_string(custom_mode(7, 0)), "STABILIZED");
    }

    #[test]
    fn decodes_auto_submodes() {
        assert_eq!(mode_string(custom_mode(4, 2)), "AUTO.TAKEOFF");
        assert_eq!(mode_string(custom_mode(4, 4)), "AUTO.MISSION");
        assert_eq!(mode_string(custom_mode(4, 5)), "AUTO.RTL");
        // Unknown sub mode falls back to the generic AUTO label.
        assert_eq!(mode_string(custom_mode(4, 99)), "AUTO");
    }

    #[test]
    fn unknown_main_mode() {
        assert_eq!(mode_string(custom_mode(42, 0)), "UNKNOWN");
    }
}
