use anyhow::{Context, Result};
use std::process::Command;

use crate::config::AppConfig;

/// Runtime container yang dipakai.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Runtime {
    Docker,
    Podman,
}

impl Runtime {
    pub fn cmd(&self) -> &str {
        match self {
            Runtime::Docker => "docker",
            Runtime::Podman => "podman",
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Runtime::Docker => "docker",
            Runtime::Podman => "podman",
        }
    }
}

/// Deteksi runtime: hormati preferensi config ("docker"/"podman"),
/// "auto" = pakai docker kalau daemon-nya hidup, else podman.
pub fn detect_runtime(prefer: &str) -> Runtime {
    match prefer {
        "docker" => Runtime::Docker,
        "podman" => Runtime::Podman,
        _ => {
            // Coba docker dulu (daemon harus hidup), fallback ke podman.
            let docker_ok = Command::new("docker")
                .args(["info"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if docker_ok {
                Runtime::Docker
            } else if Command::new("podman")
                .args(["info"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                Runtime::Podman
            } else {
                // Default: docker (pesan error nanti akan jelas saat command gagal).
                Runtime::Docker
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerState {
    NotFound,
    Running(String),
    Stopped,
    /// Runtime (docker/podman) tidak bisa dihubungi. Beda dengan NotFound:
    /// ini masalah lingkungan, bukan container.
    Unavailable(String),
}

/// Terjemahkan stderr mentah docker/podman jadi pesan Indonesia yang
/// bisa ditindaklanjuti. Urutan cek penting: pola spesifik dulu.
pub fn friendly_error(rt: Runtime, raw: &str) -> String {
    let low = raw.to_lowercase();
    let daemon_hint = match rt {
        Runtime::Docker => {
            "daemon docker tidak berjalan. Nyalakan dulu: sudo systemctl start docker"
        }
        Runtime::Podman => {
            "podman gagal merespons. Pastikan podman terinstal dan coba `podman info` di terminal"
        }
    };
    if low.contains("cannot connect")
        || low.contains("connection refused")
        || low.contains("dial unix")
        || (low.contains("daemon") && low.contains("running"))
        || low.contains("error during connect")
    {
        daemon_hint.to_string()
    } else if low.contains("port is already allocated")
        || low.contains("address already in use")
        || low.contains("bind: address")
    {
        "port host sudah dipakai aplikasi lain. Ubah Host port di panel Konfigurasi.".to_string()
    } else if low.contains("no such container") {
        "container tidak ada. Klik Start untuk membuatnya.".to_string()
    } else if low.contains("pull access denied")
        || low.contains("repository does not exist")
        || low.contains("unauthorized")
        || low.contains("no such image")
        || low.contains("image not known")
    {
        format!(
            "image tidak ditemukan / tidak bisa diunduh. Cek nama image, atau tarik manual: {} pull <image>",
            rt.cmd()
        )
    } else if low.contains("is already in use by container") || low.contains("has active mounts") {
        "volume sedang dipakai container. Hapus container dulu sebelum hapus volume.".to_string()
    } else if low.contains("no such volume") {
        "volume belum ada. Klik Siapkan Volume dulu.".to_string()
    } else if low.contains("is already exists")
        || (low.contains("already exists") && low.contains("container"))
        || low.contains("name is already in use")
    {
        "nama container sudah dipakai container lain. Ganti Nama container di panel Konfigurasi."
            .to_string()
    } else if raw.trim().is_empty() {
        "perintah gagal tanpa pesan error.".to_string()
    } else {
        raw.trim().to_string()
    }
}

pub struct ContainerManager {
    pub runtime: Runtime,
    pub cfg: AppConfig,
}

impl ContainerManager {
    pub fn new(cfg: AppConfig) -> Self {
        let runtime = detect_runtime(cfg.runtime.as_str());
        Self { runtime, cfg }
    }

    fn run(&self, args: &[&str]) -> Result<String> {
        let prog = self.runtime.cmd();
        let out = Command::new(prog).args(args).output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow::anyhow!(
                    "program '{}' tidak ditemukan. Instal dulu, mis. sudo pacman -S {}.",
                    prog,
                    prog
                )
            } else {
                anyhow::anyhow!("gagal menjalankan '{} {}': {}", prog, args.join(" "), e)
            }
        })?;
        if !out.status.success() {
            let raw = String::from_utf8_lossy(&out.stderr).trim().to_string();
            anyhow::bail!("{}", friendly_error(self.runtime, &raw));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    /// Validasi input sebelum operasi yang mengubah state.
    /// Dipisah agar bisa di-unit-test tanpa runtime container.
    pub fn validate(&self) -> Result<()> {
        if self.cfg.host_port == 0 {
            anyhow::bail!("host port tidak valid (1-65535).");
        }
        if !self.cfg.email.contains('@') {
            anyhow::bail!(
                "email '{}' tidak valid — harus mengandung '@'.",
                self.cfg.email
            );
        }
        if self.cfg.password.is_empty() {
            anyhow::bail!("password wajib diisi di panel Konfigurasi.");
        }
        if self.cfg.container_name.trim().is_empty() {
            anyhow::bail!("nama container wajib diisi.");
        }
        if self.cfg.volume_name.trim().is_empty() {
            anyhow::bail!("nama volume wajib diisi.");
        }
        if self.cfg.image.trim().is_empty() {
            anyhow::bail!("image wajib diisi (mis. docker.io/dpage/pgadmin4:latest).");
        }
        Ok(())
    }

    /// Status container berdasarkan nama. Kegagalan runtime (daemon mati /
    /// CLI hilang) dilaporkan sebagai Unavailable, bukan NotFound.
    pub fn status(&self) -> ContainerState {
        let prog = self.runtime.cmd();
        let out = match Command::new(prog)
            .args([
                "ps",
                "-a",
                "--filter",
                &format!("name=^{}$", self.cfg.container_name),
                "--format",
                "{{.Names}}|{{.Status}}",
            ])
            .output()
        {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return ContainerState::Unavailable(format!(
                    "program '{}' tidak ditemukan. Instal dulu, mis. sudo pacman -S {}.",
                    prog, prog
                ));
            }
            Err(e) => {
                return ContainerState::Unavailable(format!("gagal menjalankan '{}': {}", prog, e));
            }
            Ok(o) => o,
        };
        if !out.status.success() {
            let raw = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return ContainerState::Unavailable(friendly_error(self.runtime, &raw));
        }
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let mut parts = line.splitn(2, '|');
            let name = parts.next().unwrap_or("").trim();
            let st = parts.next().unwrap_or("").trim();
            if name == self.cfg.container_name {
                if st.to_lowercase().starts_with("up") {
                    return ContainerState::Running(st.to_string());
                } else {
                    return ContainerState::Stopped;
                }
            }
        }
        ContainerState::NotFound
    }

    /// Presetup named volume (idempoten). Ini yang bikin data persist.
    /// Catatan: `podman volume create` GAGAL kalau volume sudah ada
    /// (tidak seperti docker), jadi cek dulu + toleransi "already exists".
    pub fn ensure_volume(&self) -> Result<String> {
        if self.volume_exists() {
            return Ok(format!(
                "volume '{}' sudah ada, dipakai ulang",
                self.cfg.volume_name
            ));
        }
        match self.run(&["volume", "create", &self.cfg.volume_name]) {
            Ok(_) => Ok(format!(
                "volume '{}' siap (mount -> /var/lib/pgadmin)",
                self.cfg.volume_name
            )),
            Err(e) => {
                let msg = format!("{:#}", e);
                if msg.to_lowercase().contains("already exists") {
                    // Balapan / beda runtime: anggap sukses, pakai ulang.
                    Ok(format!(
                        "volume '{}' sudah ada, dipakai ulang",
                        self.cfg.volume_name
                    ))
                } else {
                    Err(e)
                }
            }
        }
    }

    pub fn volume_exists(&self) -> bool {
        self.run(&["volume", "inspect", &self.cfg.volume_name])
            .is_ok()
    }

    /// Lokasi penyimpanan volume di host (untuk ditampilkan di UI).
    pub fn volume_mountpoint(&self) -> Option<String> {
        self.run(&[
            "volume",
            "inspect",
            "--format",
            "{{.Mountpoint}}",
            &self.cfg.volume_name,
        ])
        .ok()
        .filter(|s| !s.is_empty())
    }

    pub fn remove_volume(&self) -> Result<String> {
        self.run(&["volume", "rm", &self.cfg.volume_name])?;
        Ok(format!("volume '{}' dihapus", self.cfg.volume_name))
    }

    /// Start: buat container bila belum ada (dengan volume + env),
    /// jalankan bila sudah ada tapi berhenti.
    pub fn start(&self) -> Result<String> {
        self.validate().with_context(|| "konfigurasi belum valid")?;
        self.ensure_volume()
            .with_context(|| "gagal menyiapkan volume")?;

        match self.status() {
            ContainerState::Running(d) => Ok(format!("sudah berjalan ({})", d)),
            ContainerState::Unavailable(reason) => {
                anyhow::bail!("{}", reason);
            }
            ContainerState::Stopped => {
                self.run(&["start", &self.cfg.container_name])
                    .with_context(|| "gagal menjalankan container yang berhenti")?;
                Ok("container dijalankan ulang".to_string())
            }
            ContainerState::NotFound => {
                let port_map = format!("{}:80", self.cfg.host_port);
                self.run(&[
                    "run",
                    "-d",
                    "--name",
                    &self.cfg.container_name,
                    "-p",
                    &port_map,
                    "-e",
                    &format!("PGADMIN_DEFAULT_EMAIL={}", self.cfg.email),
                    "-e",
                    &format!("PGADMIN_DEFAULT_PASSWORD={}", self.cfg.password),
                    "-e",
                    "PGADMIN_LISTEN_PORT=80",
                    "-v",
                    &format!("{}:/var/lib/pgadmin", self.cfg.volume_name),
                    "--restart",
                    "unless-stopped",
                    &self.cfg.image,
                ])
                .with_context(|| "gagal membuat container")?;
                Ok(format!("container dibuat & berjalan di {}", self.cfg.url()))
            }
        }
    }

    pub fn stop(&self) -> Result<String> {
        self.run(&["stop", &self.cfg.container_name])
            .with_context(|| "gagal menghentikan container")?;
        Ok("container dihentikan".to_string())
    }

    pub fn restart(&self) -> Result<String> {
        self.run(&["restart", &self.cfg.container_name])
            .with_context(|| "gagal me-restart container")?;
        Ok("container di-restart".to_string())
    }

    pub fn remove(&self) -> Result<String> {
        self.run(&["rm", "-f", &self.cfg.container_name])
            .with_context(|| "gagal menghapus container")?;
        Ok("container dihapus (data tetap aman di volume)".to_string())
    }

    pub fn logs(&self, tail: usize) -> Result<Vec<String>> {
        let out = self
            .run(&[
                "logs",
                "--tail",
                &tail.to_string(),
                &self.cfg.container_name,
            ])
            .with_context(|| "gagal membaca log")?;
        Ok(out.lines().map(|l| l.to_string()).collect())
    }

    /// Buka browser ke URL pgAdmin (non-blocking).
    pub fn open_browser(url: &str) {
        let url = url.to_string();
        std::thread::spawn(move || {
            #[cfg(target_os = "linux")]
            let _ = Command::new("xdg-open").arg(&url).output();
            #[cfg(target_os = "macos")]
            let _ = Command::new("open").arg(&url).output();
            #[cfg(target_os = "windows")]
            let _ = Command::new("cmd").args(["/C", "start", &url]).output();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    #[test]
    fn runtime_preference_respected() {
        assert_eq!(detect_runtime("docker"), Runtime::Docker);
        assert_eq!(detect_runtime("podman"), Runtime::Podman);
    }

    #[test]
    fn status_unknown_container_is_not_found() {
        let mut cfg = AppConfig::default();
        cfg.container_name = "pgadmin-manager-test-tidak-ada".to_string();
        let m = ContainerManager::new(cfg);
        // butuh podman/docker CLI yang hidup; bila keduanya mati,
        // status() tetap Unavailable (bukan NotFound yang menyesatkan)
        match m.status() {
            ContainerState::NotFound | ContainerState::Unavailable(_) => {}
            other => panic!("status tak terduga: {:?}", other),
        }
    }

    #[test]
    fn validate_rejects_bad_config() {
        let mut cfg = AppConfig::default();
        let m = ContainerManager::new(cfg.clone());
        assert!(m.validate().is_ok());

        cfg.email = "bukan-email".to_string();
        assert!(ContainerManager::new(cfg.clone()).validate().is_err());

        cfg.email = "a@b.c".to_string();
        cfg.password = String::new();
        assert!(ContainerManager::new(cfg.clone()).validate().is_err());

        cfg.password = "admin123".to_string();
        cfg.container_name = "  ".to_string();
        assert!(ContainerManager::new(cfg.clone()).validate().is_err());
    }

    #[test]
    fn friendly_error_maps_common_cases() {
        assert!(friendly_error(
            Runtime::Docker,
            "Cannot connect to the Docker daemon at unix:///var/run/docker.sock"
        )
        .contains("systemctl"));
        assert!(friendly_error(
            Runtime::Docker,
            "Bind for 0.0.0.0:5050 failed: port is already allocated"
        )
        .contains("port"));
        assert!(
            friendly_error(Runtime::Docker, "Error: No such container: pgadmin4").contains("Start")
        );
        assert!(friendly_error(
            Runtime::Docker,
            "Error response from daemon: pull access denied for pgadmin4, repository does not exist"
        )
        .contains("image"));
        assert!(friendly_error(Runtime::Docker, "").contains("tanpa pesan"));
    }
}
