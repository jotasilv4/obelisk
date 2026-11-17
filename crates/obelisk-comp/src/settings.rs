//! Configurações persistentes em `~/.config/obelisk/settings` (formato `chave=valor`).

use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub appearance: String, // "dark" | "light" | "auto"
    pub dock_magnify: bool,
    pub hot_corners: bool,
    pub snapping: bool,
    pub lock_minutes: u64,
    pub cmd_as_ctrl: bool,
    pub game_auto: bool,
    pub widgets: bool,
    pub notif_banners: bool,
    pub animations: bool,
    pub accent: String,
    /// demais chaves das páginas dos Ajustes (layout do teclado, papel de parede, timers…)
    pub extra: std::collections::BTreeMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            appearance: "dark".into(),
            dock_magnify: true,
            hot_corners: true,
            snapping: true,
            lock_minutes: 10,
            cmd_as_ctrl: true,
            game_auto: true,
            widgets: false,
            notif_banners: true,
            animations: true,
            accent: "blue".into(),
            extra: Default::default(),
        }
    }
}

fn file() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("obelisk/settings"))
}

impl Settings {
    pub fn load() -> Self {
        let mut s = Settings::default();
        let Some(text) = file().and_then(|p| std::fs::read_to_string(p).ok()) else { return s };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            let b = v == "true";
            match k {
                "appearance" => s.appearance = v.to_string(),
                "dock_magnify" => s.dock_magnify = b,
                "hot_corners" => s.hot_corners = b,
                "snapping" => s.snapping = b,
                "lock_minutes" => s.lock_minutes = v.parse().unwrap_or(10),
                "cmd_as_ctrl" => s.cmd_as_ctrl = b,
                "game_auto" => s.game_auto = b,
                "widgets" => s.widgets = b,
                "notif_banners" => s.notif_banners = b,
                "animations" => s.animations = b,
                "accent" => s.accent = v.to_string(),
                _ => {
                    s.extra.insert(k.to_string(), v.to_string());
                }
            }
        }
        s
    }

    pub fn save(&self) {
        let Some(p) = file() else { return };
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        let text = format!(
            "# Ajustes do Obelisk (editável à mão)\nappearance={}\ndock_magnify={}\nhot_corners={}\nsnapping={}\nlock_minutes={}\ncmd_as_ctrl={}\ngame_auto={}\nwidgets={}\nnotif_banners={}\nanimations={}\naccent={}\n",
            self.appearance, self.dock_magnify, self.hot_corners, self.snapping, self.lock_minutes, self.cmd_as_ctrl, self.game_auto, self.widgets, self.notif_banners, self.animations, self.accent
        );
        let mut text = text;
        for (k, v) in &self.extra {
            text.push_str(&format!("{k}={v}\n"));
        }
        let _ = std::fs::write(p, text);
    }
}

impl Settings {
    pub fn get(&self, k: &str) -> Option<&str> {
        self.extra.get(k).map(|s| s.as_str())
    }

    /// Todas as chaves como texto (para as páginas dos Ajustes).
    pub fn as_map(&self) -> std::collections::HashMap<String, String> {
        let mut m: std::collections::HashMap<String, String> = self.extra.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        m.insert("appearance".into(), self.appearance.clone());
        m.insert("accent".into(), self.accent.clone());
        m.insert("dock_magnify".into(), self.dock_magnify.to_string());
        m.insert("hot_corners".into(), self.hot_corners.to_string());
        m.insert("snapping".into(), self.snapping.to_string());
        m.insert("lock_minutes".into(), self.lock_minutes.to_string());
        m.insert("cmd_as_ctrl".into(), self.cmd_as_ctrl.to_string());
        m.insert("game_auto".into(), self.game_auto.to_string());
        m.insert("widgets".into(), self.widgets.to_string());
        m.insert("notif_banners".into(), self.notif_banners.to_string());
        m.insert("animations".into(), self.animations.to_string());
        m
    }

    /// Grava uma chave (tipada ou extra).
    pub fn set(&mut self, k: &str, v: &str) {
        let b = v == "true";
        match k {
            "appearance" => self.appearance = v.into(),
            "accent" => self.accent = v.into(),
            "dock_magnify" => self.dock_magnify = b,
            "hot_corners" => self.hot_corners = b,
            "snapping" => self.snapping = b,
            "lock_minutes" => self.lock_minutes = v.parse().unwrap_or(10),
            "cmd_as_ctrl" => self.cmd_as_ctrl = b,
            "game_auto" => self.game_auto = b,
            "widgets" => self.widgets = b,
            "notif_banners" => self.notif_banners = b,
            "animations" => self.animations = b,
            _ => {
                self.extra.insert(k.into(), v.into());
            }
        }
    }
}
