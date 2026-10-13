use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Konfigurasi aplikasi. Disimpan di ~/.config/pgadmin-manager/config.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// "auto" | "docker" | "podman"
    pub runtime: String,
    pub container_name: String,
    pub image: String,
    pub host_port: u16,
    pub email: String,
    pub password: String,
    /// Named volume untuk persistensi data pgAdmin (/var/lib/pgadmin)
    pub volume_name: String,
    pub auto_open_browser: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            runtime: "auto".to_string(),
            container_name: "pgadmin4".to_string(),
            image: "docker.io/dpage/pgadmin4:latest".to_string(),
            host_port: 5050,
            email: "pgadmin4@pgadmin.org".to_string(),
            password: "admin123".to_string(),
            volume_name: "pgadmin-data".to_string(),
            auto_open_browser: true,
        }
    }
}

impl AppConfig {
    pub fn file_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home)
            .join(".config")
            .join("pgadmin-manager")
            .join("config.toml")
    }

    pub fn load() -> Self {
        let path = Self::file_path();
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(cfg) = toml::from_str::<AppConfig>(&text) {
                return cfg;
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = Self::file_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = toml::to_string_pretty(self) {
            let _ = std::fs::write(&path, text);
        }
    }

    pub fn url(&self) -> String {
        format!("http://localhost:{}", self.host_port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_roundtrip_via_temp_home() {
        let tmp = std::env::temp_dir().join("pgadmin-manager-test");
        std::env::set_var("HOME", &tmp);
        let mut cfg = AppConfig::default();
        cfg.host_port = 5051;
        cfg.container_name = "pgadmin-test".to_string();
        cfg.save();
        let loaded = AppConfig::load();
        assert_eq!(loaded.host_port, 5051);
        assert_eq!(loaded.container_name, "pgadmin-test");
        assert_eq!(loaded.url(), "http://localhost:5051");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
