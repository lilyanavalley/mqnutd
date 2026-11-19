use std::convert::TryInto;

use rups::{Auth, ConfigBuilder};

use crate::config::NutConfig;
use crate::error::{Error, Result};

/// Live UPS readings fetched from a NUT server.
#[derive(Debug, Clone)]
pub struct UpsStatus {
    /// Raw NUT status string (space-separated tokens, e.g. `"OL"`, `"OB LB"`).
    pub status: String,
    /// Battery charge as a percentage (`0.0`–`100.0`).
    pub charge: f32,
}

impl UpsStatus {
    /// Returns `true` if the UPS is running on battery power (status contains `"OB"`).
    pub fn is_on_battery(&self) -> bool {
        self.status.split_whitespace().any(|t| t == "OB")
    }

    /// Returns `true` if the status contains any of the given trigger tokens.
    pub fn has_trigger(&self, trigger_statuses: &[String]) -> bool {
        let tokens: Vec<&str> = self.status.split_whitespace().collect();
        trigger_statuses
            .iter()
            .any(|t| tokens.contains(&t.as_str()))
    }
}

/// Queries a NUT server once and returns the current [`UpsStatus`].
pub async fn query(cfg: &NutConfig) -> Result<UpsStatus> {
    let host: rups::Host = (cfg.host.clone(), cfg.port)
        .try_into()
        .map_err(|e: rups::ClientError| Error::Nut(e))?;

    let auth = match (&cfg.username, &cfg.password) {
        (Some(user), pw) => Some(Auth::new(user.clone(), pw.clone())),
        _ => None,
    };

    let config = ConfigBuilder::new()
        .with_host(host)
        .with_auth(auth)
        .build();

    let mut conn = rups::tokio::Connection::new(&config).await?;

    let status_var = conn.get_var(&cfg.ups_name, "ups.status").await?;
    let charge_var = conn.get_var(&cfg.ups_name, "battery.charge").await?;

    conn.close().await?;

    let status = status_var.value();
    let charge: f32 = match charge_var.value().parse() {
        Ok(v) => v,
        Err(_) => {
            tracing::warn!(
                raw = %charge_var.value(),
                "Failed to parse battery.charge from NUT — assuming 0% (safe fallback)"
            );
            0.0
        }
    };

    Ok(UpsStatus { status, charge })
}
