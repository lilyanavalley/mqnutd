use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("MQTT error: {0}")]
    Mqtt(#[from] rumqttc::ClientError),

    #[error("MQTT connection error: {0}")]
    MqttConnection(#[from] rumqttc::ConnectionError),

    #[error("NUT error: {0}")]
    Nut(#[from] rups::ClientError),

    #[error("NUT config is required for publisher mode but was not provided")]
    NutConfigMissing,

    #[error("Executor command is empty")]
    EmptyCommand,


}

pub type Result<T> = std::result::Result<T, Error>;
