//! Vehicle parameter store and MAVLink parameter read/write (Phase 4).
//!
//! `maestros` owns the MAVLink link, so vehicle parameters live here too. The
//! [`ParamStore`] caches `PARAM_VALUE` updates; the tuning panel can request the
//! full list, set values (`PARAM_SET`), refresh a single parameter
//! (`PARAM_REQUEST_READ`), and save / diff snapshots. When no real link is
//! configured a synthetic PX4-like parameter set is seeded so the panel works
//! out of the box — the same philosophy as the synthetic telemetry source.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use mavlink::common::{
    MavMessage, MavParamType, PARAM_REQUEST_LIST_DATA, PARAM_REQUEST_READ_DATA, PARAM_SET_DATA,
};
use mavlink::{MavConnection, MavHeader};

/// The active MAVLink connection, published by the source once connected and
/// cleared on disconnect. `None` while reconnecting.
pub type SharedConn = Arc<Mutex<Option<Arc<dyn MavConnection<MavMessage> + Send + Sync>>>>;

const GCS_SYSTEM_ID: u8 = 255;
const GCS_COMPONENT_ID: u8 = 190; // MAV_COMP_ID_MISSIONPLANNER

/// One cached parameter.
#[derive(Debug, Clone)]
pub struct Param {
    pub id: String,
    pub value: f32,
    pub ptype: u8,
    pub index: u16,
    /// Store version at the last update (for change tracking).
    pub version: u64,
}

/// A single difference between the current values and a saved snapshot.
#[derive(Debug, Clone)]
pub struct DiffEntry {
    pub id: String,
    pub from: Option<f32>,
    pub to: Option<f32>,
    pub kind: &'static str, // "changed" | "added" | "removed"
}

pub struct ParamStore {
    params: BTreeMap<String, Param>,
    total: u16,
    version: u64,
    target_system: u8,
    target_component: u8,
    snapshots: BTreeMap<String, BTreeMap<String, f32>>,
}

impl Default for ParamStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ParamStore {
    pub fn new() -> Self {
        Self {
            params: BTreeMap::new(),
            total: 0,
            version: 0,
            target_system: 1, // PX4 autopilot defaults
            target_component: 1,
            snapshots: BTreeMap::new(),
        }
    }

    /// Insert or update a parameter from a received `PARAM_VALUE`.
    pub fn upsert(&mut self, id: String, value: f32, ptype: u8, index: u16, count: u16) {
        self.version += 1;
        self.total = self.total.max(count);
        let v = self.version;
        self.params
            .entry(id.clone())
            .and_modify(|p| {
                p.value = value;
                p.ptype = ptype;
                p.index = index;
                p.version = v;
            })
            .or_insert(Param {
                id,
                value,
                ptype,
                index,
                version: v,
            });
    }

    /// Optimistically apply a local set (UI responsiveness; a real link will
    /// confirm via the echoed `PARAM_VALUE`). Returns the param's MAV type if
    /// the parameter is known.
    pub fn set_local(&mut self, id: &str, value: f32) -> Option<u8> {
        self.version += 1;
        let v = self.version;
        if let Some(p) = self.params.get_mut(id) {
            p.value = value;
            p.version = v;
            Some(p.ptype)
        } else {
            None
        }
    }

    pub fn received(&self) -> usize {
        self.params.len()
    }

    pub fn total(&self) -> u16 {
        self.total.max(self.params.len() as u16)
    }

    pub fn current_version(&self) -> u64 {
        self.version
    }

    /// Parameters changed since store version `since`.
    pub fn changed_since(&self, since: u64) -> Vec<Param> {
        self.params
            .values()
            .filter(|p| p.version > since)
            .cloned()
            .collect()
    }

    pub fn set_target(&mut self, system: u8, component: u8) {
        if system != 0 {
            self.target_system = system;
            self.target_component = component;
        }
    }

    pub fn target(&self) -> (u8, u8) {
        (self.target_system, self.target_component)
    }

    pub fn save_snapshot(&mut self, name: String) -> usize {
        let snap: BTreeMap<String, f32> = self
            .params
            .iter()
            .map(|(k, p)| (k.clone(), p.value))
            .collect();
        let n = snap.len();
        self.snapshots.insert(name, snap);
        n
    }

    pub fn delete_snapshot(&mut self, name: &str) -> bool {
        self.snapshots.remove(name).is_some()
    }

    pub fn snapshot_names(&self) -> Vec<String> {
        self.snapshots.keys().cloned().collect()
    }

    /// Diff the current values against a saved snapshot.
    pub fn diff(&self, name: &str) -> Option<Vec<DiffEntry>> {
        let snap = self.snapshots.get(name)?;
        let mut out = Vec::new();
        for (id, p) in &self.params {
            match snap.get(id) {
                Some(&old) if (old - p.value).abs() > f32::EPSILON => out.push(DiffEntry {
                    id: id.clone(),
                    from: Some(old),
                    to: Some(p.value),
                    kind: "changed",
                }),
                None => out.push(DiffEntry {
                    id: id.clone(),
                    from: None,
                    to: Some(p.value),
                    kind: "added",
                }),
                _ => {}
            }
        }
        for (id, &old) in snap {
            if !self.params.contains_key(id) {
                out.push(DiffEntry {
                    id: id.clone(),
                    from: Some(old),
                    to: None,
                    kind: "removed",
                });
            }
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Some(out)
    }
}

/// How parameter writes reach the vehicle.
pub enum Link {
    /// No real link — operate purely on the local store.
    Synthetic,
    /// Send over the shared MAVLink connection.
    Mavlink(SharedConn),
}

/// Front door for the WebSocket layer to drive parameter operations.
pub struct ParamService {
    pub store: Arc<Mutex<ParamStore>>,
    link: Link,
    seq: AtomicU8,
}

impl ParamService {
    pub fn new(store: Arc<Mutex<ParamStore>>, link: Link) -> Arc<Self> {
        Arc::new(Self {
            store,
            link,
            seq: AtomicU8::new(0),
        })
    }

    /// Ask the vehicle to stream its full parameter list. (Synthetic: the store
    /// is already populated, so this is a no-op.)
    pub fn request_list(&self) {
        if let Link::Mavlink(_) = self.link {
            let (sys, comp) = self.store.lock().expect("param store poisoned").target();
            self.send(MavMessage::PARAM_REQUEST_LIST(PARAM_REQUEST_LIST_DATA {
                target_system: sys,
                target_component: comp,
            }));
        }
    }

    /// Set a parameter. Updates the store optimistically and, on a real link,
    /// sends `PARAM_SET`. Returns true if the parameter was known.
    pub fn set_param(&self, id: &str, value: f64) -> bool {
        let ptype = self
            .store
            .lock()
            .expect("param store poisoned")
            .set_local(id, value as f32);
        let Some(ptype) = ptype else {
            return false;
        };
        if let Link::Mavlink(_) = self.link {
            let (sys, comp) = self.store.lock().expect("param store poisoned").target();
            self.send(MavMessage::PARAM_SET(PARAM_SET_DATA {
                param_value: value as f32,
                target_system: sys,
                target_component: comp,
                param_id: encode_id(id),
                param_type: mav_param_type(ptype),
            }));
        }
        true
    }

    /// Re-read a single parameter from the vehicle (real link only).
    pub fn refresh_param(&self, id: &str) {
        if let Link::Mavlink(_) = self.link {
            let (sys, comp) = self.store.lock().expect("param store poisoned").target();
            self.send(MavMessage::PARAM_REQUEST_READ(PARAM_REQUEST_READ_DATA {
                param_index: -1,
                target_system: sys,
                target_component: comp,
                param_id: encode_id(id),
            }));
        }
    }

    fn send(&self, msg: MavMessage) {
        let Link::Mavlink(slot) = &self.link else {
            return;
        };
        let conn = slot.lock().expect("conn slot poisoned").clone();
        if let Some(conn) = conn {
            let header = MavHeader {
                system_id: GCS_SYSTEM_ID,
                component_id: GCS_COMPONENT_ID,
                sequence: self.seq.fetch_add(1, Ordering::Relaxed),
            };
            if let Err(e) = conn.send(&header, &msg) {
                tracing::warn!(error = %e, "failed to send MAVLink param command");
            }
        } else {
            tracing::warn!("no MAVLink connection — param command dropped");
        }
    }
}

/// Encode a parameter id into the fixed 16-byte MAVLink field.
pub fn encode_id(id: &str) -> [u8; 16] {
    let mut out = [0u8; 16];
    for (dst, src) in out.iter_mut().zip(id.bytes()) {
        *dst = src;
    }
    out
}

/// Decode a 16-byte MAVLink id field (NUL-padded) into a string.
pub fn decode_id(raw: &[u8; 16]) -> String {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    String::from_utf8_lossy(&raw[..end]).into_owned()
}

/// Map a stored MAV_PARAM_TYPE discriminant back to the enum (default REAL32).
pub fn mav_param_type(n: u8) -> MavParamType {
    match n {
        1 => MavParamType::MAV_PARAM_TYPE_UINT8,
        2 => MavParamType::MAV_PARAM_TYPE_INT8,
        3 => MavParamType::MAV_PARAM_TYPE_UINT16,
        4 => MavParamType::MAV_PARAM_TYPE_INT16,
        5 => MavParamType::MAV_PARAM_TYPE_UINT32,
        6 => MavParamType::MAV_PARAM_TYPE_INT32,
        7 => MavParamType::MAV_PARAM_TYPE_UINT64,
        8 => MavParamType::MAV_PARAM_TYPE_INT64,
        10 => MavParamType::MAV_PARAM_TYPE_REAL64,
        _ => MavParamType::MAV_PARAM_TYPE_REAL32,
    }
}

/// Seed a synthetic PX4-like parameter set so the tuning panel is usable with
/// no vehicle/SITL connected. Types: 9 = REAL32, 6 = INT32.
pub fn seed_synthetic(store: &Arc<Mutex<ParamStore>>) {
    const SEED: &[(&str, f32, u8)] = &[
        ("MC_ROLLRATE_P", 0.15, 9),
        ("MC_ROLLRATE_I", 0.2, 9),
        ("MC_ROLLRATE_D", 0.003, 9),
        ("MC_PITCHRATE_P", 0.15, 9),
        ("MC_PITCHRATE_I", 0.2, 9),
        ("MC_PITCHRATE_D", 0.003, 9),
        ("MC_YAWRATE_P", 0.2, 9),
        ("MC_ROLL_P", 6.5, 9),
        ("MC_PITCH_P", 6.5, 9),
        ("MC_YAW_P", 2.8, 9),
        ("MPC_XY_P", 0.95, 9),
        ("MPC_XY_VEL_P_ACC", 1.8, 9),
        ("MPC_Z_P", 1.0, 9),
        ("MPC_Z_VEL_P_ACC", 4.0, 9),
        ("MPC_XY_CRUISE", 5.0, 9),
        ("MPC_Z_VEL_MAX_UP", 3.0, 9),
        ("MPC_Z_VEL_MAX_DN", 1.5, 9),
        ("MPC_TILTMAX_AIR", 45.0, 9),
        ("MPC_THR_HOVER", 0.5, 9),
        ("EKF2_GPS_CHECK", 245.0, 6),
        ("EKF2_AID_MASK", 1.0, 6),
        ("EKF2_HGT_REF", 1.0, 6),
        ("EKF2_RNG_AID", 0.0, 6),
        ("BAT1_N_CELLS", 4.0, 6),
        ("BAT1_V_EMPTY", 3.5, 9),
        ("BAT1_V_CHARGED", 4.2, 9),
        ("BAT1_CAPACITY", 5000.0, 9),
        ("COM_RC_IN_MODE", 0.0, 6),
        ("COM_DISARM_LAND", 2.0, 9),
        ("COM_RCL_EXCEPT", 0.0, 6),
        ("NAV_RCL_ACT", 2.0, 6),
        ("NAV_DLL_ACT", 0.0, 6),
        ("RTL_RETURN_ALT", 30.0, 9),
        ("RTL_DESCEND_ALT", 10.0, 9),
        ("SYS_AUTOSTART", 4001.0, 6),
        ("SYS_MC_EST_GROUP", 2.0, 6),
        ("PWM_MAIN_MIN", 1000.0, 6),
        ("PWM_MAIN_MAX", 2000.0, 6),
        ("CBRK_IO_SAFETY", 0.0, 6),
        ("SENS_BOARD_ROT", 0.0, 6),
    ];
    let count = SEED.len() as u16;
    let mut s = store.lock().expect("param store poisoned");
    for (i, (id, value, ptype)) in SEED.iter().enumerate() {
        s.upsert((*id).to_string(), *value, *ptype, i as u16, count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_roundtrip() {
        let raw = encode_id("MC_ROLLRATE_P");
        assert_eq!(decode_id(&raw), "MC_ROLLRATE_P");
        // 16-char id fills the field exactly with no NUL terminator.
        let exact = encode_id("ABCDEFGHIJKLMNOP");
        assert_eq!(decode_id(&exact), "ABCDEFGHIJKLMNOP");
    }

    #[test]
    fn upsert_bumps_version_and_tracks_changes() {
        let mut s = ParamStore::new();
        s.upsert("A".into(), 1.0, 9, 0, 2);
        s.upsert("B".into(), 2.0, 9, 1, 2);
        let v = s.current_version();
        assert_eq!(s.received(), 2);
        assert_eq!(s.total(), 2);
        s.upsert("A".into(), 1.5, 9, 0, 2);
        let changed = s.changed_since(v);
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].id, "A");
        assert_eq!(changed[0].value, 1.5);
    }

    #[test]
    fn set_local_returns_type_and_updates() {
        let mut s = ParamStore::new();
        s.upsert("MPC_XY_P".into(), 0.95, 9, 0, 1);
        assert_eq!(s.set_local("MPC_XY_P", 1.2), Some(9));
        assert!(s.set_local("UNKNOWN", 1.0).is_none());
    }

    #[test]
    fn snapshot_diff_detects_change_add_remove() {
        let mut s = ParamStore::new();
        s.upsert("A".into(), 1.0, 9, 0, 3);
        s.upsert("B".into(), 2.0, 9, 1, 3);
        s.save_snapshot("base".into());
        s.set_local("A", 1.5); // changed
        s.upsert("C".into(), 9.0, 9, 2, 4); // added
                                            // (B unchanged; nothing removed unless we drop a key)
        let diff = s.diff("base").unwrap();
        let kinds: Vec<_> = diff.iter().map(|d| (d.id.as_str(), d.kind)).collect();
        assert!(kinds.contains(&("A", "changed")));
        assert!(kinds.contains(&("C", "added")));
        assert!(!kinds.iter().any(|(_, k)| *k == "removed"));
        assert!(s.diff("missing").is_none());
    }

    #[test]
    fn synthetic_seed_populates_store() {
        let store = Arc::new(Mutex::new(ParamStore::new()));
        seed_synthetic(&store);
        let s = store.lock().unwrap();
        assert!(s.received() >= 40);
        assert_eq!(s.total(), s.received() as u16);
    }

    #[test]
    fn param_type_mapping() {
        assert!(matches!(
            mav_param_type(6),
            MavParamType::MAV_PARAM_TYPE_INT32
        ));
        assert!(matches!(
            mav_param_type(9),
            MavParamType::MAV_PARAM_TYPE_REAL32
        ));
        assert!(matches!(
            mav_param_type(99),
            MavParamType::MAV_PARAM_TYPE_REAL32
        ));
    }
}
