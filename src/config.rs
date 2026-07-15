use serde::{Deserialize, Serialize};

/// Daemon operating mode.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Read NUT status and publish it to MQTT.
    Publisher,
    /// Subscribe to MQTT status and execute a plan when triggered.
    Subscriber,
}

/// NUT server connection settings (required for `publisher` mode).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NutConfig {
    /// Hostname or IP address of the NUT server (`upsd`).
    pub host: String,
    /// TCP port of the NUT server (default: 3493).
    #[serde(default = "default_nut_port")]
    pub port: u16,
    /// UPS device name as configured in `ups.conf`.
    pub ups_name: String,
    /// Optional NUT username.
    pub username: Option<String>,
    /// Optional NUT password.
    pub password: Option<String>,
}

fn default_nut_port() -> u16 {
    3493
}

/// MQTT broker connection settings.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MqttConfig {
    /// Hostname or IP address of the MQTT broker.
    pub host: String,
    /// MQTT broker port (default: 1883).
    #[serde(default = "default_mqtt_port")]
    pub port: u16,
    /// MQTT client identifier (must be unique per broker).
    pub client_id: String,
    /// Optional MQTT username.
    pub username: Option<String>,
    /// Optional MQTT password.
    pub password: Option<String>,
    /// MQTT keep-alive interval in seconds (default: 30).
    #[serde(default = "default_keepalive")]
    pub keepalive_secs: u64,
}

fn default_mqtt_port() -> u16 {
    1883
}

fn default_keepalive() -> u64 {
    30
}

/// MQTT topic configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TopicsConfig {
    /// Topic where the publisher writes — and the subscriber reads — the UPS status string
    /// (e.g. `"OL"`, `"OB"`, `"LB OB"`).
    pub status: String,
    /// Topic for the battery charge percentage (e.g. `"75.5"`).
    pub charge: String,
    /// Last Will Testament topic.  The broker publishes `"offline"` here when the publisher
    /// disconnects unexpectedly; subscribers treat this as an emergency trigger.
    pub lwt: String,
}

/// Plan that drives the shutdown decision.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlanConfig {
    /// Shut down when battery charge (%) drops **below** this value while on battery.
    #[serde(default = "default_charge_threshold")]
    pub charge_threshold: f32,
    /// Shut down immediately when the UPS status contains any of these tokens
    /// (e.g. `["LB"]` for low-battery).
    #[serde(default = "default_trigger_statuses")]
    pub trigger_statuses: Vec<String>,
}

fn default_charge_threshold() -> f32 {
    20.0
}

fn default_trigger_statuses() -> Vec<String> {
    vec!["LB".to_string()]
}

/// How to perform the shutdown action.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ExecutorConfig {
    /// Invoke `systemctl poweroff` via systemd (default).
    Systemd,
    /// Run an arbitrary shell command.
    Command {
        /// Full shell command to execute (passed to `sh -c`).
        command: String,
    },
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        ExecutorConfig::Systemd
    }
}

/// Runtime daemon settings.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DaemonConfig {
    /// How often to poll NUT (publisher) or evaluate state (subscriber), in seconds.
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,
    /// Subscriber: trigger the plan if the MQTT broker cannot be reached for this many seconds.
    /// Set to `0` to disable.
    #[serde(default = "default_reconnect_timeout")]
    pub mqtt_reconnect_timeout_secs: u64,
}

fn default_poll_interval() -> u64 {
    10
}

fn default_reconnect_timeout() -> u64 {
    60
}

/// Top-level daemon configuration (loaded from a TOML file).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// Operating mode of this daemon instance.
    pub mode: Mode,
    /// NUT server settings — required when `mode = "publisher"`.
    pub nut: Option<NutConfig>,
    pub mqtt: MqttConfig,
    pub topics: TopicsConfig,
    pub plan: PlanConfig,
    #[serde(default)]
    pub executor: ExecutorConfig,
    #[serde(default = "default_daemon")]
    pub daemon: DaemonConfig,
}

fn default_daemon() -> DaemonConfig {
    DaemonConfig {
        poll_interval_secs: default_poll_interval(),
        mqtt_reconnect_timeout_secs: default_reconnect_timeout(),
    }
}
