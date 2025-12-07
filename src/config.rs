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

    #[serde(default)]
    pub rate_limit: Option<u32>,

    #[serde(default)]
    pub api_listen: Option<String>,
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

    #[test]
    fn test_invalid_yaml() {
        let yaml = r#"
server:
  listen: "127.0.0.1:5353"
  invalid_key: {{{
"#;
        let result: Result<Config, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err(), "Should fail to parse invalid YAML");
    }

    #[test]
    fn test_missing_required_fields() {
        // Missing zones
        let yaml = r#"
server:
  listen: "127.0.0.1:5353"
"#;
        let result: Result<Config, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err(), "Should fail without zones");

        // Missing server
        let yaml = r#"
zones:
  - name: example.com
    file: /tmp/example.com.zone
"#;
        let result: Result<Config, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err(), "Should fail without server");
    }

    #[test]
    fn test_empty_zone_name() {
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new().unwrap();
        let yaml = format!(r#"
server:
  listen: "127.0.0.1:5353"
zones:
  - name: ""
    file: {}
"#, temp_file.path().display());

        let config: Config = serde_yaml::from_str(&yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err(), "Should fail validation with empty zone name");
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn test_nonexistent_zone_file() {
        let yaml = r#"
server:
  listen: "127.0.0.1:5353"
zones:
  - name: example.com
    file: /nonexistent/path/to/zone.file
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err(), "Should fail validation with nonexistent file");
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn test_zero_zones() {
        let yaml = r#"
server:
  listen: "127.0.0.1:5353"
zones: []
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err(), "Should fail validation with zero zones");
        assert!(result.unwrap_err().to_string().contains("At least one zone"));
    }

    #[test]
    fn test_all_optional_fields() {
        use tempfile::NamedTempFile;
        use std::io::Write;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "test").unwrap();
        temp_file.flush().unwrap();

        let yaml = format!(r#"
server:
  listen: "127.0.0.1:5353"
  workers: 8
  log_level: debug
  rate_limit: 500
  api_listen: "127.0.0.1:8080"
zones:
  - name: example.com
    file: {}
"#, temp_file.path().display());

        let config: Config = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(config.server.workers, 8);
        assert_eq!(config.server.log_level, "debug");
        assert_eq!(config.server.rate_limit, Some(500));
        assert_eq!(config.server.api_listen, Some("127.0.0.1:8080".to_string()));

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_port_number() {
        let yaml = r#"
server:
  listen: "127.0.0.1:99999"
zones:
  - name: example.com
    file: /tmp/example.com.zone
"#;
        // YAML parsing will succeed, but the listen string is just a string
        // The actual port validation happens at bind time
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.server.listen, "127.0.0.1:99999");
    }

    #[test]
    fn test_multiple_zones() {
        use tempfile::NamedTempFile;
        use std::io::Write;

        let mut temp_file1 = NamedTempFile::new().unwrap();
        let mut temp_file2 = NamedTempFile::new().unwrap();
        writeln!(temp_file1, "test").unwrap();
        writeln!(temp_file2, "test").unwrap();
        temp_file1.flush().unwrap();
        temp_file2.flush().unwrap();

        let yaml = format!(r#"
server:
  listen: "127.0.0.1:5353"
zones:
  - name: example.com
    file: {}
  - name: example.org
    file: {}
  - name: example.net
    file: {}
"#, temp_file1.path().display(), temp_file2.path().display(), temp_file1.path().display());

        let config: Config = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(config.zones.len(), 3);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_extreme_worker_count() {
        use tempfile::NamedTempFile;
        use std::io::Write;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "test").unwrap();
        temp_file.flush().unwrap();

        // Test with 0 workers (edge case)
        let yaml = format!(r#"
server:
  listen: "127.0.0.1:5353"
  workers: 0
zones:
  - name: example.com
    file: {}
"#, temp_file.path().display());

        let config: Config = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(config.server.workers, 0);

        // Test with very large worker count
        let yaml = format!(r#"
server:
  listen: "127.0.0.1:5353"
  workers: 1000
zones:
  - name: example.com
    file: {}
"#, temp_file.path().display());

        let config: Config = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(config.server.workers, 1000);
    }
}
