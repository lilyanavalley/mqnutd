use crate::error::{Error, Result};
use crate::executor::Executor;

/// Shuts down the system by running an arbitrary shell command.
pub struct CommandExecutor {
    command: String,
}

impl CommandExecutor {
    pub fn new(command: String) -> Self {
        Self { command }
    }
}

impl Executor for CommandExecutor {
    fn execute(&self) -> Result<()> {
        if self.command.trim().is_empty() {
            return Err(Error::EmptyCommand);
        }
        tracing::info!(command = %self.command, "Executing shutdown command");
        std::process::Command::new("sh")
            .arg("-c")
            .arg(&self.command)
            .status()?;
        Ok(())
    }

    fn name(&self) -> &str {
        "command"
    }
}
