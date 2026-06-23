//! ROS 2 graph model: parsing `ros2` CLI output, plus a synthetic graph for
//! when ROS 2 is not installed (so the companion panel works out of the box).

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct NodeInfo {
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TopicInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphFrame {
    pub ros_available: bool,
    pub synthetic: bool,
    pub nodes: Vec<NodeInfo>,
    pub topics: Vec<TopicInfo>,
    pub services: Vec<ServiceInfo>,
}

/// Parse `ros2 node list` (one node name per line).
pub fn parse_node_list(stdout: &str) -> Vec<NodeInfo> {
    stdout
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|l| NodeInfo {
            name: l.to_string(),
        })
        .collect()
}

/// Parse `ros2 topic list -t` / `ros2 service list -t`:
/// lines like `/topic [pkg/msg/Type]`.
pub fn parse_named_with_types(stdout: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(open) = line.find('[') {
            let name = line[..open].trim().to_string();
            let ty = line[open + 1..].trim_end_matches(']').trim().to_string();
            out.push((name, ty));
        } else {
            out.push((line.to_string(), String::new()));
        }
    }
    out
}

pub fn topics_from_types(pairs: Vec<(String, String)>) -> Vec<TopicInfo> {
    pairs
        .into_iter()
        .map(|(name, type_name)| TopicInfo { name, type_name })
        .collect()
}

pub fn services_from_types(pairs: Vec<(String, String)>) -> Vec<ServiceInfo> {
    pairs
        .into_iter()
        .map(|(name, type_name)| ServiceInfo { name, type_name })
        .collect()
}

/// A realistic PX4 + companion ROS 2 graph for offline/demo use.
pub fn synthetic_graph() -> GraphFrame {
    let nodes = [
        "/micro_ros_agent",
        "/mavros",
        "/camera_node",
        "/ekf2_node",
        "/mission_manager",
        "/rosbag2_recorder",
    ]
    .iter()
    .map(|n| NodeInfo {
        name: n.to_string(),
    })
    .collect();

    let topics = [
        ("/fmu/out/vehicle_odometry", "px4_msgs/msg/VehicleOdometry"),
        ("/fmu/out/vehicle_status", "px4_msgs/msg/VehicleStatus"),
        (
            "/fmu/in/trajectory_setpoint",
            "px4_msgs/msg/TrajectorySetpoint",
        ),
        ("/mavros/state", "mavros_msgs/msg/State"),
        (
            "/mavros/global_position/global",
            "sensor_msgs/msg/NavSatFix",
        ),
        ("/camera/image_raw", "sensor_msgs/msg/Image"),
        ("/camera/camera_info", "sensor_msgs/msg/CameraInfo"),
        ("/tf", "tf2_msgs/msg/TFMessage"),
        ("/scan", "sensor_msgs/msg/LaserScan"),
        ("/rosout", "rcl_interfaces/msg/Log"),
    ]
    .iter()
    .map(|(n, t)| TopicInfo {
        name: n.to_string(),
        type_name: t.to_string(),
    })
    .collect();

    let services = [
        ("/mavros/cmd/arming", "mavros_msgs/srv/CommandBool"),
        ("/mavros/set_mode", "mavros_msgs/srv/SetMode"),
        (
            "/ekf2_node/set_parameters",
            "rcl_interfaces/srv/SetParameters",
        ),
    ]
    .iter()
    .map(|(n, t)| ServiceInfo {
        name: n.to_string(),
        type_name: t.to_string(),
    })
    .collect();

    GraphFrame {
        ros_available: false,
        synthetic: true,
        nodes,
        topics,
        services,
    }
}

/// A synthetic one-shot sample for `ros2 topic echo` when ROS is absent.
pub fn synthetic_echo(topic: &str) -> String {
    match topic {
        t if t.contains("vehicle_status") => {
            "nav_state: 2\narming_state: 2\nfailsafe: false".to_string()
        }
        t if t.contains("odometry") => {
            "position: [1.2, -0.4, -5.0]\nq: [1.0, 0.0, 0.0, 0.0]".to_string()
        }
        t if t.contains("NavSatFix") || t.contains("global") => {
            "latitude: 47.397742\nlongitude: 8.545594\naltitude: 488.0".to_string()
        }
        _ => format!("(synthetic) one sample from {topic}\ndata: …"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_node_list() {
        let n = parse_node_list("/mavros\n/camera_node\n\n  /ekf2  \n");
        assert_eq!(n.len(), 3);
        assert_eq!(n[2].name, "/ekf2");
    }

    #[test]
    fn parses_typed_list() {
        let pairs = parse_named_with_types(
            "/fmu/out/vehicle_status [px4_msgs/msg/VehicleStatus]\n/tf [tf2_msgs/msg/TFMessage]\n",
        );
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].0, "/fmu/out/vehicle_status");
        assert_eq!(pairs[0].1, "px4_msgs/msg/VehicleStatus");
    }

    #[test]
    fn synthetic_graph_is_populated() {
        let g = synthetic_graph();
        assert!(g.synthetic && !g.ros_available);
        assert!(!g.nodes.is_empty() && !g.topics.is_empty() && !g.services.is_empty());
    }
}
