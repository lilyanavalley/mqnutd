use crate::error::Result;
use crate::executor::Executor;

/// Shuts down the system via `systemctl poweroff`.
pub struct SystemdExecutor;

impl Executor for SystemdExecutor {
    fn execute(&self) -> Result<()> {
        tracing::info!("Executing systemd poweroff");
        std::process::Command::new("systemctl")
            .arg("poweroff")
            .status()?;
        Ok(())
    }

    fn name(&self) -> &str {
        "systemd"
    }
}
