use crate::error::Result;

/// Performs the shutdown action when a plan is triggered.
pub trait Executor: Send + Sync {
    /// Execute the shutdown action.
    fn execute(&self) -> Result<()>;

    /// Human-readable name of this executor (for log messages).
    fn name(&self) -> &str;
}

pub mod systemd;
pub mod command;

/// Build an executor from the config.
pub fn from_config(cfg: &crate::config::ExecutorConfig) -> Box<dyn Executor> {
    match cfg {
        crate::config::ExecutorConfig::Systemd => Box::new(systemd::SystemdExecutor),
        crate::config::ExecutorConfig::Command { command } => {
            Box::new(command::CommandExecutor::new(command.clone()))
        }
    }
}
