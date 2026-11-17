//! Estado do sistema para a barra e a Central de Controle.
//! Lido numa thread à parte (a cada 2 s) com ferramentas padrão: nmcli, bluetoothctl,
//! wpctl (PipeWire), brightnessctl e playerctl. Sem a ferramenta, o item fica oculto.

use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Media {
    pub playing: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// capa do álbum como arquivo local (baixada/decodificada pela thread)
    pub art: String,
    /// nome do player (spotify, firefox, mpv…) para o selo no notch
    pub player: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SysStatus {
    pub wifi_available: bool,
    pub wired: bool,
    pub wifi_on: bool,
    pub ssid: Option<String>,
    pub signal: u8,
    pub bt_available: bool,
    pub bt_on: bool,
    pub volume: Option<f32>,
    pub muted: bool,
    pub brightness: Option<f32>,
    pub media: Option<Media>,
    pub battery: Option<(u8, bool)>,
    /// aparelhos Bluetooth conectados (nome)
    pub bt_connected: Vec<String>,
}

/// Roda uma ferramenta com limite de 1,5 s (ex.: `bluetoothctl` trava sem controlador).
fn run(cmd: &str, args: &[&str]) -> Option<String> {
    use std::io::Read;
    let mut child = Command::new(cmd).args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut out = String::new();
                child.stdout.take()?.read_to_string(&mut out).ok()?;
                return status.success().then_some(out);
            }
            Ok(None) if start.elapsed() < Duration::from_millis(1500) => std::thread::sleep(Duration::from_millis(15)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// Capa do álbum como arquivo local: file:// direto; http(s) baixado; data: decodificado.
/// Guarda a última para não baixar de novo a cada consulta.
fn local_art(url: &str) -> String {
    static LAST: Mutex<(String, String)> = Mutex::new((String::new(), String::new()));
    if url.is_empty() {
        return String::new();
    }
    if let Some(p) = url.strip_prefix("file://") {
        return percent_decode(p);
    }
    if let Ok(last) = LAST.lock() {
        if last.0 == url {
            return last.1.clone();
        }
    }
    let dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let n = url.len() ^ url.bytes().fold(0usize, |a, b| a.wrapping_mul(31).wrapping_add(b as usize));
    let path = format!("{dir}/obelisk-art-{n:x}");
    let ok = if url.starts_with("http://") || url.starts_with("https://") {
        Command::new("curl").args(["-fsSL", "--max-time", "4", "-o", &path, url]).stdin(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
    } else if let Some(b64) = url.strip_prefix("data:").and_then(|d| d.split_once(',').map(|x| x.1)) {
        use std::io::Write;
        let child = Command::new("base64").args(["-d"]).stdin(Stdio::piped()).stdout(std::fs::File::create(&path).map(Stdio::from).unwrap_or(Stdio::null())).spawn();
        match child {
            Ok(mut c) => {
                if let Some(mut i) = c.stdin.take() {
                    let _ = i.write_all(b64.as_bytes());
                }
                c.wait().is_ok_and(|s| s.success())
            }
            Err(_) => false,
        }
    } else {
        false
    };
    let out = if ok { path } else { String::new() };
    if let Ok(mut last) = LAST.lock() {
        *last = (url.to_string(), out.clone());
    }
    out
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn poll() -> SysStatus {
    let mut s = SysStatus::default();
    if let Some(r) = run("nmcli", &["-t", "-f", "WIFI", "radio"]) {
        s.wifi_available = true;
        s.wifi_on = r.trim() == "enabled";
        if let Some(list) = run("nmcli", &["-t", "-f", "ACTIVE,SSID,SIGNAL", "dev", "wifi"]) {
            for l in list.lines() {
                let parts: Vec<&str> = l.splitn(3, ':').collect();
                if parts.len() == 3 && parts[0] == "yes" {
                    s.ssid = Some(parts[1].replace("\\:", ":"));
                    s.signal = parts[2].parse().unwrap_or(0);
                }
            }
        }
    }
    if let Some(r) = run("nmcli", &["-t", "-f", "TYPE,STATE", "dev"]) {
        s.wired = r.lines().any(|l| l.starts_with("ethernet:connected"));
    }
    if let Some(r) = run("bluetoothctl", &["show"]) {
        s.bt_available = r.contains("Controller");
        s.bt_on = r.lines().any(|l| l.trim() == "Powered: yes");
        if s.bt_on {
            if let Some(list) = run("bluetoothctl", &["devices", "Connected"]) {
                s.bt_connected = list.lines().filter_map(|l| l.splitn(3, ' ').nth(2)).map(str::to_string).collect();
            }
        }
    }
    if let Some(r) = run("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"]) {
        s.volume = r.split_whitespace().nth(1).and_then(|v| v.parse().ok());
        s.muted = r.contains("MUTED");
    }
    if let Some(r) = run("brightnessctl", &["-m", "-c", "backlight"]) {
        // nome,classe,atual,pct%,max
        let f: Vec<&str> = r.trim().split(',').collect();
        if f.len() >= 5 {
            if let (Ok(c), Ok(m)) = (f[2].parse::<f32>(), f[4].parse::<f32>()) {
                if m > 0.0 {
                    s.brightness = Some(c / m);
                }
            }
        }
    }
    if let Some(r) = run("playerctl", &["metadata", "--format", "{{status}}\t{{artist}}\t{{title}}\t{{album}}\t{{playerName}}\t{{mpris:artUrl}}"]) {
        let f: Vec<&str> = r.trim_end_matches('\n').splitn(6, '\t').collect();
        if f.len() >= 3 && !f[2].is_empty() {
            let url = f.get(5).copied().unwrap_or("");
            s.media = Some(Media {
                playing: f[0] == "Playing",
                artist: f[1].into(),
                title: f[2].into(),
                album: f.get(3).copied().unwrap_or("").into(),
                player: f.get(4).copied().unwrap_or("").into(),
                art: local_art(url),
            });
        }
    }
    s.battery = crate::shell::menubar::battery();
    s
}

#[derive(Clone)]
pub struct Services {
    pub status: Arc<Mutex<SysStatus>>,
    /// Modo Jogo: a thread para de rodar ferramentas (nada consultando o sistema à toa).
    pub paused: Arc<std::sync::atomic::AtomicBool>,
}

impl Services {
    pub fn start() -> Self {
        let status = Arc::new(Mutex::new(SysStatus::default()));
        let paused = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (st, pz) = (status.clone(), paused.clone());
        std::thread::Builder::new()
            .name("obelisk-services".into())
            .spawn(move || loop {
                if !pz.load(std::sync::atomic::Ordering::Relaxed) {
                    let s = poll();
                    *st.lock().unwrap() = s;
                }
                std::thread::sleep(Duration::from_secs(3));
            })
            .ok();
        Services { status, paused }
    }

    pub fn set_paused(&self, on: bool) {
        self.paused.store(on, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn get(&self) -> SysStatus {
        self.status.lock().unwrap().clone()
    }

    fn update(&self, f: impl FnOnce(&mut SysStatus)) {
        f(&mut self.status.lock().unwrap());
    }

    pub fn set_wifi(&self, on: bool) {
        crate::apps::spawn(&format!("nmcli radio wifi {}", if on { "on" } else { "off" }));
        self.update(|s| s.wifi_on = on);
    }

    pub fn set_bluetooth(&self, on: bool) {
        crate::apps::spawn(&format!("bluetoothctl power {}", if on { "on" } else { "off" }));
        self.update(|s| s.bt_on = on);
    }

    pub fn set_volume(&self, v: f32) {
        let v = v.clamp(0.0, 1.0);
        crate::apps::spawn(&format!("wpctl set-volume @DEFAULT_AUDIO_SINK@ {v:.2}"));
        self.update(|s| s.volume = Some(v));
    }

    pub fn toggle_mute(&self) {
        crate::apps::spawn("wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle");
        self.update(|s| s.muted = !s.muted);
    }

    pub fn volume_step(&self, delta: f32) -> f32 {
        let cur = self.get().volume.unwrap_or(0.5);
        let v = (cur + delta).clamp(0.0, 1.0);
        self.set_volume(v);
        v
    }

    pub fn brightness_step(&self, delta: f32) -> f32 {
        let cur = self.get().brightness.unwrap_or(0.5);
        let v = (cur + delta).clamp(0.02, 1.0);
        self.set_brightness(v);
        v
    }

    pub fn set_brightness(&self, v: f32) {
        let pct = (v.clamp(0.02, 1.0) * 100.0).round();
        crate::apps::spawn(&format!("brightnessctl -q -c backlight set {pct}%"));
        self.update(|s| s.brightness = Some(v));
    }

    pub fn media(&self, action: &str) {
        crate::apps::spawn(&format!("playerctl {action}"));
        if action == "play-pause" {
            self.update(|s| {
                if let Some(m) = &mut s.media {
                    m.playing = !m.playing;
                }
            });
        }
    }
}
