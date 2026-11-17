//! "Sobre o App" (menu do app): janela pequena com ícone, nome, versão, descrição e pacote,
//! como o painel "Sobre" dos apps do macOS. Versão vem do pacman (ou do Flatpak).

use resvg::tiny_skia::Pixmap;

use super::layer::Layer;
use crate::ui::canvas::Canvas;
use crate::ui::text::Weight;
use crate::ui::theme::Theme;

const W: f32 = 300.0;
const SHADOW: f32 = 44.0;

#[derive(Default)]
pub struct AboutApp {
    pub open: bool,
    pub name: String,
    pub version: String,
    pub comment: String,
    pub package: String,
    pub icon: Option<Pixmap>,
    pub layer: Layer,
    pub pos: (f32, f32),
    pub size: (f32, f32),
    ok_rect: (f32, f32, f32, f32),
    close_rect: (f32, f32, f32, f32),
    key: Option<String>,
}

/// Versão e pacote do app: `pacman -Qo` do executável, ou `flatpak info`.
pub fn app_version(exec: &str) -> (String, String) {
    use crate::shell::settings_model::{q, sh};
    let words: Vec<&str> = exec.split_whitespace().collect();
    if let Some(pos) = words.iter().position(|w| *w == "run") {
        if words.first().is_some_and(|w| w.ends_with("flatpak")) {
            if let Some(id) = words.iter().skip(pos + 1).find(|w| !w.starts_with('-')) {
                let info = sh(&format!("flatpak info {}", q(id)));
                let ver = info.lines().find_map(|l| l.trim().strip_prefix("Version:")).unwrap_or("").trim().to_string();
                return (ver, format!("Flatpak · {id}"));
            }
        }
    }
    let bin = words.iter().find(|w| !w.contains('=')).copied().unwrap_or("");
    let path = if bin.starts_with('/') { bin.to_string() } else { sh(&format!("command -v {}", q(bin))).trim().to_string() };
    if path.is_empty() {
        return (String::new(), String::new());
    }
    // "/usr/bin/foot is owned by foot 1.21.0-1" (ou em português: "pertence a")
    let out = sh(&format!("LANG=C pacman -Qo {}", q(&path)));
    let tail = out.split(" by ").nth(1).unwrap_or("").trim().to_string();
    let mut it = tail.split_whitespace();
    let pkg = it.next().unwrap_or("").to_string();
    let ver = it.next().unwrap_or("").to_string();
    (ver, if pkg.is_empty() { String::new() } else { format!("Pacote {pkg}") })
}

impl AboutApp {
    pub fn new(name: String, version: String, comment: String, package: String, icon: Option<Pixmap>) -> Self {
        AboutApp { open: true, name, version, comment, package, icon, ..Default::default() }
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        self.open && (x as f32) >= self.pos.0 && (x as f32) < self.pos.0 + self.size.0 && (y as f32) >= self.pos.1 && (y as f32) < self.pos.1 + self.size.1
    }

    /// `true` se o clique foi no OK ou no fechar.
    pub fn closes(&self, x: f64, y: f64) -> bool {
        let (x, y) = (x as f32, y as f32);
        [self.ok_rect, self.close_rect].iter().any(|r| x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3)
    }

    pub fn update(&mut self, theme: &Theme, sw: i32, sh: i32, scale: i32, backdrop: Option<&Pixmap>) {
        let key = format!("{}|{}|{}|{sw}|{scale}", self.name, self.version, theme.appearance as u8);
        let comment_lines = if self.comment.is_empty() { 0 } else { 2 };
        let h = 40.0 + 96.0 + 16.0 + 26.0 + 20.0 + comment_lines as f32 * 16.0 + 18.0 + 18.0 + 24.0 + 26.0 + 20.0;
        self.size = (W, h);
        self.pos = (((sw as f32 - W) / 2.0).round(), ((sh as f32 - h) / 3.0).round());
        if self.key.as_deref() == Some(key.as_str()) && self.layer.is_set() {
            return;
        }
        self.key = Some(key);
        let (sx, sy) = (self.pos.0 - SHADOW, self.pos.1 - SHADOW);
        let mut c = Canvas::new((W + 2.0 * SHADOW) as i32, (h + 2.0 * SHADOW) as i32, scale as f32).unwrap();
        let (ox, oy) = (SHADOW, SHADOW);
        c.shadow_rrect(ox, oy + 14.0, W, h, 12.0, 40.0, 130);
        if let Some(bd) = backdrop {
            c.fill_rrect_backdrop(ox, oy, W, h, 12.0, bd, (sx, sy), (sw as f32, sh as f32));
        }
        c.fill_rrect(ox, oy, W, h, 12.0, theme.window_bg);
        c.stroke_rrect(ox, oy, W, h, 12.0, 1.0, theme.fg(0.12));
        for (i, col) in [theme.tl_close, theme.fg(0.15), theme.fg(0.15)].iter().enumerate() {
            c.fill_circle(ox + 18.0 + i as f32 * 20.0, oy + 18.0, 6.0, *col);
        }
        self.close_rect = (self.pos.0 + 10.0, self.pos.1 + 10.0, 16.0, 16.0);
        let cx = ox + W / 2.0;
        let mut y = oy + 40.0;
        if let Some(pm) = &self.icon {
            c.blit(pm, cx - 48.0, y, 96.0, 96.0, 1.0);
        }
        y += 96.0 + 16.0;
        let center = |c: &mut Canvas, t: &str, y: f32, size: f32, wt: Weight, col| {
            let t = c.ellipsized(t, size, wt, W - 40.0);
            let w = c.measure(&t, size, wt);
            c.text(&t, cx - w / 2.0, y, size, wt, col);
        };
        center(&mut c, &self.name, y, 20.0, Weight::Bold, theme.label);
        y += 26.0;
        let ver = if self.version.is_empty() { "Versão desconhecida".to_string() } else { format!("Versão {}", self.version) };
        center(&mut c, &ver, y, 12.0, Weight::Regular, theme.secondary);
        y += 20.0;
        if !self.comment.is_empty() {
            for line in super::notch::wrap(&self.comment, W - 48.0, 2, &c) {
                center(&mut c, &line, y, 12.0, Weight::Regular, theme.label);
                y += 16.0;
            }
        }
        y += 18.0;
        if !self.package.is_empty() {
            center(&mut c, &self.package, y, 11.0, Weight::Regular, theme.tertiary);
        }
        y += 18.0 + 24.0 - 18.0;
        let bw = 80.0;
        let (bx, by) = (cx - bw / 2.0, y);
        c.fill_rrect(bx, by, bw, 26.0, 7.0, theme.accent);
        let tw = c.measure("OK", 13.0, Weight::Medium);
        c.text_vcenter("OK", cx - tw / 2.0, by, 26.0, 13.0, Weight::Medium, theme.text_on_accent);
        self.ok_rect = (bx - ox + self.pos.0, by - oy + self.pos.1, bw, 26.0);
        self.layer.set(&c);
    }
}
