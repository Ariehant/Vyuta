//! Shared companion-agent state (lifecycle + recent logs).

use std::collections::VecDeque;

use crate::protocol::StatusFrame;

const LOG_RING_CAP: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Building,
    Deploying,
    Error,
}

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Idle => "idle",
            Phase::Building => "building",
            Phase::Deploying => "deploying",
            Phase::Error => "error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogLine {
    pub stream: &'static str,
    pub line: String,
}

pub struct CompanionState {
    pub ros_available: bool,
    pub colcon_available: bool,
    pub phase: Phase,
    pub task: String,
    pub bridge: String,
    pub workspace: String,
    pub deploy_target: String,
    pub message: String,
    logs: VecDeque<LogLine>,
}

impl CompanionState {
    pub fn new(
        ros_available: bool,
        colcon_available: bool,
        workspace: String,
        deploy_target: String,
    ) -> Self {
        Self {
            ros_available,
            colcon_available,
            phase: Phase::Idle,
            task: "idle".to_string(),
            bridge: if ros_available {
                "unknown".to_string()
            } else {
                "synthetic".to_string()
            },
            workspace,
            deploy_target,
            message: "ready".to_string(),
            logs: VecDeque::with_capacity(LOG_RING_CAP),
        }
    }

    pub fn push_log(&mut self, stream: &'static str, line: String) {
        if self.logs.len() == LOG_RING_CAP {
            self.logs.pop_front();
        }
        self.logs.push_back(LogLine { stream, line });
    }

    pub fn recent_logs(&self) -> impl Iterator<Item = &LogLine> {
        self.logs.iter()
    }

    pub fn status_frame(&self) -> StatusFrame {
        StatusFrame {
            ros_available: self.ros_available,
            colcon_available: self.colcon_available,
            phase: self.phase.as_str(),
            task: self.task.clone(),
            bridge: self.bridge.clone(),
            workspace: self.workspace.clone(),
            deploy_target: self.deploy_target.clone(),
            message: self.message.clone(),
        }
    }
}
