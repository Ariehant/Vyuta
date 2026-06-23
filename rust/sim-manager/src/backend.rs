//! Simulator-agnostic launch backends (Phase 8).
//!
//! A [`SimControl`] implementation knows how to detect and launch one simulator
//! (Gazebo, jMAVSim, AirSim). The manager picks one by id; whichever is selected
//! produces a [`LaunchSpec`] the process layer runs, or — when unavailable — the
//! built-in mock flight takes over (unchanged).

use std::path::{Path, PathBuf};

use crate::protocol::CatalogEntry;
use crate::worlds;

/// Everything a backend needs to detect/launch a simulator.
pub struct SimEnv<'a> {
    pub px4_dir: Option<&'a Path>,
    pub gz_bin: &'a str,
}

/// A concrete process invocation.
pub struct LaunchSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
}

pub trait SimControl {
    fn id(&self) -> &'static str;
    fn label(&self) -> &'static str;
    /// Whether this simulator can actually run on this host.
    fn available(&self, env: &SimEnv) -> bool;
    /// Build the launch invocation for a vehicle/world.
    fn launch(&self, vehicle: &str, world: &str, headless: bool, env: &SimEnv) -> LaunchSpec;
}

pub struct Gazebo;
impl SimControl for Gazebo {
    fn id(&self) -> &'static str {
        "gazebo"
    }
    fn label(&self) -> &'static str {
        "Gazebo (gz sim)"
    }
    fn available(&self, env: &SimEnv) -> bool {
        px4_present(env.px4_dir) && which(env.gz_bin)
    }
    fn launch(&self, vehicle: &str, world: &str, headless: bool, env: &SimEnv) -> LaunchSpec {
        let mut e = Vec::new();
        if headless {
            e.push(("HEADLESS".to_string(), "1".to_string()));
        }
        LaunchSpec {
            program: "make".to_string(),
            args: vec!["px4_sitl".to_string(), worlds::make_target(vehicle, world)],
            cwd: env.px4_dir.map(|p| p.to_path_buf()),
            env: e,
        }
    }
}

pub struct Jmavsim;
impl SimControl for Jmavsim {
    fn id(&self) -> &'static str {
        "jmavsim"
    }
    fn label(&self) -> &'static str {
        "jMAVSim"
    }
    fn available(&self, env: &SimEnv) -> bool {
        px4_present(env.px4_dir) && which("java")
    }
    fn launch(&self, _vehicle: &str, _world: &str, headless: bool, env: &SimEnv) -> LaunchSpec {
        let mut e = Vec::new();
        if headless {
            e.push(("HEADLESS".to_string(), "1".to_string()));
        }
        // jMAVSim is quadrotor-only (iris); world/vehicle are not parameterised.
        LaunchSpec {
            program: "make".to_string(),
            args: vec!["px4_sitl".to_string(), "jmavsim".to_string()],
            cwd: env.px4_dir.map(|p| p.to_path_buf()),
            env: e,
        }
    }
}

pub struct Airsim;
impl SimControl for Airsim {
    fn id(&self) -> &'static str {
        "airsim"
    }
    fn label(&self) -> &'static str {
        "AirSim"
    }
    fn available(&self, _env: &SimEnv) -> bool {
        // AirSim ships as a prebuilt environment binary that varies per project;
        // we can't reliably detect it, so the mock flight is used.
        which("AirSimNH") || which("Blocks")
    }
    fn launch(&self, _vehicle: &str, _world: &str, _headless: bool, _env: &SimEnv) -> LaunchSpec {
        LaunchSpec {
            program: which_or("AirSimNH", "Blocks"),
            args: vec!["-windowed".to_string()],
            cwd: None,
            env: vec![],
        }
    }
}

/// Resolve a backend by id (defaults to Gazebo).
pub fn backend(id: &str) -> Box<dyn SimControl> {
    match id {
        "jmavsim" => Box::new(Jmavsim),
        "airsim" => Box::new(Airsim),
        _ => Box::new(Gazebo),
    }
}

/// The simulator catalogue for the panel picker.
pub fn simulators() -> Vec<CatalogEntry> {
    vec![
        CatalogEntry {
            id: "gazebo",
            label: "Gazebo (gz sim)",
            description: "PX4 SITL + Gazebo — the primary target.",
            class: "",
        },
        CatalogEntry {
            id: "jmavsim",
            label: "jMAVSim",
            description: "Lightweight Java quadrotor simulator.",
            class: "",
        },
        CatalogEntry {
            id: "airsim",
            label: "AirSim",
            description: "Unreal-based photorealistic simulator.",
            class: "",
        },
    ]
}

fn px4_present(dir: Option<&Path>) -> bool {
    dir.map(|d| d.join("Makefile").is_file()).unwrap_or(false)
}

fn which(bin: &str) -> bool {
    if bin.contains('/') {
        return Path::new(bin).is_file();
    }
    std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).any(|d| d.join(bin).is_file()))
        .unwrap_or(false)
}

fn which_or(a: &str, b: &str) -> String {
    if which(a) {
        a.to_string()
    } else {
        b.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> SimEnv<'static> {
        SimEnv {
            px4_dir: None,
            gz_bin: "gz",
        }
    }

    #[test]
    fn gazebo_launch_uses_make_target() {
        let g = Gazebo;
        let spec = g.launch("x500", "baylands", true, &env());
        assert_eq!(spec.program, "make");
        assert_eq!(spec.args, vec!["px4_sitl", "gz_x500_baylands"]);
        assert!(spec.env.iter().any(|(k, _)| k == "HEADLESS"));
    }

    #[test]
    fn jmavsim_is_quad_only() {
        let j = Jmavsim;
        let spec = j.launch("x500", "default", false, &env());
        assert_eq!(spec.args, vec!["px4_sitl", "jmavsim"]);
        assert!(spec.env.is_empty());
    }

    #[test]
    fn backend_defaults_to_gazebo() {
        assert_eq!(backend("nope").id(), "gazebo");
        assert_eq!(backend("jmavsim").id(), "jmavsim");
        assert_eq!(simulators().len(), 3);
    }
}
