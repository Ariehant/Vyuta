//! Wire protocol between the companion panel (TypeScript) and the `agent`.
//!
//! JSON over a WebSocket — the same transport the other Vyuta sidecars use
//! (the plan's tonic gRPC surface is the documented upgrade once protoc is
//! available). [`Command`] is client → agent; [`Outbound`] is agent → client.

use serde::{Deserialize, Serialize};

use crate::graph::GraphFrame;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    /// Introspect the ROS 2 graph (nodes / topics / services).
    Graph,
    /// Sample one message from a topic.
    Echo { topic: String },
    /// Run `colcon build` in the workspace (optionally a package subset).
    Build {
        #[serde(default)]
        workspace: Option<String>,
        #[serde(default)]
        packages: Option<Vec<String>>,
    },
    /// Sync the workspace to the drone (`rsync` over SSH).
    Deploy {
        #[serde(default)]
        source: Option<String>,
        #[serde(default)]
        target: Option<String>,
    },
    /// Request a status frame.
    Status,
    /// Cancel the running build/deploy.
    Cancel,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Outbound {
    Graph(GraphFrame),
    Status(StatusFrame),
    Log(LogFrame),
    Echo(EchoFrame),
    Ack(AckFrame),
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusFrame {
    pub ros_available: bool,
    pub colcon_available: bool,
    /// `idle` | `building` | `deploying` | `error`.
    pub phase: &'static str,
    /// Human label of the current/last task.
    pub task: String,
    /// MAVLink↔ROS bridge status (`mavros`/`micro-ros-agent` heuristic).
    pub bridge: String,
    pub workspace: String,
    pub deploy_target: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogFrame {
    pub stream: &'static str, // stdout | stderr | agent
    pub line: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EchoFrame {
    pub topic: String,
    pub sample: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AckFrame {
    pub cmd: String,
    pub ok: bool,
    pub message: String,
}

impl Outbound {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|e| {
            format!("{{\"type\":\"log\",\"stream\":\"agent\",\"line\":\"serialize error: {e}\"}}")
        })
    }
}
