use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
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

    #[serde(default)]
    pub dnssec: Option<DnssecConfig>,

    #[serde(default)]
    pub tcp: Option<TcpConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct TcpConfig {
    /// Idle timeout for TCP connections in seconds (default: 30)
    #[serde(default = "default_tcp_idle_timeout")]
    pub idle_timeout: u64,

    /// Maximum number of queries per TCP connection (default: 100)
    #[serde(default = "default_tcp_max_queries")]
    pub max_queries_per_connection: usize,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DnssecConfig {
    #[serde(default)]
    pub validate_signatures: bool,

    #[serde(default)]
    pub require_dnssec: bool,

    #[serde(default = "default_auto_include_dnssec")]
    pub auto_include_dnssec: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ZoneConfig {
    pub name: String,
    pub file: PathBuf,
}

fn default_listen() -> String {
    "0.0.0.0:53".to_string()
}

fn default_auto_include_dnssec() -> bool {
    true
}

fn default_workers() -> usize {
    4
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_tcp_idle_timeout() -> u64 {
    30
}

fn default_tcp_max_queries() -> usize {
    100
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content =
            std::fs::read_to_string(path.as_ref()).context("Failed to read configuration file")?;

        let config: Config =
            serde_yaml::from_str(&content).context("Failed to parse YAML configuration")?;

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
        let yaml = format!(
            r#"
server:
  listen: "127.0.0.1:5353"
zones:
  - name: ""
    file: {}
"#,
            temp_file.path().display()
        );

        let config: Config = serde_yaml::from_str(&yaml).unwrap();
        let result = config.validate();
        assert!(
            result.is_err(),
            "Should fail validation with empty zone name"
        );
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
        assert!(
            result.is_err(),
            "Should fail validation with nonexistent file"
        );
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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("At least one zone")
        );
    }

    #[test]
    fn test_all_optional_fields() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "test").unwrap();
        temp_file.flush().unwrap();

        let yaml = format!(
            r#"
server:
  listen: "127.0.0.1:5353"
  workers: 8
  log_level: debug
  rate_limit: 500
  api_listen: "127.0.0.1:8080"
zones:
  - name: example.com
    file: {}
"#,
            temp_file.path().display()
        );

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
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file1 = NamedTempFile::new().unwrap();
        let mut temp_file2 = NamedTempFile::new().unwrap();
        writeln!(temp_file1, "test").unwrap();
        writeln!(temp_file2, "test").unwrap();
        temp_file1.flush().unwrap();
        temp_file2.flush().unwrap();

        let yaml = format!(
            r#"
server:
  listen: "127.0.0.1:5353"
zones:
  - name: example.com
    file: {}
  - name: example.org
    file: {}
  - name: example.net
    file: {}
"#,
            temp_file1.path().display(),
            temp_file2.path().display(),
            temp_file1.path().display()
        );

        let config: Config = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(config.zones.len(), 3);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_extreme_worker_count() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "test").unwrap();
        temp_file.flush().unwrap();

        // Test with 0 workers (edge case)
        let yaml = format!(
            r#"
server:
  listen: "127.0.0.1:5353"
  workers: 0
zones:
  - name: example.com
    file: {}
"#,
            temp_file.path().display()
        );

        let config: Config = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(config.server.workers, 0);

        // Test with very large worker count
        let yaml = format!(
            r#"
server:
  listen: "127.0.0.1:5353"
  workers: 1000
zones:
  - name: example.com
    file: {}
"#,
            temp_file.path().display()
        );

        let config: Config = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(config.server.workers, 1000);
    }

    #[test]
    fn test_default_listen_address() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "test").unwrap();
        temp_file.flush().unwrap();

        // Config without explicit listen address should use default
        let yaml = format!(
            r#"
server:
  workers: 4
zones:
  - name: example.com
    file: {}
"#,
            temp_file.path().display()
        );

        let config: Config = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(config.server.listen, "0.0.0.0:53");
    }

    #[test]
    fn test_from_file_success() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut zone_file = NamedTempFile::new().unwrap();
        writeln!(zone_file, "test zone").unwrap();
        zone_file.flush().unwrap();

        let mut config_file = NamedTempFile::new().unwrap();
        writeln!(config_file, "server:").unwrap();
        writeln!(config_file, "  listen: \"127.0.0.1:5353\"").unwrap();
        writeln!(config_file, "zones:").unwrap();
        writeln!(config_file, "  - name: example.com").unwrap();
        writeln!(config_file, "    file: {}", zone_file.path().display()).unwrap();
        config_file.flush().unwrap();

        let config = Config::from_file(config_file.path()).unwrap();
        assert_eq!(config.server.listen, "127.0.0.1:5353");
        assert_eq!(config.zones.len(), 1);
        assert_eq!(config.zones[0].name, "example.com");
    }

    #[test]
    fn test_from_file_not_found() {
        let result = Config::from_file("/nonexistent/path/to/config.yaml");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Failed to read configuration file"));
    }

    #[test]
    fn test_from_file_invalid_yaml() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut config_file = NamedTempFile::new().unwrap();
        writeln!(config_file, "invalid: yaml: {{{{").unwrap();
        config_file.flush().unwrap();

        let result = Config::from_file(config_file.path());
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Failed to parse YAML configuration"));
    }

    #[test]
    fn test_all_defaults() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "test").unwrap();
        temp_file.flush().unwrap();

        // Minimal config, all optional fields should get defaults
        let yaml = format!(
            r#"
server: {{}}
zones:
  - name: example.com
    file: {}
"#,
            temp_file.path().display()
        );

        let config: Config = serde_yaml::from_str(&yaml).unwrap();

        // Verify all defaults
        assert_eq!(config.server.listen, "0.0.0.0:53");
        assert_eq!(config.server.workers, 4);
        assert_eq!(config.server.log_level, "info");
        assert_eq!(config.server.rate_limit, None);
        assert_eq!(config.server.api_listen, None);
    }

    #[test]
    fn test_validate_success() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut zone_file = NamedTempFile::new().unwrap();
        writeln!(zone_file, "test").unwrap();
        zone_file.flush().unwrap();

        let yaml = format!(
            r#"
server:
  listen: "127.0.0.1:5353"
zones:
  - name: example.com
    file: {}
"#,
            zone_file.path().display()
        );

        let config: Config = serde_yaml::from_str(&yaml).unwrap();

        // Should validate successfully
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_partial_zone_name() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "test").unwrap();
        temp_file.flush().unwrap();

        // Zone name that's not empty but might be invalid
        let yaml = format!(
            r#"
server:
  listen: "127.0.0.1:5353"
zones:
  - name: "."
    file: {}
"#,
            temp_file.path().display()
        );

        let config: Config = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(config.zones[0].name, ".");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_tcp_config_defaults() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "test").unwrap();
        temp_file.flush().unwrap();

        // Config without TCP settings should use defaults
        let yaml = format!(
            r#"
server:
  listen: "127.0.0.1:5353"
zones:
  - name: example.com
    file: {}
"#,
            temp_file.path().display()
        );

        let config: Config = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(config.server.tcp, None);
    }

    #[test]
    fn test_tcp_config_custom_values() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "test").unwrap();
        temp_file.flush().unwrap();

        // Config with custom TCP settings
        let yaml = format!(
            r#"
server:
  listen: "127.0.0.1:5353"
  tcp:
    idle_timeout: 60
    max_queries_per_connection: 200
zones:
  - name: example.com
    file: {}
"#,
            temp_file.path().display()
        );

        let config: Config = serde_yaml::from_str(&yaml).unwrap();
        assert!(config.server.tcp.is_some());

        let tcp_config = config.server.tcp.unwrap();
        assert_eq!(tcp_config.idle_timeout, 60);
        assert_eq!(tcp_config.max_queries_per_connection, 200);
    }

    #[test]
    fn test_tcp_config_partial_values() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "test").unwrap();
        temp_file.flush().unwrap();

        // Config with only idle_timeout specified, max_queries should default
        let yaml = format!(
            r#"
server:
  listen: "127.0.0.1:5353"
  tcp:
    idle_timeout: 45
zones:
  - name: example.com
    file: {}
"#,
            temp_file.path().display()
        );

        let config: Config = serde_yaml::from_str(&yaml).unwrap();
        assert!(config.server.tcp.is_some());

        let tcp_config = config.server.tcp.unwrap();
        assert_eq!(tcp_config.idle_timeout, 45);
        assert_eq!(tcp_config.max_queries_per_connection, 100); // default
    }
}
