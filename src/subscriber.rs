use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};

use crate::config::Config;
use crate::error::Result;
use crate::executor;

/// Tracks the latest UPS state received over MQTT.
#[derive(Debug, Default, Clone)]
struct UpsState {
    /// Most recent UPS status string (e.g. `"OB"`).
    status: Option<String>,
    /// Most recent battery charge percentage.
    charge: Option<f32>,
    /// Whether the publisher's LWT "offline" message has been received.
    lwt_offline: bool,
}

impl UpsState {
    fn should_shutdown(&self, plan: &crate::config::PlanConfig) -> bool {
        // LWT fired — publisher is gone, assume worst case.
        if self.lwt_offline {
            return true;
        }

        let status = match &self.status {
            Some(s) => s,
            None => return false,
        };

        let tokens: Vec<&str> = status.split_whitespace().collect();

        // Immediate trigger tokens (e.g. "LB").
        if plan
            .trigger_statuses
            .iter()
            .any(|t| tokens.contains(&t.as_str()))
        {
            return true;
        }

        // On battery AND charge below threshold.
        let on_battery = tokens.contains(&"OB");
        let below_threshold = self
            .charge
            .map(|c| c < plan.charge_threshold)
            .unwrap_or(false);

        on_battery && below_threshold
    }
}

/// Run the subscriber daemon loop.
///
/// Connects to the MQTT broker, subscribes to UPS topics, and executes the
/// configured plan when shutdown conditions are detected.
pub async fn run(config: &Config) -> Result<()> {
    let mqtt_cfg = &config.mqtt;
    let topics = &config.topics;
    let plan = &config.plan;
    let daemon = &config.daemon;

    let reconnect_timeout = if daemon.mqtt_reconnect_timeout_secs > 0 {
        Some(Duration::from_secs(daemon.mqtt_reconnect_timeout_secs))
    } else {
        None
    };

    let mut mqtt_opts = MqttOptions::new(
        &mqtt_cfg.client_id,
        &mqtt_cfg.host,
        mqtt_cfg.port,
    );
    mqtt_opts.set_keep_alive(Duration::from_secs(mqtt_cfg.keepalive_secs));

    if let (Some(user), Some(pass)) = (&mqtt_cfg.username, &mqtt_cfg.password) {
        mqtt_opts.set_credentials(user, pass);
    }

    let (mqtt_client, mut eventloop) = AsyncClient::new(mqtt_opts, 32);

    // Subscribe to all relevant topics.
    mqtt_client.subscribe(&topics.status, QoS::AtLeastOnce).await?;
    mqtt_client.subscribe(&topics.charge, QoS::AtLeastOnce).await?;
    mqtt_client.subscribe(&topics.lwt, QoS::AtLeastOnce).await?;

    tracing::info!(
        mqtt_host = %mqtt_cfg.host,
        status_topic = %topics.status,
        charge_topic = %topics.charge,
        lwt_topic = %topics.lwt,
        "Subscriber started"
    );

    let state = Arc::new(Mutex::new(UpsState::default()));
    let executor = Arc::new(executor::from_config(&config.executor));

    // Shared flag to avoid triggering the plan more than once.
    let triggered = Arc::new(Mutex::new(false));

    let status_topic = topics.status.clone();
    let charge_topic = topics.charge.clone();
    let lwt_topic = topics.lwt.clone();
    let plan_clone = plan.clone();
    let state_clone = Arc::clone(&state);
    let executor_clone = Arc::clone(&executor);
    let triggered_clone = Arc::clone(&triggered);

    // Track when we last had a successful MQTT connection.
    // Initialised to the daemon start time so that the reconnect timeout also
    // triggers when the broker was never reachable in the first place.
    let daemon_start = Instant::now();
    let last_connected: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
    let last_connected_clone = Arc::clone(&last_connected);

    tokio::spawn(async move {
        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Packet::ConnAck(_))) => {
                    tracing::info!("MQTT connected");
                    *last_connected_clone.lock().unwrap() = Some(Instant::now());
                }
                Ok(Event::Incoming(Packet::Publish(msg))) => {
                    *last_connected_clone.lock().unwrap() = Some(Instant::now());

                    let payload = match std::str::from_utf8(&msg.payload) {
                        Ok(s) => s.trim().to_string(),
                        Err(_) => {
                            tracing::warn!(topic = %msg.topic, "Non-UTF8 MQTT payload, ignoring");
                            continue;
                        }
                    };

                    tracing::debug!(topic = %msg.topic, payload = %payload, "MQTT message");

                    let mut s = state_clone.lock().unwrap();

                    if msg.topic == status_topic {
                        s.status = Some(payload);
                    } else if msg.topic == charge_topic {
                        s.charge = payload.parse().ok();
                    } else if msg.topic == lwt_topic && payload.eq_ignore_ascii_case("offline") {
                        tracing::warn!("Publisher LWT 'offline' received — broker lost contact with publisher");
                        s.lwt_offline = true;
                    }

                    let snapshot = s.clone();
                    drop(s);

                    maybe_trigger(&snapshot, &plan_clone, &executor_clone, &triggered_clone);
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "MQTT event loop error (subscriber)");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    });

    // Periodic check: if MQTT is unreachable beyond the timeout, trigger the plan.
    let poll = Duration::from_secs(daemon.poll_interval_secs);
    loop {
        tokio::time::sleep(poll).await;

        if let Some(timeout) = reconnect_timeout {
            // Use the last successful connection time, or daemon start if we
            // have never successfully connected.
            let elapsed = {
                let lc = last_connected.lock().unwrap();
                match *lc {
                    Some(t) => t.elapsed(),
                    None => daemon_start.elapsed(),
                }
            };

            if elapsed > timeout {
                tracing::warn!(
                    elapsed_secs = elapsed.as_secs(),
                    timeout_secs = timeout.as_secs(),
                    "MQTT broker unreachable beyond timeout — triggering plan"
                );
                let mut t = triggered.lock().unwrap();
                if !*t {
                    *t = true;
                    drop(t);
                    if let Err(e) = executor.execute() {
                        tracing::error!(error = %e, "Executor failed");
                    }
                }
            }
        }

        tracing::trace!("Subscriber heartbeat");
    }
}

fn maybe_trigger(
    state: &UpsState,
    plan: &crate::config::PlanConfig,
    executor: &Arc<Box<dyn crate::executor::Executor>>,
    triggered: &Arc<Mutex<bool>>,
) {
    if !state.should_shutdown(plan) {
        return;
    }

    let mut t = triggered.lock().unwrap();
    if *t {
        return;
    }
    *t = true;
    drop(t);

    tracing::warn!(
        status = ?state.status,
        charge = ?state.charge,
        lwt_offline = state.lwt_offline,
        "Plan triggered — initiating shutdown"
    );

    if let Err(e) = executor.execute() {
        tracing::error!(error = %e, "Executor failed");
    }
}
