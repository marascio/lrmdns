use serde::{Deserialize, Serialize};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub zones: Vec<ZoneConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ServerConfig {
    #[serde(default = "default_listen")]
    pub listen: String,

    #[serde(default = "default_workers")]
    pub workers: usize,

    #[serde(default = "default_log_level")]
    pub log_level: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ZoneConfig {
    pub name: String,
    pub file: PathBuf,
}

fn default_listen() -> String {
    "0.0.0.0:53".to_string()
}

fn default_workers() -> usize {
    4
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .context("Failed to read configuration file")?;

        let config: Config = serde_yaml::from_str(&content)
            .context("Failed to parse YAML configuration")?;

        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        if self.zones.is_empty() {
            anyhow::bail!("At least one zone must be configured");
        }

        for zone in &self.zones {
            if zone.name.is_empty() {
                anyhow::bail!("Zone name cannot be empty");
            }

            if !zone.file.exists() {
                anyhow::bail!("Zone file does not exist: {}", zone.file.display());
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        let yaml = r#"
server:
  listen: "127.0.0.1:5353"
zones:
  - name: example.com
    file: /tmp/example.com.zone
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.server.listen, "127.0.0.1:5353");
        assert_eq!(config.server.workers, 4);
        assert_eq!(config.server.log_level, "info");
    }
}
