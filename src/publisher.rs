use std::time::Duration;

use rumqttc::{AsyncClient, LastWill, MqttOptions, QoS};

use crate::config::Config;
use crate::error::Result;
use crate::executor;
use crate::nut;

/// Run the publisher daemon loop.
///
/// Periodically polls the NUT server, publishes UPS status to MQTT, and
/// optionally executes the plan if the local machine's own shutdown conditions
/// are met.
pub async fn run(config: &Config) -> Result<()> {
    let nut_cfg = config.nut.as_ref().ok_or(crate::error::Error::NutConfigMissing)?;
    let mqtt_cfg = &config.mqtt;
    let topics = &config.topics;
    let plan = &config.plan;
    let daemon = &config.daemon;

    let poll = Duration::from_secs(daemon.poll_interval_secs);

    let mut mqtt_opts = MqttOptions::new(
        &mqtt_cfg.client_id,
        &mqtt_cfg.host,
        mqtt_cfg.port,
    );
    mqtt_opts.set_keep_alive(Duration::from_secs(mqtt_cfg.keepalive_secs));

    if let (Some(user), Some(pass)) = (&mqtt_cfg.username, &mqtt_cfg.password) {
        mqtt_opts.set_credentials(user, pass);
    }

    // LWT fires "offline" if we disconnect unexpectedly.
    mqtt_opts.set_last_will(LastWill::new(
        &topics.lwt,
        "offline",
        QoS::AtLeastOnce,
        true,
    ));

    let (mqtt_client, mut eventloop) = AsyncClient::new(mqtt_opts, 32);

    // Drive the MQTT event loop in a background task.
    tokio::spawn(async move {
        loop {
            if let Err(e) = eventloop.poll().await {
                tracing::warn!(error = %e, "MQTT event loop error (publisher)");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    });

    // Announce we are online.
    mqtt_client
        .publish(&topics.lwt, QoS::AtLeastOnce, true, "online")
        .await?;

    tracing::info!(
        ups = %nut_cfg.ups_name,
        host = %nut_cfg.host,
        mqtt_host = %mqtt_cfg.host,
        "Publisher started"
    );

    let executor = executor::from_config(&config.executor);
    let mut shutdown_triggered = false;

    loop {
        match nut::query(nut_cfg).await {
            Ok(status) => {
                tracing::debug!(
                    status = %status.status,
                    charge = status.charge,
                    "NUT poll"
                );

                // Publish UPS status and charge to MQTT.
                if let Err(e) = mqtt_client
                    .publish(&topics.status, QoS::AtLeastOnce, false, status.status.as_bytes())
                    .await
                {
                    tracing::warn!(error = %e, "Failed to publish UPS status");
                }

                let charge_str = format!("{:.1}", status.charge);
                if let Err(e) = mqtt_client
                    .publish(&topics.charge, QoS::AtLeastOnce, false, charge_str.as_bytes())
                    .await
                {
                    tracing::warn!(error = %e, "Failed to publish battery charge");
                }

                // Evaluate plan — shut down this machine too if conditions are met.
                if !shutdown_triggered && should_shutdown(&status, plan) {
                    tracing::warn!(
                        status = %status.status,
                        charge = status.charge,
                        threshold = plan.charge_threshold,
                        executor = executor.name(),
                        "Plan triggered — initiating shutdown"
                    );
                    shutdown_triggered = true;
                    if let Err(e) = executor.execute() {
                        tracing::error!(error = %e, "Executor failed");
                    }
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to query NUT server");
            }
        }

        tokio::time::sleep(poll).await;
    }
}

/// Returns `true` when the UPS status warrants a shutdown per the plan.
fn should_shutdown(status: &nut::UpsStatus, plan: &crate::config::PlanConfig) -> bool {
    // Immediate trigger statuses (e.g. LB = low battery).
    if status.has_trigger(&plan.trigger_statuses) {
        return true;
    }
    // On battery AND charge below threshold.
    status.is_on_battery() && status.charge < plan.charge_threshold
}
