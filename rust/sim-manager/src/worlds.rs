//! Catalogue of PX4-SITL/Gazebo worlds and vehicles offered in the pickers,
//! plus the `make` target each combination maps to.
//!
//! PX4's Gazebo SITL targets follow the shape `gz_<vehicle>[_<world>]`, e.g.
//! `make px4_sitl gz_x500` (default world) or `make px4_sitl gz_x500_baylands`.
//! The default empty world has no suffix.

use crate::protocol::CatalogEntry;

pub struct World {
    pub id: &'static str,
    pub label: &'static str,
    /// Target suffix appended after the vehicle, or empty for the default world.
    pub target_suffix: &'static str,
    pub description: &'static str,
}

pub struct Vehicle {
    pub id: &'static str,
    pub label: &'static str,
    /// PX4 Gazebo model name used in the make target (after the `gz_` prefix).
    pub model: &'static str,
    /// Vehicle class / profile: multirotor | vtol | fixedwing | rover.
    pub class: &'static str,
    pub description: &'static str,
}

pub const DEFAULT_WORLD: &str = "default";
pub const DEFAULT_VEHICLE: &str = "x500";
pub const DEFAULT_SIMULATOR: &str = "gazebo";

pub const WORLDS: &[World] = &[
    World {
        id: "default",
        label: "Default (empty)",
        target_suffix: "",
        description: "Flat ground plane — fastest to start.",
    },
    World {
        id: "baylands",
        label: "Baylands",
        target_suffix: "baylands",
        description: "Open park/wetland environment.",
    },
    World {
        id: "windy",
        label: "Windy",
        target_suffix: "windy",
        description: "Empty world preconfigured with a wind plugin.",
    },
    World {
        id: "lawn",
        label: "Lawn",
        target_suffix: "lawn",
        description: "Grass field for rover/ground testing.",
    },
];

pub const VEHICLES: &[Vehicle] = &[
    Vehicle {
        id: "x500",
        label: "x500 Quadcopter",
        model: "x500",
        class: "multirotor",
        description: "Holybro X500 quad — the default PX4 multirotor.",
    },
    Vehicle {
        id: "standard_vtol",
        label: "Standard VTOL",
        model: "standard_vtol",
        class: "vtol",
        description: "Quadplane VTOL (hover + fixed-wing cruise).",
    },
    Vehicle {
        id: "rc_cessna",
        label: "RC Cessna (plane)",
        model: "rc_cessna",
        class: "fixedwing",
        description: "Fixed-wing aircraft.",
    },
    Vehicle {
        id: "r1_rover",
        label: "R1 Rover",
        model: "r1_rover",
        class: "rover",
        description: "Ground rover (Aion R1).",
    },
];

/// Resolve the `make px4_sitl <target>` argument for a world/vehicle pair,
/// falling back to defaults for unknown ids.
pub fn make_target(vehicle_id: &str, world_id: &str) -> String {
    let model = VEHICLES
        .iter()
        .find(|v| v.id == vehicle_id)
        .map(|v| v.model)
        .unwrap_or(DEFAULT_VEHICLE);
    let suffix = WORLDS
        .iter()
        .find(|w| w.id == world_id)
        .map(|w| w.target_suffix)
        .unwrap_or("");
    if suffix.is_empty() {
        format!("gz_{model}")
    } else {
        format!("gz_{model}_{suffix}")
    }
}

/// Look up a world's display label (for status/log messages).
pub fn world_label(world_id: &str) -> &'static str {
    WORLDS
        .iter()
        .find(|w| w.id == world_id)
        .map(|w| w.label)
        .unwrap_or("Default (empty)")
}

/// Look up a vehicle's display label.
pub fn vehicle_label(vehicle_id: &str) -> &'static str {
    VEHICLES
        .iter()
        .find(|v| v.id == vehicle_id)
        .map(|v| v.label)
        .unwrap_or("x500 Quadcopter")
}

/// True if `id` names a vehicle that primarily drives on the ground.
pub fn is_ground_vehicle(vehicle_id: &str) -> bool {
    matches!(vehicle_id, "r1_rover")
}

/// Build the catalogue frame payload for the pickers.
pub fn catalog() -> (Vec<CatalogEntry>, Vec<CatalogEntry>) {
    let worlds = WORLDS
        .iter()
        .map(|w| CatalogEntry {
            id: w.id,
            label: w.label,
            description: w.description,
            class: "",
        })
        .collect();
    let vehicles = VEHICLES
        .iter()
        .map(|v| CatalogEntry {
            id: v.id,
            label: v.label,
            description: v.description,
            class: v.class,
        })
        .collect();
    (worlds, vehicles)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_target_has_no_world_suffix() {
        assert_eq!(make_target("x500", "default"), "gz_x500");
    }

    #[test]
    fn world_suffix_is_appended() {
        assert_eq!(make_target("x500", "baylands"), "gz_x500_baylands");
        assert_eq!(
            make_target("standard_vtol", "windy"),
            "gz_standard_vtol_windy"
        );
    }

    #[test]
    fn unknown_ids_fall_back_to_defaults() {
        assert_eq!(make_target("nope", "nope"), "gz_x500");
    }

    #[test]
    fn catalogue_is_non_empty_and_labelled() {
        let (worlds, vehicles) = catalog();
        assert!(!worlds.is_empty() && !vehicles.is_empty());
        assert_eq!(world_label("baylands"), "Baylands");
        assert_eq!(vehicle_label("x500"), "x500 Quadcopter");
    }

    #[test]
    fn rover_is_ground_vehicle() {
        assert!(is_ground_vehicle("r1_rover"));
        assert!(!is_ground_vehicle("x500"));
    }
}
