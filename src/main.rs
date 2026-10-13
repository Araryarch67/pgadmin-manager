mod config;
mod container;
mod text_input;

use config::AppConfig;
use container::{ContainerManager, ContainerState};
use text_input::TextInput;

use gpui::prelude::*;
use gpui::{
    actions, div, px, rgb, rgba, size, AppContext, Application, Bounds, ClickEvent, Context, Div,
    Entity, FontWeight, InteractiveElement, IntoElement, KeyBinding, ParentElement, Render,
    Stateful, StatefulInteractiveElement, Styled, Subscription, TitlebarOptions, Window,
    WindowBounds, WindowOptions,
};

actions!(
    text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        Paste,
        Cut,
        Copy
    ]
);

// ---------- palet: Everblush ----------
// background #141b1e, foreground #dadada, border dari color8 #2d3437.
// Satu aksen (blue #67b0e9) untuk aksi primer & state aktif.
// green/yellow/red Everblush reserved untuk dot status (state nyata)
// dan konfirmasi destruktif saja.
fn bg_app() -> gpui::Rgba {
    rgb(0x141b1e)
}
fn bg_card() -> gpui::Rgba {
    rgb(0x1b2226)
}
fn bg_field() -> gpui::Rgba {
    rgb(0x101415)
}
fn border() -> gpui::Rgba {
    rgb(0x2d3437)
}
fn txt() -> gpui::Rgba {
    rgb(0xdadada)
}
fn muted() -> gpui::Rgba {
    rgb(0x8b918b)
}
fn accent() -> gpui::Rgba {
    rgb(0x67b0e9)
}
fn green() -> gpui::Rgba {
    rgb(0x8ccf7e)
}
fn red() -> gpui::Rgba {
    rgb(0xe57474)
}
fn amber() -> gpui::Rgba {
    rgb(0xe5c76b)
}

// ---------- tombol ----------
// ID stabil (tidak ikut berubah saat label/state berubah) agar GPUI tidak
// menganggapnya elemen baru setiap render — itu yang bikin layout lompat.
fn btn_ghost(id: &'static str, label: &str) -> Stateful<Div> {
    div()
        .id(id)
        .cursor_pointer()
        .px_4()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(border())
        .text_color(txt())
        .text_sm()
        .hover(|s| s.bg(rgba(0xffffff0c)).border_color(muted()))
        .child(label.to_string())
}

fn btn_primary(id: &'static str, label: &str) -> Stateful<Div> {
    div()
        .id(id)
        .cursor_pointer()
        .px_4()
        .py_2()
        .rounded_md()
        .bg(accent())
        .text_color(rgb(0x141b1e))
        .text_sm()
        .font_weight(FontWeight::SEMIBOLD)
        .hover(|s| s.opacity(0.85))
        .child(label.to_string())
}

/// Aksi utama saat state-nya "jalan": solid netral, bukan aksen.
fn btn_primary_neutral(id: &'static str, label: &str) -> Stateful<Div> {
    div()
        .id(id)
        .cursor_pointer()
        .px_4()
        .py_2()
        .rounded_md()
        .bg(txt())
        .text_color(rgb(0x141b1e))
        .text_sm()
        .font_weight(FontWeight::SEMIBOLD)
        .hover(|s| s.opacity(0.85))
        .child(label.to_string())
}

/// Tombol nonaktif yang jujur: tidak bisa diklik, tampak redup.
fn btn_off(id: &'static str, label: &str) -> Stateful<Div> {
    div()
        .id(id)
        .px_4()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(border())
        .text_color(muted())
        .opacity(0.45)
        .text_sm()
        .child(label.to_string())
}

fn btn_danger_armed(id: &'static str, label: &str) -> Stateful<Div> {
    div()
        .id(id)
        .cursor_pointer()
        .px_4()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(red())
        .text_color(red())
        .text_sm()
        .hover(|s| s.bg(red()).text_color(rgb(0x141b1e)))
        .child(label.to_string())
}

/// Pilihan kecil (preset tag, toggle): aksen hanya bila aktif.
fn btn_choice(id: &'static str, label: &str, active: bool) -> Stateful<Div> {
    let (bd, fg) = if active {
        (accent(), accent())
    } else {
        (border(), muted())
    };
    div()
        .id(id)
        .cursor_pointer()
        .px_3()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(bd)
        .text_color(fg)
        .text_sm()
        .hover(|s| s.border_color(if active { accent() } else { muted() }))
        .child(label.to_string())
}

fn section_title(label: &str) -> Div {
    div().text_sm().text_color(muted()).child(label.to_string())
}

fn card() -> Div {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .p_4()
        .rounded_lg()
        .bg(bg_card())
        .border_1()
        .border_color(border())
}

/// Baris label: nilai untuk kartu Instance.
fn kv(label: &str, value: String) -> Div {
    div()
        .flex()
        .items_baseline()
        .justify_between()
        .gap_3()
        .child(div().text_xs().text_color(muted()).child(label.to_string()))
        .child(div().text_sm().text_color(txt()).truncate().child(value))
}

// ---------- state ----------
struct RootView {
    cfg: AppConfig,
    manager: ContainerManager,
    state: ContainerState,
    volume_ok: bool,
    volume_mount: Option<String>,
    logs: Vec<String>,
    message: String,
    msg_error: bool,
    confirm_remove: bool,
    confirm_volume: bool,
    email_input: Entity<TextInput>,
    pass_input: Entity<TextInput>,
    name_input: Entity<TextInput>,
    image_input: Entity<TextInput>,
    volume_input: Entity<TextInput>,
    _subscriptions: Vec<Subscription>,
}

impl RootView {
    fn new(cx: &mut Context<Self>) -> Self {
        let cfg = AppConfig::load();
        let manager = ContainerManager::new(cfg.clone());
        let state = manager.status();
        let volume_ok = manager.volume_exists();
        let volume_mount = if volume_ok {
            manager.volume_mountpoint()
        } else {
            None
        };
        let logs = manager.logs(50).unwrap_or_default();
        let (message, msg_error) = match &state {
            ContainerState::Unavailable(reason) => (
                format!(
                    "Runtime tak tersedia: {}. Perbaiki lalu klik Refresh.",
                    reason
                ),
                true,
            ),
            _ => (
                "Siap. Status dibaca dari runtime container.".to_string(),
                false,
            ),
        };
        let email_input =
            cx.new(|cx| TextInput::new(cx, "f-email", &cfg.email, "nama@contoh.com", false));
        let pass_input =
            cx.new(|cx| TextInput::new(cx, "f-pass", &cfg.password, "wajib diisi", true));
        let name_input =
            cx.new(|cx| TextInput::new(cx, "f-name", &cfg.container_name, "pgadmin4", false));
        let image_input =
            cx.new(|cx| TextInput::new(cx, "f-image", &cfg.image, "registry/org/image:tag", false));
        let volume_input =
            cx.new(|cx| TextInput::new(cx, "f-vol", &cfg.volume_name, "pgadmin-data", false));
        // Setiap ketikan langsung tersimpan ke config (tanpa refresh status
        // yang berat; status di-refresh lewat aksi / tombol Refresh).
        let mut subscriptions = Vec::new();
        subscriptions.push(cx.observe(&email_input, |this, entity, cx| {
            this.cfg.email = entity.read(cx).text();
            this.cfg.save();
        }));
        subscriptions.push(cx.observe(&pass_input, |this, entity, cx| {
            this.cfg.password = entity.read(cx).text();
            this.cfg.save();
        }));
        subscriptions.push(cx.observe(&name_input, |this, entity, cx| {
            this.cfg.container_name = entity.read(cx).text();
            this.cfg.save();
        }));
        subscriptions.push(cx.observe(&image_input, |this, entity, cx| {
            this.cfg.image = entity.read(cx).text();
            this.cfg.save();
        }));
        subscriptions.push(cx.observe(&volume_input, |this, entity, cx| {
            this.cfg.volume_name = entity.read(cx).text();
            this.cfg.save();
        }));
        Self {
            cfg,
            manager,
            state,
            volume_ok,
            volume_mount,
            logs,
            message,
            msg_error,
            confirm_remove: false,
            confirm_volume: false,
            email_input,
            pass_input,
            name_input,
            image_input,
            volume_input,
            _subscriptions: subscriptions,
        }
    }

    /// Simpan config -> rebuild manager -> refresh status.
    fn sync(&mut self) {
        self.cfg.save();
        self.manager = ContainerManager::new(self.cfg.clone());
        self.state = self.manager.status();
        self.volume_ok = self.manager.volume_exists();
        self.volume_mount = if self.volume_ok {
            self.manager.volume_mountpoint()
        } else {
            None
        };
    }

    fn set_msg(&mut self, m: String, err: bool) {
        self.message = m;
        self.msg_error = err;
    }

    fn running(&self) -> bool {
        matches!(self.state, ContainerState::Running(_))
    }

    fn exists(&self) -> bool {
        !matches!(self.state, ContainerState::NotFound)
    }

    /// Runtime bisa dihubungi. Kalau tidak, semua aksi merusak/destruktif
    /// dinonaktifkan jujur — hanya Refresh yang tersisa sebagai jalan pulih.
    fn available(&self) -> bool {
        !matches!(self.state, ContainerState::Unavailable(_))
    }

    // ----- aksi container -----
    fn act_start(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        match self.manager.start() {
            Ok(m) => {
                self.set_msg(m, false);
                if self.cfg.auto_open_browser {
                    ContainerManager::open_browser(&self.cfg.url());
                }
            }
            Err(e) => self.set_msg(format!("Gagal start: {:#}", e), true),
        }
        self.sync();
        self.reload_logs();
        cx.notify();
    }

    fn act_stop(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        match self.manager.stop() {
            Ok(_) => self.set_msg("Container dihentikan.".to_string(), false),
            Err(e) => self.set_msg(format!("Gagal stop: {:#}", e), true),
        }
        self.sync();
        cx.notify();
    }

    fn act_restart(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        match self.manager.restart() {
            Ok(_) => self.set_msg("Container di-restart.".to_string(), false),
            Err(e) => self.set_msg(format!("Gagal restart: {:#}", e), true),
        }
        self.sync();
        self.reload_logs();
        cx.notify();
    }

    fn act_remove(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.confirm_remove {
            self.confirm_remove = true;
            self.set_msg(
                "Klik Hapus sekali lagi untuk konfirmasi. Data tetap aman di volume.".to_string(),
                false,
            );
            cx.notify();
            return;
        }
        self.confirm_remove = false;
        match self.manager.remove() {
            Ok(_) => self.set_msg(
                "Container dihapus. Data tetap aman di volume.".to_string(),
                false,
            ),
            Err(e) => self.set_msg(format!("Gagal hapus: {:#}", e), true),
        }
        self.sync();
        cx.notify();
    }

    fn act_open(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        ContainerManager::open_browser(&self.cfg.url());
        self.set_msg(format!("Membuka {}", self.cfg.url()), false);
        cx.notify();
    }

    fn act_refresh(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.sync();
        self.reload_logs();
        self.set_msg("Status diperbarui.".to_string(), false);
        cx.notify();
    }

    fn act_logs(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.reload_logs();
        self.set_msg("Log diperbarui.".to_string(), false);
        cx.notify();
    }

    fn reload_logs(&mut self) {
        self.logs = self.manager.logs(50).unwrap_or_default();
    }

    // ----- volume -----
    fn act_vol_up(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        match self.manager.ensure_volume() {
            Ok(m) => self.set_msg(m, false),
            Err(e) => self.set_msg(format!("Gagal siapkan volume: {:#}", e), true),
        }
        self.sync();
        cx.notify();
    }

    fn act_vol_rm(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.confirm_volume {
            self.confirm_volume = true;
            self.set_msg(
                "Menghapus volume = menghapus SEMUA data pgAdmin. Klik lagi untuk konfirmasi."
                    .to_string(),
                true,
            );
            cx.notify();
            return;
        }
        self.confirm_volume = false;
        match self.manager.remove_volume() {
            Ok(m) => self.set_msg(format!("{}. Data pgAdmin hilang.", m), false),
            Err(e) => self.set_msg(format!("Gagal hapus volume: {:#}", e), true),
        }
        self.sync();
        cx.notify();
    }

    // ----- config UI -----
    fn cycle_runtime(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.cfg.runtime = match self.cfg.runtime.as_str() {
            "auto" => "docker".to_string(),
            "docker" => "podman".to_string(),
            _ => "auto".to_string(),
        };
        self.confirm_remove = false;
        self.sync();
        self.set_msg(
            format!(
                "Runtime: {} (efektif: {})",
                self.cfg.runtime,
                self.manager.runtime.label()
            ),
            false,
        );
        cx.notify();
    }

    fn toggle_autopen(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.cfg.auto_open_browser = !self.cfg.auto_open_browser;
        self.sync();
        cx.notify();
    }

    fn preset_image(&mut self, tag: &str) {
        self.cfg.image = format!("docker.io/dpage/pgadmin4:{}", tag);
        self.sync();
    }

    // ----- render helpers -----
    /// Label + input. Wrapper flex_1: tiga field sebaris berbagi ruang sama,
    /// tinggi input fixed 34px dari TextInput (tidak ikut melar).
    fn labeled(&self, label: &str, input: &Entity<TextInput>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .flex_1()
            .child(div().text_xs().text_color(muted()).child(label.to_string()))
            .child(input.clone())
    }

    /// Dot status: menandai state nyata container (bukan dekorasi).
    fn status_row(&self) -> Div {
        let (color, text) = match &self.state {
            ContainerState::Running(d) => (green(), format!("Berjalan · {}", d)),
            ContainerState::Stopped => (amber(), "Berhenti".to_string()),
            ContainerState::NotFound => (muted(), "Belum dibuat".to_string()),
            ContainerState::Unavailable(_) => (red(), "Runtime tak tersedia".to_string()),
        };
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(div().size_2().rounded_full().bg(color))
            .child(div().text_sm().text_color(txt()).child(text))
            .child(
                div()
                    .text_sm()
                    .text_color(muted())
                    .child(format!("· {}", self.cfg.url())),
            )
    }

    fn instance_card(&mut self, cx: &mut Context<Self>) -> Div {
        let is_running = self.running();
        let exists = self.exists();
        let avail = self.available();
        card()
            .child(section_title("Instance"))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(kv("Container", self.cfg.container_name.clone()))
                    .child(kv("Image", self.cfg.image.clone()))
                    .child(kv("Port", self.cfg.host_port.to_string())),
            )
            .child(
                // aksi utama full-width: konteksnya jelas dari state.
                // runtime mati -> nonaktif jujur (jalan pulih: Refresh).
                if !avail {
                    btn_off("act-primary", if is_running { "Stop" } else { "Start" })
                        .w_full()
                        .justify_center()
                } else if is_running {
                    btn_primary_neutral("act-primary", "Stop")
                        .w_full()
                        .justify_center()
                        .on_click(cx.listener(Self::act_stop))
                } else {
                    btn_primary("act-primary", "Start")
                        .w_full()
                        .justify_center()
                        .on_click(cx.listener(Self::act_start))
                },
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(div().flex_1().child(if avail && exists {
                        btn_ghost("act-restart", "Restart")
                            .w_full()
                            .justify_center()
                            .on_click(cx.listener(Self::act_restart))
                    } else {
                        btn_off("act-restart", "Restart").w_full().justify_center()
                    }))
                    .child(div().flex_1().child(if avail && exists {
                        if self.confirm_remove {
                            btn_danger_armed("act-remove", "Konfirmasi hapus")
                                .w_full()
                                .justify_center()
                                .on_click(cx.listener(Self::act_remove))
                        } else {
                            btn_ghost("act-remove", "Hapus")
                                .w_full()
                                .justify_center()
                                .on_click(cx.listener(Self::act_remove))
                        }
                    } else {
                        btn_off("act-remove", "Hapus").w_full().justify_center()
                    })),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(div().flex_1().child(if is_running {
                        btn_ghost("act-browser", "Buka Browser")
                            .w_full()
                            .justify_center()
                            .on_click(cx.listener(Self::act_open))
                    } else {
                        btn_off("act-browser", "Buka Browser")
                            .w_full()
                            .justify_center()
                    }))
                    .child(
                        div().flex_1().child(
                            btn_ghost("act-refresh", "Refresh")
                                .w_full()
                                .justify_center()
                                .on_click(cx.listener(Self::act_refresh)),
                        ),
                    ),
            )
    }

    fn data_card(&mut self, cx: &mut Context<Self>) -> Div {
        let avail = self.available();
        let vol_note = if self.volume_ok {
            format!("Data persist di volume {}.", self.cfg.volume_name)
        } else {
            format!(
                "Volume {} belum ada — siapkan dulu agar data persist.",
                self.cfg.volume_name
            )
        };
        let mount_line = self.volume_mount.clone();
        card()
            .child(section_title("Data"))
            .child(div().text_sm().text_color(muted()).child(vol_note))
            .child(
                div()
                    .text_xs()
                    .text_color(muted())
                    .child(mount_line.unwrap_or_default()),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().child(if avail {
                        btn_ghost("act-vol-up", "Siapkan Volume")
                            .w_full()
                            .justify_center()
                            .on_click(cx.listener(Self::act_vol_up))
                    } else {
                        btn_off("act-vol-up", "Siapkan Volume")
                            .w_full()
                            .justify_center()
                    }))
                    .child(div().child(if !avail {
                        btn_off("act-vol-rm", "Hapus Volume")
                            .w_full()
                            .justify_center()
                    } else if self.confirm_volume {
                        btn_danger_armed("act-vol-rm", "Konfirmasi hapus volume")
                            .w_full()
                            .justify_center()
                            .on_click(cx.listener(Self::act_vol_rm))
                    } else {
                        btn_ghost("act-vol-rm", "Hapus Volume")
                            .w_full()
                            .justify_center()
                            .on_click(cx.listener(Self::act_vol_rm))
                    })),
            )
    }

    fn config_card(&mut self, cx: &mut Context<Self>) -> Div {
        let port = self.cfg.host_port;
        let autopen = self.cfg.auto_open_browser;

        card()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(section_title("Konfigurasi"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted())
                            .child("tersimpan otomatis".to_string()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .gap_2()
                    .child(div().text_sm().child("Port".to_string()))
                    .child(btn_ghost("port-dec", "−").on_click(cx.listener(
                        |this: &mut Self,
                         _: &ClickEvent,
                         _: &mut Window,
                         cx: &mut Context<Self>| {
                            this.cfg.host_port = this.cfg.host_port.saturating_sub(1).max(1);
                            this.sync();
                            cx.notify();
                        },
                    )))
                    .child(
                        div()
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(bg_field())
                            .border_1()
                            .border_color(border())
                            .text_sm()
                            .child(port.to_string()),
                    )
                    .child(btn_ghost("port-inc", "+").on_click(cx.listener(
                        |this: &mut Self,
                         _: &ClickEvent,
                         _: &mut Window,
                         cx: &mut Context<Self>| {
                            this.cfg.host_port = this.cfg.host_port.saturating_add(1);
                            this.sync();
                            cx.notify();
                        },
                    )))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .ml_2()
                            .child(
                                btn_choice(
                                    "tag-latest",
                                    "latest",
                                    self.cfg.image.ends_with(":latest"),
                                )
                                .on_click(cx.listener(
                                    |this: &mut Self,
                                     _: &ClickEvent,
                                     _: &mut Window,
                                     cx: &mut Context<Self>| {
                                        this.preset_image("latest");
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                btn_choice("tag-9", "9", self.cfg.image.ends_with(":9"))
                                    .on_click(cx.listener(
                                    |this: &mut Self,
                                     _: &ClickEvent,
                                     _: &mut Window,
                                     cx: &mut Context<Self>| {
                                        this.preset_image("9");
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                btn_choice("tag-8", "8", self.cfg.image.ends_with(":8"))
                                    .on_click(cx.listener(
                                    |this: &mut Self,
                                     _: &ClickEvent,
                                     _: &mut Window,
                                     cx: &mut Context<Self>| {
                                        this.preset_image("8");
                                        cx.notify();
                                    },
                                )),
                            ),
                    )
                    .child(
                        btn_choice(
                            "cfg-autoopen",
                            if autopen {
                                "Auto-open: on"
                            } else {
                                "Auto-open: off"
                            },
                            autopen,
                        )
                        .on_click(cx.listener(Self::toggle_autopen)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.labeled("Email", &self.email_input.clone()))
                    .child(self.labeled("Password", &self.pass_input.clone())),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.labeled("Nama container", &self.name_input.clone()))
                    .child(self.labeled("Nama volume", &self.volume_input.clone())),
            )
            .child(self.labeled("Image", &self.image_input.clone()))
    }

    fn log_card(&mut self, cx: &mut Context<Self>) -> Div {
        let log_start = self.logs.len().saturating_sub(30);
        let log_lines: Vec<String> = self.logs[log_start..].to_vec();
        card()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(section_title("Log"))
                    .child(
                        btn_ghost("act-logs", "Muat ulang").on_click(cx.listener(Self::act_logs)),
                    ),
            )
            .child(
                div()
                    .id("logs")
                    .flex()
                    .flex_col()
                    .gap_1()
                    .h_72()
                    .overflow_scroll()
                    .p_3()
                    .rounded_md()
                    .bg(bg_field())
                    .border_1()
                    .border_color(border())
                    .children(log_lines.into_iter().map(|l| {
                        div().text_xs().text_color(muted()).child(if l.len() > 160 {
                            format!("{}…", &l[..160])
                        } else {
                            l
                        })
                    }))
                    .when(self.logs.is_empty(), |s| {
                        s.child(div().text_xs().text_color(muted()).child(
                            "Belum ada log — start container untuk melihat output.".to_string(),
                        ))
                    }),
            )
    }
}

impl Render for RootView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let runtime_cfg = self.cfg.runtime.clone();
        let runtime_fx = self.manager.runtime.label().to_string();

        div()
            .id("root")
            .size_full()
            .flex()
            .flex_col()
            .bg(bg_app())
            .text_color(txt())
            .font_family("Iosevka Nerd Font")
            .p_5()
            .gap_4()
            .overflow_scroll()
            // header
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(border())
                    .pb_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("pgAdmin Manager".to_string()),
                            )
                            .child(self.status_row()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted())
                                    .child(format!("{} ({})", runtime_cfg, runtime_fx)),
                            )
                            .child(
                                btn_ghost("act-runtime", "Ganti")
                                    .on_click(cx.listener(Self::cycle_runtime)),
                            ),
                    ),
            )
            // konten dua kolom: kiri kontrol, kanan konfigurasi + log
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .w(px(340.))
                            .child(self.instance_card(cx))
                            .child(self.data_card(cx)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .flex_1()
                            .child(self.config_card(cx))
                            .child(self.log_card(cx)),
                    ),
            )
            // pesan status: teks polos, merah hanya saat error
            .child(
                div()
                    .text_xs()
                    .text_color(if self.msg_error { red() } else { muted() })
                    .child(self.message.clone()),
            )
    }
}

fn main() {
    Application::new().run(|cx| {
        // Shortcut umum aktif hanya dalam konteks TextInput (Linux pakai
        // ctrl, macOS pakai cmd — daftarkan keduanya).
        cx.bind_keys([
            KeyBinding::new("backspace", Backspace, Some("TextInput")),
            KeyBinding::new("delete", Delete, Some("TextInput")),
            KeyBinding::new("left", Left, Some("TextInput")),
            KeyBinding::new("right", Right, Some("TextInput")),
            KeyBinding::new("shift-left", SelectLeft, Some("TextInput")),
            KeyBinding::new("shift-right", SelectRight, Some("TextInput")),
            KeyBinding::new("home", Home, Some("TextInput")),
            KeyBinding::new("end", End, Some("TextInput")),
            KeyBinding::new("ctrl-a", SelectAll, Some("TextInput")),
            KeyBinding::new("cmd-a", SelectAll, Some("TextInput")),
            KeyBinding::new("ctrl-c", Copy, Some("TextInput")),
            KeyBinding::new("cmd-c", Copy, Some("TextInput")),
            KeyBinding::new("ctrl-x", Cut, Some("TextInput")),
            KeyBinding::new("cmd-x", Cut, Some("TextInput")),
            KeyBinding::new("ctrl-v", Paste, Some("TextInput")),
            KeyBinding::new("cmd-v", Paste, Some("TextInput")),
        ]);
        let bounds = WindowBounds::Windowed(Bounds::centered(None, size(px(1240.), px(820.)), cx));
        cx.open_window(
            WindowOptions {
                window_bounds: Some(bounds),
                titlebar: Some(TitlebarOptions {
                    title: Some("pgAdmin Manager".into()),
                    appears_transparent: false,
                    traffic_light_position: None,
                }),
                ..Default::default()
            },
            |_, cx| cx.new(RootView::new),
        )
        .unwrap();
    });
}
