//! Notch (docs/03-especificacao-kyro.md §12), no estilo do boring.notch/NotchNook:
//! silhueta preta com orelhas côncavas, tamanhos animados por molas do SwiftUI.
//!
//! Modos (prioridade): aberto 640×168 · carregando 393×32 · HUD 377×32 · música 261×32 · ocioso 185×32.
//! Desempenho: só redesenha enquanto anima; CPU/RAM só são lidos com o notch aberto; no Modo Jogo
//! vira uma pílula preta estática (sem visualizador nem relógio de animação).

use std::time::{Duration, Instant};

use resvg::tiny_skia::{Path, PathBuilder};

use super::layer::Layer;
use crate::services::SysStatus;
use crate::ui::canvas::Canvas;
use crate::ui::spring::Spring;
use crate::ui::text::Weight;
use crate::ui::theme::{Rgba, Theme};

pub const CLOSED_W: f32 = 185.0;
pub const H: f32 = 32.0;
pub const OPEN_W: f32 = 640.0;
pub const OPEN_H: f32 = 168.0;
const MAX_EAR: f32 = 19.0;
const CANVAS_W: f32 = OPEN_W + 2.0 * MAX_EAR + 2.0;
const CANVAS_H: f32 = OPEN_H + 2.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HudKind {
    Volume,
    Brightness,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Idle,
    Music,
    Hud(HudKind, f32),
    Charging(u8),
    Recording,
    Notif,
    Open,
    Auth,
    /// cartão da captura de tela (8 s; pausa com o mouse em cima)
    Capture,
    /// arquivo sendo arrastado sobre o notch: Bandeja | Copiar
    Drop,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Tab {
    #[default]
    Home,
    Tray,
}

/// Dados de fora que o notch mostra (captura, arrasto, bandeja, selo do player).
#[derive(Default)]
pub struct NotchExtra<'a> {
    /// (caminho, miniatura, deu certo)
    pub capture: Option<(&'a str, Option<&'a resvg::tiny_skia::Pixmap>, bool)>,
    /// arrasto em curso sobre o notch: fração x do ponteiro dentro do corpo
    pub drop: Option<f32>,
    pub shelf: &'a [String],
    /// ícone do player (selo no canto da capa)
    pub badge: Option<&'a resvg::tiny_skia::Pixmap>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NotchHit {
    Body,
    MediaPrev,
    MediaPlay,
    MediaNext,
    GameMode,
    NotifBody,
    NotifAction(usize),
    NotifClose,
    StopRecording,
    AuthOk,
    AuthCancel,
    TabHome,
    TabTray,
    ShelfItem(usize),
    ShelfClear,
    CaptureOpen,
    CaptureCopy,
    CaptureAnnotate,
    CaptureSaveAs,
    CaptureClose,
}

pub struct Notch {
    pub layer: Layer,
    w: Spring,
    h: Spring,
    ear: Spring,
    radius: Spring,
    content: Spring,
    shown_mode: Mode,
    pub hovered_since: Option<Instant>,
    pub left_at: Option<Instant>,
    pub open: bool,
    hud: Option<(HudKind, f32, Instant)>,
    charging: Option<(u8, Instant)>,
    last_charging: Option<bool>,
    last: Instant,
    /// Posição do canvas na tela (x) e retângulo do corpo (tela) para hit-test.
    pub buf_x: f32,
    pub body: (f32, f32, f32, f32),
    hits: Vec<(NotchHit, (f32, f32, f32, f32))>,
    stats: Option<(f32, f32, Instant)>,
    cpu_prev: Option<(u64, u64)>,
    key: Option<String>,
    epoch: Instant,
    pub tab: Tab,
    /// capa do álbum carregada: (arquivo, imagem, cor dominante)
    art: Option<(String, resvg::tiny_skia::Pixmap, Rgba)>,
}

impl Default for Notch {
    fn default() -> Self {
        Notch {
            layer: Layer::default(),
            w: Spring::new(CLOSED_W, 0.45, 1.0),
            h: Spring::new(H, 0.45, 1.0),
            ear: Spring::new(6.0, 0.45, 1.0),
            radius: Spring::new(14.0, 0.45, 1.0),
            content: Spring::new(1.0, 0.4, 1.0),
            shown_mode: Mode::Idle,
            hovered_since: None,
            left_at: None,
            open: false,
            hud: None,
            charging: None,
            last_charging: None,
            last: Instant::now(),
            buf_x: 0.0,
            body: (0.0, 0.0, 0.0, 0.0),
            hits: Vec::new(),
            stats: None,
            cpu_prev: None,
            key: None,
            epoch: Instant::now(),
            tab: Tab::Home,
            art: None,
        }
    }
}

/// Silhueta do notch (DynamicNotchKit): curvas quadráticas com o controle no canto.
fn shape(x: f32, body_w: f32, body_h: f32, radius: f32, ear: f32) -> Option<Path> {
    let h = body_h.max(ear + 2.0);
    let r = radius.min(h - ear).min(body_w / 2.0).max(0.0);
    let rx = x + ear + body_w;
    let width = body_w + 2.0 * ear;
    let mut pb = PathBuilder::new();
    pb.move_to(x, 0.0);
    pb.quad_to(x + ear, 0.0, x + ear, ear);
    pb.line_to(x + ear, h - r);
    pb.quad_to(x + ear, h, x + ear + r, h);
    pb.line_to(rx - r, h);
    pb.quad_to(rx, h, rx, h - r);
    pb.line_to(rx, ear);
    pb.quad_to(rx, 0.0, x + width, 0.0);
    pb.close();
    pb.finish()
}

fn read_cpu() -> Option<(u64, u64)> {
    let s = std::fs::read_to_string("/proc/stat").ok()?;
    let f: Vec<u64> = s.lines().next()?.split_whitespace().skip(1).filter_map(|v| v.parse().ok()).collect();
    let idle = f.get(3)? + f.get(4).unwrap_or(&0);
    Some((f.iter().sum(), idle))
}

fn read_ram() -> Option<f32> {
    let s = std::fs::read_to_string("/proc/meminfo").ok()?;
    let get = |k: &str| s.lines().find(|l| l.starts_with(k)).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse::<f32>().ok());
    let (total, avail) = (get("MemTotal:")?, get("MemAvailable:")?);
    Some((total - avail) / 1024.0 / 1024.0)
}

impl Notch {
    pub fn show_hud(&mut self, kind: HudKind, value: f32) {
        self.hud = Some((kind, value.clamp(0.0, 1.0), Instant::now()));
    }

    /// Chamado a cada quadro com o estado do sistema: detecta a ligação do carregador.
    pub fn observe(&mut self, st: &SysStatus) {
        if let Some((pct, charging)) = st.battery {
            if self.last_charging == Some(false) && charging {
                self.charging = Some((pct, Instant::now()));
            }
            self.last_charging = Some(charging);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn mode(&self, st: &SysStatus, game: bool, banner: bool, recording: bool, auth: bool, dropping: bool, capture: bool) -> Mode {
        if auth {
            return Mode::Auth;
        }
        if dropping {
            return Mode::Drop;
        }
        if capture {
            return Mode::Capture;
        }
        if banner {
            return Mode::Notif;
        }
        if self.open {
            return Mode::Open;
        }
        if recording && self.hud.is_none_or(|(_, _, t)| t.elapsed() >= Duration::from_millis(1500)) && self.charging.is_none_or(|(_, t)| t.elapsed() >= Duration::from_millis(2800)) {
            return Mode::Recording;
        }
        if let Some((pct, t)) = self.charging {
            if t.elapsed() < Duration::from_millis(2800) {
                return Mode::Charging(pct);
            }
        }
        if let Some((k, v, t)) = self.hud {
            if t.elapsed() < Duration::from_millis(1500) {
                return Mode::Hud(k, v);
            }
        }
        if !game && st.media.as_ref().is_some_and(|m| m.playing) {
            return Mode::Music;
        }
        Mode::Idle
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        let (bx, by, bw, bh) = self.body;
        (x as f32) >= bx && (x as f32) < bx + bw && (y as f32) >= by && (y as f32) < by + bh.max(H)
    }

    pub fn hit(&self, x: f64, y: f64) -> Option<NotchHit> {
        if !self.contains(x, y) {
            return None;
        }
        let (x, y) = (x as f32, y as f32);
        self.hits
            .iter()
            .find(|(_, r)| x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3)
            .map(|(h, _)| *h)
            .or(Some(NotchHit::Body))
    }

    /// Mouse em cima por 200 ms abre; saída fecha depois de 250 ms.
    pub fn set_hover(&mut self, inside: bool) {
        let now = Instant::now();
        if inside {
            self.left_at = None;
            if self.hovered_since.is_none() {
                self.hovered_since = Some(now);
            }
        } else {
            self.hovered_since = None;
            if self.open && self.left_at.is_none() {
                self.left_at = Some(now);
            }
        }
    }

    pub fn animating(&self) -> bool {
        !(self.w.settled() && self.h.settled() && self.ear.settled() && self.radius.settled() && self.content.settled())
            || self.hud.is_some_and(|(_, _, t)| t.elapsed() < Duration::from_millis(1600))
            || self.charging.is_some_and(|(_, t)| t.elapsed() < Duration::from_millis(2900))
            || self.hovered_since.is_some() && !self.open
            || self.left_at.is_some()
            || self.shown_mode == Mode::Recording
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        theme: &Theme,
        st: &SysStatus,
        sw: i32,
        scale: i32,
        game: bool,
        music_anim: bool,
        banner: Option<(&crate::notifications::Notif, usize)>,
        icons: &mut crate::ui::icons::IconStore,
        idle: Option<(&str, &str)>,
        recording: Option<u64>,
        auth: Option<(&str, usize)>,
        extra: &NotchExtra,
    ) {
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f32().min(1.0 / 30.0);
        self.last = now;

        // abrir/fechar pelo hover
        if !self.open && self.hovered_since.is_some_and(|t| now.duration_since(t) >= Duration::from_millis(200)) {
            self.open = true;
        }
        if self.open && self.left_at.is_some_and(|t| now.duration_since(t) >= Duration::from_millis(250)) {
            self.open = false;
            self.left_at = None;
            self.tab = Tab::Home;
        }

        let mode = self.mode(st, game, banner.is_some(), recording.is_some(), auth.is_some(), extra.drop.is_some(), extra.capture.is_some());
        // capa do álbum: carrega quando o arquivo muda (uma vez por faixa)
        let art_path = st.media.as_ref().map(|m| m.art.clone()).unwrap_or_default();
        if self.art.as_ref().map(|a| a.0.as_str()) != Some(art_path.as_str()) {
            self.art = if art_path.is_empty() {
                None
            } else {
                crate::settings_ctl::load_image(&art_path, 192).map(|pm| {
                    let acc = dominant(&pm);
                    (art_path.clone(), pm, acc)
                })
            };
        }
        // notificação: expande com o mouse em cima
        let notif_size = banner.map(|(n, _)| {
            let expanded = self.hovered_since.is_some();
            if expanded {
                let text_h: f32 = if n.body.is_empty() { 18.0 } else { 18.0 + 16.0 * 3.0 };
                let h = 32.0 + text_h.max(38.0) + if n.actions.is_empty() { 0.0 } else { 40.0 } + 14.0;
                (480.0, h.min(OPEN_H))
            } else {
                (440.0, 88.0)
            }
        });
        let (tw, th, open_shape) = match mode {
            Mode::Notif => {
                let (w, h) = notif_size.unwrap_or((440.0, 88.0));
                (w, h, true)
            }
            Mode::Auth => (420.0, 180.0, true),
            Mode::Drop => (OPEN_W, 150.0, true),
            Mode::Capture => (460.0, 122.0, true),
            Mode::Open => (OPEN_W, OPEN_H, true),
            Mode::Charging(_) => (CLOSED_W + 2.0 * 104.0, H, false),
            Mode::Recording => (CLOSED_W + 2.0 * 64.0, H, false),
            Mode::Hud(..) => (CLOSED_W + 2.0 * 96.0, H, false),
            Mode::Music => (CLOSED_W + 2.0 * 38.0, H, false),
            Mode::Idle => (CLOSED_W, H, false),
        };
        let (resp, damp) = if open_shape { (0.42, 0.8) } else { (0.45, 1.0) };
        for s in [&mut self.w, &mut self.h] {
            s.response = resp;
            s.damping = damp;
        }
        self.w.set_target(tw);
        self.h.set_target(th);
        self.ear.set_target(if open_shape { 19.0 } else { 6.0 });
        self.radius.set_target(if open_shape { 24.0 } else { 14.0 });
        // conteúdo: some e reaparece quando o modo muda (entra 0,4 / sai 0,22)
        if std::mem::discriminant(&mode) != std::mem::discriminant(&self.shown_mode) {
            if self.content.value < 0.05 {
                self.shown_mode = mode;
                self.content.response = 0.4;
                self.content.set_target(1.0);
            } else {
                self.content.response = 0.22;
                self.content.set_target(0.0);
            }
        } else {
            self.shown_mode = mode;
            self.content.response = 0.4;
            self.content.set_target(1.0);
        }
        for s in [&mut self.w, &mut self.h, &mut self.ear, &mut self.radius, &mut self.content] {
            s.step(dt);
        }

        let (w, h, ear, r) = (self.w.value, self.h.value, self.ear.value, self.radius.value);
        self.buf_x = ((sw as f32 - CANVAS_W) / 2.0).round();
        let x0 = (CANVAS_W - w - 2.0 * ear) / 2.0;
        self.body = (self.buf_x + x0 + ear, 0.0, w, h);

        // CPU/RAM só com o notch aberto (1 s)
        if self.shown_mode == Mode::Open && self.stats.is_none_or(|(_, _, t)| t.elapsed() > Duration::from_secs(1)) {
            if let Some((tot, idle)) = read_cpu() {
                let cpu = match self.cpu_prev {
                    Some((pt, pi)) if tot > pt => 1.0 - (idle - pi) as f32 / (tot - pt) as f32,
                    _ => 0.0,
                };
                self.cpu_prev = Some((tot, idle));
                self.stats = Some((cpu, read_ram().unwrap_or(0.0), now));
            }
        }

        // visualizador de música anima por tempo; sem animação, a chave não muda
        let viz_t = if self.shown_mode == Mode::Music && music_anim && !game { (now.duration_since(self.epoch).as_millis() / 33) as u64 } else { 0 };
        let key = format!(
            "{w:.1}|{h:.1}|{ear:.2}|{r:.2}|{:.2}|{:?}|{:?}|{game}|{viz_t}|{sw}|{scale}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{}",
            self.content.value,
            self.shown_mode,
            st.media,
            chrono::Local::now().format("%d"),
            self.stats.map(|s| (s.0 * 100.0) as u32),
            banner.map(|(n, c)| (n.id, c, n.summary.len())),
            self.tab,
            extra.capture.map(|c| (c.0, c.2)),
            extra.drop.map(|d| d > 0.55),
            extra.shelf.len()
        ) + &format!("|{:?}|{:?}|{:?}|{}", idle, recording, auth, if recording.is_some() { (now.duration_since(self.epoch).as_millis() / 120) as u64 } else { 0 });
        if self.key.as_deref() == Some(key.as_str()) && self.layer.is_set() {
            return;
        }
        self.key = Some(key);

        let mut base = Canvas::new(CANVAS_W as i32, CANVAS_H as i32, scale as f32).unwrap();
        let black = Rgba(0, 0, 0, 255);
        // cópia 1 px maior por baixo: evita o halo claro na borda suavizada
        if let Some(p) = shape(x0 - 1.0, w + 2.0, h + 1.0, r + 0.5, ear + 0.5) {
            base.fill_path(&p, black);
        }
        if let Some(p) = shape(x0, w, h, r, ear) {
            base.fill_path(&p, black);
        }
        // conteúdo num canvas à parte: entra crescendo do topo (abertos) ou das laterais (asas)
        let mut c = Canvas::new(CANVAS_W as i32, CANVAS_H as i32, scale as f32).unwrap();

        self.hits.clear();
        let a = self.content.value.clamp(0.0, 1.0);
        let wa = |o: f32| Rgba(255, 255, 255, (255.0 * o * a) as u8);
        let bx = x0 + ear; // borda esquerda do corpo (canvas)
        let to_screen = |r: (f32, f32, f32, f32)| (r.0 + self.buf_x, r.1, r.2, r.3);
        let mut hits: Vec<(NotchHit, (f32, f32, f32, f32))> = Vec::new();

        match self.shown_mode {
            Mode::Idle => {
                // como no kyro: usuário@máquina em cima, título da janela ativa embaixo
                if let Some((who, title)) = idle {
                    let maxw = w - 28.0;
                    let t1 = c.ellipsized(who, 9.0, Weight::Medium, maxw);
                    let w1 = c.measure(&t1, 9.0, Weight::Medium);
                    c.text(&t1, bx + (w - w1) / 2.0, 3.0, 9.0, Weight::Medium, wa(0.55));
                    if !title.is_empty() {
                        let t2 = c.ellipsized(title, 11.0, Weight::Semibold, maxw);
                        let w2 = c.measure(&t2, 11.0, Weight::Semibold);
                        c.text(&t2, bx + (w - w2) / 2.0, 14.0, 11.0, Weight::Semibold, wa(0.85));
                    }
                }
                if game {
                    c.fill_circle(bx + w - 9.0 - 2.5, h / 2.0, 2.5, Rgba(0x30, 0xd1, 0x58, (255.0 * a) as u8));
                }
            }
            Mode::Auth => {
                if let Some((msg, n)) = auth {
                    c.symbol("lock", bx + 30.0, 46.0, 22.0, wa(0.9));
                    c.text("Autenticação necessária", bx + 52.0, 36.0, 14.0, Weight::Bold, wa(0.95));
                    let mut ly = 60.0;
                    for line in wrap(msg, w - 70.0, 2, &c) {
                        c.text(&line, bx + 52.0, ly, 12.0, Weight::Regular, wa(0.6));
                        ly += 16.0;
                    }
                    let (fx, fy, fw) = (bx + 52.0, 98.0, w - 52.0 - 24.0);
                    c.fill_rrect(fx, fy, fw, 30.0, 15.0, wa(0.14));
                    if n == 0 {
                        c.text_vcenter("Senha", fx + 14.0, fy, 30.0, 13.0, Weight::Regular, wa(0.45));
                    } else {
                        c.text_vcenter(&"●".repeat(n.min(28)), fx + 14.0, fy, 30.0, 10.0, Weight::Regular, wa(1.0));
                    }
                    let by = 140.0;
                    let ok_x = bx + w - 24.0 - 90.0;
                    let cancel_x = ok_x - 8.0 - 90.0;
                    c.fill_rrect(cancel_x, by, 90.0, 28.0, 14.0, wa(0.13));
                    let cw = c.measure("Cancelar", 12.0, Weight::Semibold);
                    c.text_vcenter("Cancelar", cancel_x + (90.0 - cw) / 2.0, by, 28.0, 12.0, Weight::Semibold, wa(0.9));
                    c.fill_rrect(ok_x, by, 90.0, 28.0, 14.0, Rgba(0x00, 0x91, 0xff, (255.0 * a) as u8));
                    let ow = c.measure("Autenticar", 12.0, Weight::Semibold);
                    c.text_vcenter("Autenticar", ok_x + (90.0 - ow) / 2.0, by, 28.0, 12.0, Weight::Semibold, wa(1.0));
                    hits.push((NotchHit::AuthCancel, to_screen((cancel_x, by, 90.0, 28.0))));
                    hits.push((NotchHit::AuthOk, to_screen((ok_x, by, 90.0, 28.0))));
                }
            }
            Mode::Recording => {
                // asa esquerda: ponto vermelho pulsando; asa direita: tempo de gravação
                let pulse = 0.55 + 0.45 * ((now.duration_since(self.epoch).as_secs_f32() * 3.2).sin() * 0.5 + 0.5);
                c.fill_circle(bx + 18.0, h / 2.0, 6.0, Rgba(0xff, 0x42, 0x45, (255.0 * a * pulse) as u8));
                c.fill_circle(bx + 18.0, h / 2.0, 9.0, Rgba(0xff, 0x42, 0x45, (60.0 * a * pulse) as u8));
                let secs = recording.unwrap_or(0);
                let t = format!("{}:{:02}", secs / 60, secs % 60);
                let tw = c.measure(&t, 12.0, Weight::Semibold);
                c.text_vcenter(&t, bx + w - 16.0 - tw, 0.0, h, 12.0, Weight::Semibold, Rgba(0xff, 0x6b, 0x6b, (255.0 * a) as u8));
                hits.push((NotchHit::StopRecording, to_screen((bx, 0.0, w, h))));
            }
            Mode::Music => {
                match &self.art {
                    Some((_, pm, _)) => c.blit_rounded(pm, bx + 9.0, 5.0, 22.0, 22.0, 6.0, a),
                    None => {
                        c.fill_rrect(bx + 9.0, 5.0, 22.0, 22.0, 6.0, Rgba(0xff, 0x37, 0x5f, (230.0 * a) as u8));
                        c.symbol("music_note", bx + 20.0, 16.0, 14.0, wa(1.0));
                    }
                }
                let acc = lighter(self.art.as_ref().map(|x| x.2).unwrap_or(Rgba(0xff, 0x7a, 0x95, 255)), 1.15);
                // visualizador: 4 barras (paradas se não houver animação)
                let t = now.duration_since(self.epoch).as_secs_f32();
                for i in 0..4 {
                    let phase = t * 6.0 + i as f32 * 1.3;
                    let bh = if music_anim && !game { 4.0 + 10.0 * (0.5 + 0.5 * phase.sin()).abs() } else { 6.0 };
                    let x = bx + w - 11.0 - 18.0 + i as f32 * 5.0;
                    c.fill_rrect(x, 16.0 - bh / 2.0, 3.0, bh, 1.5, acc.with_alpha((255.0 * a) as u8));
                }
            }
            Mode::Hud(kind, v) => {
                let sym = match kind {
                    HudKind::Volume if v <= 0.001 => "volume_off",
                    HudKind::Volume => "volume_up",
                    HudKind::Brightness => "light_mode",
                };
                c.symbol(sym, bx + 14.0 + 9.0, 16.0, 18.0, wa(1.0));
                let (tx, tw) = (bx + w - 16.0 - 64.0, 64.0);
                c.fill_rrect(tx, 13.5, tw, 5.0, 2.5, wa(0.2));
                c.fill_rrect(tx, 13.5, (tw * v).max(5.0), 5.0, 2.5, wa(0.85));
            }
            Mode::Charging(pct) => {
                let green = Rgba(0x30, 0xd1, 0x58, (255.0 * a) as u8);
                c.symbol("bolt", bx + 14.0 + 8.0, 16.0, 16.0, green);
                c.text_vcenter("Carregando", bx + 34.0, 0.0, 32.0, 12.0, Weight::Semibold, green);
                let label = format!("{pct}%");
                let lw = c.measure(&label, 12.0, Weight::Semibold);
                let bxr = bx + w - 16.0 - 24.0;
                c.text_vcenter(&label, bxr - 6.0 - lw, 0.0, 32.0, 12.0, Weight::Semibold, wa(1.0));
                c.stroke_rrect(bxr, 10.0, 24.0, 12.0, 3.5, 1.0, Rgba(0x30, 0xd1, 0x58, (153.0 * a) as u8));
                c.fill_rrect(bxr + 2.0, 12.0, (20.0 * pct as f32 / 100.0).max(2.0), 8.0, 2.0, green);
            }
            Mode::Notif => {
                if let Some((n, more)) = banner {
                    let red = Rgba(0xff, 0x6b, 0x6b, (255.0 * a) as u8);
                    let app = if n.app.is_empty() { "Notificação" } else { n.app.as_str() };
                    c.text_vcenter(app, bx + 18.0, 0.0, 32.0, 11.0, Weight::Semibold, if n.critical { red } else { wa(0.55) });
                    let when = if more > 0 { format!("+{more}") } else { "agora".to_string() };
                    let ww = c.measure(&when, 11.0, Weight::Regular);
                    c.text_vcenter(&when, bx + w - 18.0 - ww - 22.0, 0.0, 32.0, 11.0, Weight::Regular, wa(0.45));
                    // fechar
                    let cx = bx + w - 18.0 - 7.0;
                    c.symbol("close", cx, 16.0, 14.0, wa(0.6));
                    hits.push((NotchHit::NotifClose, to_screen((cx - 11.0, 4.0, 22.0, 24.0))));
                    // ícone
                    let (ix, iy) = (bx + 16.0, 30.0);
                    let icon_name = if n.icon.is_empty() { n.app.to_lowercase() } else { n.icon.clone() };
                    let icon_pm = if n.icon.starts_with('/') || !icon_name.is_empty() {
                        Some(icons.get_theme(&icon_name, &n.app).clone())
                    } else {
                        None
                    };
                    match icon_pm {
                        Some(pm) => c.blit(&pm, ix, iy, 38.0, 38.0, a),
                        None => {
                            c.fill_rrect(ix, iy, 38.0, 38.0, 10.0, Rgba(0x00, 0x91, 0xff, (255.0 * a) as u8));
                            c.symbol("notifications", ix + 19.0, iy + 19.0, 22.0, wa(1.0));
                        }
                    }
                    let tx = ix + 38.0 + 12.0;
                    let tmax = bx + w - 18.0 - tx;
                    let t = c.ellipsized(&n.summary, 13.0, Weight::Bold, tmax);
                    c.text(&t, tx, iy, 13.0, Weight::Bold, wa(0.95));
                    let expanded = h > 90.0;
                    let lines = if expanded { 3 } else { 1 };
                    let mut ly = iy + 18.0;
                    for line in wrap(&n.body, tmax, lines, &c) {
                        c.text(&line, tx, ly, 12.0, Weight::Regular, wa(0.55));
                        ly += 16.0;
                    }
                    hits.push((NotchHit::NotifBody, to_screen((bx, 28.0, w, 50.0))));
                    if expanded && !n.actions.is_empty() {
                        let mut ax = tx;
                        let ay = h - 14.0 - 28.0;
                        for (i, (_, label)) in n.actions.iter().enumerate().take(3) {
                            let lw = c.measure(label, 12.0, Weight::Semibold) + 24.0;
                            c.fill_rrect(ax, ay, lw, 28.0, 14.0, wa(0.13));
                            c.text_vcenter(label, ax + 12.0, ay, 28.0, 12.0, Weight::Semibold, wa(0.9));
                            hits.push((NotchHit::NotifAction(i), to_screen((ax, ay, lw, 28.0))));
                            ax += lw + 8.0;
                        }
                    }
                }
            }
            Mode::Open => {
                // abas (Início / Bandeja) e estatísticas
                let mut tx = bx + 16.0;
                let tray_label = if extra.shelf.is_empty() { "Bandeja".to_string() } else { format!("Bandeja · {}", extra.shelf.len()) };
                for (tab, sym, label, hit) in [(Tab::Home, "home", "Início".to_string(), NotchHit::TabHome), (Tab::Tray, "inbox", tray_label, NotchHit::TabTray)] {
                    let lw = c.measure(&label, 12.0, Weight::Semibold) + 40.0;
                    if self.tab == tab {
                        c.fill_rrect(tx, 10.0, lw, 26.0, 13.0, wa(0.14));
                    }
                    c.symbol(sym, tx + 16.0, 23.0, 14.0, wa(0.85));
                    c.text_vcenter(&label, tx + 28.0, 10.0, 26.0, 12.0, Weight::Semibold, wa(0.85));
                    hits.push((hit, to_screen((tx, 10.0, lw, 26.0))));
                    tx += lw + 4.0;
                }
                if let Some((cpu, ram, _)) = self.stats {
                    let t = format!("CPU {}%   RAM {:.1} GB", (cpu * 100.0).round(), ram).replace('.', ",");
                    let tw = c.measure(&t, 11.0, Weight::Semibold);
                    c.text_vcenter(&t, bx + w - 18.0 - tw, 10.0, 26.0, 11.0, Weight::Semibold, wa(0.55));
                }
                let top = 42.0;
                let inner_h = h - top - 18.0;
                if self.tab == Tab::Tray {
                    shelf_view(&mut c, &mut hits, &to_screen, extra.shelf, bx + 18.0, top, w - 36.0, inner_h, a);
                } else {
                    let x = bx + 18.0;
                    now_playing(&mut c, &mut hits, &to_screen, st.media.as_ref(), self.art.as_ref().map(|x| (&x.1, x.2)), extra.badge, game, x, top, 236.0, inner_h, a);
                    let mut x = x + 236.0;
                    c.fill_rrect(x, top, 1.0, inner_h, 0.0, wa(0.12));
                    x += 18.0;
                    week_strip(&mut c, x, top, inner_h, a);
                    // --- Modo Jogo (botão redondo 84)
                    let gx = bx + w - 18.0 - 84.0;
                    c.fill_rrect(gx - 18.0, top, 1.0, inner_h, 0.0, wa(0.12));
                    let gy = top + (inner_h - 84.0) / 2.0 - 6.0;
                    let fill = if game { Rgba(0x30, 0xd1, 0x58, (56.0 * a) as u8) } else { wa(0.10) };
                    c.fill_circle(gx + 42.0, gy + 42.0, 42.0, fill);
                    let gc = if game { Rgba(0x30, 0xd1, 0x58, (255.0 * a) as u8) } else { wa(0.85) };
                    c.symbol("sports_esports", gx + 42.0, gy + 36.0, 30.0, gc);
                    let label = if game { "Jogo ativo" } else { "Modo Jogo" };
                    let lw = c.measure(label, 11.0, Weight::Semibold);
                    c.text(label, gx + 42.0 - lw / 2.0, gy + 56.0, 11.0, Weight::Semibold, gc);
                    hits.push((NotchHit::GameMode, to_screen((gx, gy, 84.0, 84.0))));
                }
            }
            Mode::Drop => {
                let hot_copy = extra.drop.unwrap_or(0.0) > 0.55;
                let (cy, ch) = (H + 2.0, h - H - 2.0 - 16.0);
                let cw = (w - 32.0 - 10.0) / 2.0;
                for (i, (sym, label, hot)) in [("inbox", "Bandeja de Arquivos", !hot_copy), ("content_copy", "Copiar", hot_copy)].iter().enumerate() {
                    let k = if *hot { 1.02 } else { 1.0 };
                    let (cw2, ch2) = (cw * k, ch * k);
                    let cx = bx + 16.0 + i as f32 * (cw + 10.0) - (cw2 - cw) / 2.0;
                    let yy = cy - (ch2 - ch) / 2.0;
                    if i == 0 {
                        dashed_rrect(&mut c, cx, yy, cw2, ch2, 20.0, Rgba(0x00, 0x91, 0xff, (255.0 * a) as u8));
                        if *hot {
                            c.fill_rrect(cx, yy, cw2, ch2, 20.0, Rgba(0x00, 0x91, 0xff, (40.0 * a) as u8));
                        }
                    } else {
                        let bg = if *hot { Rgba(0, 77, 153, (191.0 * a) as u8) } else { Rgba(0, 56, 115, (153.0 * a) as u8) };
                        c.fill_rrect(cx, yy, cw2, ch2, 20.0, bg);
                    }
                    c.symbol(sym, cx + cw2 / 2.0, yy + ch2 / 2.0 - 10.0, 28.0, wa(0.9));
                    let lw = c.measure(label, 12.0, Weight::Semibold);
                    c.text(label, cx + (cw2 - lw) / 2.0, yy + ch2 / 2.0 + 10.0, 12.0, Weight::Semibold, wa(0.9));
                }
            }
            Mode::Capture => {
                if let Some((path, thumb, ok)) = extra.capture {
                    let col = if ok { Rgba(0x30, 0xd1, 0x58, (255.0 * a) as u8) } else { Rgba(0xff, 0x9f, 0x0a, (255.0 * a) as u8) };
                    c.symbol(if ok { "check_circle" } else { "error" }, bx + 18.0 + 7.0, H / 2.0, 14.0, col);
                    c.text_vcenter("Captura de Tela", bx + 18.0 + 20.0, 0.0, H, 11.0, Weight::Semibold, wa(0.55));
                    c.symbol("close", bx + w - 12.0 - 11.0, H / 2.0, 14.0, wa(0.55));
                    hits.push((NotchHit::CaptureClose, to_screen((bx + w - 34.0, 4.0, 24.0, 24.0))));
                    let (px, py) = (bx + 16.0, H - 2.0);
                    c.fill_rrect(px, py, 112.0, 70.0, 10.0, wa(0.08));
                    if let Some(pm) = thumb {
                        // cabe inteira (como PreserveAspectFit)
                        let s = (112.0 / pm.width() as f32).min(70.0 / pm.height() as f32);
                        let (tw, th) = (pm.width() as f32 * s, pm.height() as f32 * s);
                        c.blit_rounded(pm, px + (112.0 - tw) / 2.0, py + (70.0 - th) / 2.0, tw, th, 6.0, a);
                    } else {
                        c.symbol("broken_image", px + 56.0, py + 35.0, 24.0, wa(0.55));
                    }
                    hits.push((NotchHit::CaptureOpen, to_screen((px, py, 112.0, 70.0))));
                    let tx = px + 112.0 + 12.0;
                    let tmax = bx + w - 16.0 - tx;
                    c.text(if ok { "Captura de tela" } else { "A captura falhou" }, tx, py, 13.0, Weight::Bold, wa(0.95));
                    let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    let name = c.ellipsized(&name, 11.0, Weight::Regular, tmax);
                    c.text(&name, tx, py + 18.0, 11.0, Weight::Regular, wa(0.55));
                    let mut ax = tx;
                    let ay = py + 70.0 - 28.0;
                    for (label, hit) in [("Copiar", NotchHit::CaptureCopy), ("Anotar", NotchHit::CaptureAnnotate), ("Salvar como…", NotchHit::CaptureSaveAs)] {
                        let lw = c.measure(label, 12.0, Weight::Semibold) + 24.0;
                        c.fill_rrect(ax, ay, lw, 28.0, 14.0, wa(0.13));
                        c.text_vcenter(label, ax + 12.0, ay, 28.0, 12.0, Weight::Semibold, wa(0.9));
                        hits.push((hit, to_screen((ax, ay, lw, 28.0))));
                        ax += lw + 6.0;
                    }
                }
            }
        }
        let _ = theme;
        self.hits = hits;
        let p = self.content.value.clamp(0.0, 1.0);
        let k = 0.6 + 0.4 * p;
        let closed = matches!(self.shown_mode, Mode::Idle | Mode::Music | Mode::Hud(..) | Mode::Charging(_) | Mode::Recording);
        let f = scale as f32;
        let tf = if closed {
            let cx = (bx + w / 2.0) * f;
            resvg::tiny_skia::Transform::from_row(k, 0.0, 0.0, 1.0, cx * (1.0 - k), 0.0)
        } else {
            resvg::tiny_skia::Transform::from_row(1.0, 0.0, 0.0, k, 0.0, 0.0)
        };
        // recorta pelo corpo (o conteúdo nunca passa da silhueta)
        let mut mask = resvg::tiny_skia::Mask::new(base.pm.width(), base.pm.height());
        if let (Some(m), Some(path)) = (mask.as_mut(), shape(x0, w, h, r, ear)) {
            m.fill_path(&path, resvg::tiny_skia::FillRule::Winding, true, resvg::tiny_skia::Transform::from_scale(f, f));
        }
        base.pm.draw_pixmap(0, 0, c.pm.as_ref(), &resvg::tiny_skia::PixmapPaint::default(), tf, mask.as_ref());
        self.layer.set(&base);
    }
}

fn week_strip(c: &mut Canvas, x: f32, top: f32, h: f32, a: f32) {
    use chrono::{Datelike, Duration as CD, Local};
    const MESES: [&str; 12] = ["Jan", "Fev", "Mar", "Abr", "Mai", "Jun", "Jul", "Ago", "Set", "Out", "Nov", "Dez"];
    const DIAS: [&str; 7] = ["SEG", "TER", "QUA", "QUI", "SEX", "SÁB", "DOM"];
    let today = Local::now().date_naive();
    let w = |o: f32| Rgba(255, 255, 255, (255.0 * o * a) as u8);
    let cy = top + h / 2.0;
    let month = MESES[today.month0() as usize];
    let mw = c.measure(month, 26.0, Weight::Bold);
    c.text_vcenter(month, x, cy - 18.0, 32.0, 26.0, Weight::Bold, w(0.9));
    // 5 dias centrados em hoje; os distantes vão sumindo
    let mut dx = x + mw + 14.0;
    for off in -2i64..=2 {
        let d = today + CD::days(off);
        let wd = d.weekday().num_days_from_monday() as usize;
        let is_today = off == 0;
        let weekend = wd >= 5;
        let fade = if is_today { 1.0 } else { 1.0 - off.abs() as f32 * 0.3 };
        let label = if is_today { DIAS[wd].to_string() } else { DIAS[wd][..DIAS[wd].char_indices().nth(1).map(|x| x.0).unwrap_or(1)].to_string() };
        let lw = c.measure(&label, 10.0, Weight::Bold);
        c.text(&label, dx + 17.0 - lw / 2.0, cy - 24.0, 10.0, Weight::Bold, w(if is_today { 0.85 } else { 0.4 * fade }));
        let num = format!("{:02}", d.day());
        let (size, wt) = if is_today { (22.0, Weight::Bold) } else { (17.0, Weight::Semibold) };
        let nw = c.measure(&num, size, wt);
        let col = if is_today {
            Rgba(0x0a, 0x84, 0xff, (255.0 * a) as u8)
        } else if weekend {
            Rgba(255, 69, 58, (191.0 * fade * a) as u8)
        } else {
            w(0.55 * fade)
        };
        c.text_vcenter(&num, dx + 17.0 - nw / 2.0, cy - 12.0, 30.0, size, wt, col);
        dx += 34.0;
    }
    let date = Local::now().format("%d/%m/%Y").to_string();
    c.text(&date, x, cy + 24.0, 12.0, Weight::Regular, w(0.4));
}

/// Tocando agora (como no NotchNook): capa 96 com brilho na cor dela e selo do app,
/// título (2 linhas), álbum, artista e controles pequenos.
#[allow(clippy::too_many_arguments)]
fn now_playing(
    c: &mut Canvas,
    hits: &mut Vec<(NotchHit, (f32, f32, f32, f32))>,
    to_screen: &dyn Fn((f32, f32, f32, f32)) -> (f32, f32, f32, f32),
    media: Option<&crate::services::Media>,
    art: Option<(&resvg::tiny_skia::Pixmap, Rgba)>,
    badge: Option<&resvg::tiny_skia::Pixmap>,
    game: bool,
    x: f32,
    top: f32,
    width: f32,
    h: f32,
    a: f32,
) {
    let wa = |o: f32| Rgba(255, 255, 255, (255.0 * o * a) as u8);
    let playing = media.is_some_and(|m| m.playing);
    let k = if playing || media.is_none() { 1.0 } else { 0.92 };
    let s = 96.0 * k;
    let (ax, ay) = (x + (96.0 - s) / 2.0, top + (h - 96.0) / 2.0 + (96.0 - s) / 2.0);
    if let (Some((_, acc)), false) = (art, game) {
        c.glow_rrect_add(ax, ay, s, s, 16.0, 24.0, acc.with_alpha(((if playing { 0.8 } else { 0.25 }) * 255.0 * a) as u8));
    }
    c.fill_rrect(ax, ay, s, s, 16.0, wa(0.08));
    match art {
        Some((pm, _)) => c.blit_rounded(pm, ax, ay, s, s, 16.0, a),
        None => c.symbol(if media.is_some() { "music_note" } else { "music_off" }, ax + s / 2.0, ay + s / 2.0, 40.0, wa(0.4)),
    }
    if media.is_some() {
        let (bx2, by2) = (x + 96.0 - 30.0 + 5.0, top + (h - 96.0) / 2.0 + 96.0 - 30.0 + 5.0);
        match badge {
            Some(pm) => c.blit(pm, bx2, by2, 30.0, 30.0, a),
            None => {
                c.fill_rrect(bx2 - 1.5, by2 - 1.5, 33.0, 33.0, 9.0, Rgba(0, 0, 0, (255.0 * a) as u8));
                c.fill_rrect_vgradient(bx2, by2, 30.0, 30.0, 8.0, Rgba(0xff, 0x5f, 0x6d, (255.0 * a) as u8), Rgba(0xfa, 0x23, 0x3b, (255.0 * a) as u8));
                c.symbol("music_note", bx2 + 15.0, by2 + 15.0, 18.0, wa(1.0));
            }
        }
    }
    let tx = x + 96.0 + 14.0;
    let tmax = width - 96.0 - 14.0 - 8.0;
    let mut ty = top + 8.0;
    match media {
        Some(m) => {
            for line in wrap_sized(&m.title, tmax, 2, 15.0, Weight::Bold, c) {
                c.text(&line, tx, ty, 15.0, Weight::Bold, wa(0.85));
                ty += 18.0;
            }
            let sub = if m.album.is_empty() { &m.artist } else { &m.album };
            if !sub.is_empty() {
                let t = c.ellipsized(sub, 12.0, Weight::Semibold, tmax);
                c.text(&t, tx, ty + 1.0, 12.0, Weight::Semibold, wa(0.55));
                ty += 16.0;
            }
            if !m.album.is_empty() && !m.artist.is_empty() {
                let t = c.ellipsized(&m.artist, 12.0, Weight::Regular, tmax);
                c.text(&t, tx, ty + 1.0, 12.0, Weight::Regular, wa(0.4));
            }
            let cy = top + h - 16.0;
            let play = if m.playing { "pause" } else { "play_arrow" };
            for (i, (sym, hit, size)) in [("skip_previous", NotchHit::MediaPrev, 20.0), (play, NotchHit::MediaPlay, 24.0), ("skip_next", NotchHit::MediaNext, 20.0)].iter().enumerate() {
                let cx = tx + 9.0 + i as f32 * 32.0;
                c.symbol(sym, cx, cy, *size, wa(0.9));
                hits.push((*hit, to_screen((cx - 15.0, cy - 15.0, 30.0, 30.0))));
            }
        }
        None => {
            c.text("Nada tocando", tx, top + h / 2.0 - 20.0, 15.0, Weight::Bold, wa(0.85));
            c.text("Abra um player de música", tx, top + h / 2.0, 12.0, Weight::Semibold, wa(0.55));
        }
    }
}

/// Bandeja: arquivos soltos no notch. Clique abre, botão direito tira da bandeja.
#[allow(clippy::too_many_arguments)]
fn shelf_view(
    c: &mut Canvas,
    hits: &mut Vec<(NotchHit, (f32, f32, f32, f32))>,
    to_screen: &dyn Fn((f32, f32, f32, f32)) -> (f32, f32, f32, f32),
    files: &[String],
    x: f32,
    top: f32,
    w: f32,
    h: f32,
    a: f32,
) {
    let wa = |o: f32| Rgba(255, 255, 255, (255.0 * o * a) as u8);
    if files.is_empty() {
        dashed_rrect(c, x, top, w, h, 20.0, wa(0.18));
        let t = "Arraste arquivos até o notch para guardá-los aqui";
        let tw = c.measure(t, 13.0, Weight::Regular) + 38.0;
        let sx = x + (w - tw) / 2.0;
        c.symbol("inbox", sx + 14.0, top + h / 2.0, 28.0, wa(0.3));
        c.text_vcenter(t, sx + 38.0, top, h, 13.0, Weight::Regular, wa(0.55));
        return;
    }
    let mut tx = x;
    for (i, f) in files.iter().enumerate() {
        if tx + 86.0 > x + w - 64.0 {
            break;
        }
        c.fill_rrect(tx, top, 86.0, h, 16.0, wa(0.08));
        c.symbol(file_symbol(f), tx + 43.0, top + 30.0, 34.0, Rgba(0x0a, 0x84, 0xff, (255.0 * a) as u8));
        let name = std::path::Path::new(f).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        for (k, line) in wrap_sized(&name, 74.0, 2, 10.0, Weight::Semibold, c).iter().enumerate() {
            let lw = c.measure(line, 10.0, Weight::Semibold);
            c.text(line, tx + 43.0 - lw / 2.0, top + 56.0 + k as f32 * 13.0, 10.0, Weight::Semibold, wa(0.85));
        }
        hits.push((NotchHit::ShelfItem(i), to_screen((tx, top, 86.0, h))));
        tx += 94.0;
    }
    let cx = x + w - 32.0;
    c.symbol("delete_sweep", cx, top + h / 2.0, 22.0, wa(0.55));
    hits.push((NotchHit::ShelfClear, to_screen((cx - 24.0, top + h / 2.0 - 24.0, 48.0, 48.0))));
}

/// Símbolo do tipo de arquivo (como o Shelf do kyro).
pub fn file_symbol(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" | "bmp" => "image",
        "mp3" | "flac" | "ogg" | "wav" | "m4a" | "opus" => "music_note",
        "mp4" | "mkv" | "webm" | "mov" | "avi" => "movie",
        "zip" | "tar" | "gz" | "xz" | "7z" | "rar" | "zst" => "folder_zip",
        "pdf" => "picture_as_pdf",
        "exe" | "msi" | "appimage" | "sh" => "terminal",
        "txt" | "md" | "log" | "conf" | "ini" | "json" | "lua" | "qml" | "py" | "js" | "rs" => "description",
        _ => "draft",
    }
}

/// Contorno tracejado de um retângulo arredondado (traço 2, 3,5/3).
fn dashed_rrect(c: &mut Canvas, x: f32, y: f32, w: f32, h: f32, r: f32, color: Rgba) {
    let Some(path) = crate::ui::canvas::rrect_path(x + 1.0, y + 1.0, w - 2.0, h - 2.0, [r; 4]) else { return };
    let Some(dash) = resvg::tiny_skia::StrokeDash::new(vec![3.5, 3.0], 0.0) else { return };
    let stroke = resvg::tiny_skia::Stroke { width: 2.0, dash: Some(dash), ..Default::default() };
    c.stroke_path_with(&path, &stroke, color);
}

/// Cor dominante da capa: média dos pixels mais saturados.
fn dominant(pm: &resvg::tiny_skia::Pixmap) -> Rgba {
    let (mut r, mut g, mut b, mut n) = (0u64, 0u64, 0u64, 0u64);
    for px in pm.pixels().iter().step_by(7) {
        let (pr, pg, pb) = (px.red() as i32, px.green() as i32, px.blue() as i32);
        let sat = pr.max(pg).max(pb) - pr.min(pg).min(pb);
        if sat > 40 {
            r += pr as u64;
            g += pg as u64;
            b += pb as u64;
            n += 1;
        }
    }
    if n == 0 {
        return Rgba(0x8e, 0x8e, 0x93, 255);
    }
    Rgba((r / n) as u8, (g / n) as u8, (b / n) as u8, 255)
}

fn lighter(c: Rgba, f: f32) -> Rgba {
    Rgba((c.0 as f32 * f).min(255.0) as u8, (c.1 as f32 * f).min(255.0) as u8, (c.2 as f32 * f).min(255.0) as u8, c.3)
}

/// Como `wrap`, para outro tamanho/peso de fonte.
fn wrap_sized(text: &str, max_w: f32, max_lines: usize, size: f32, weight: Weight, c: &Canvas) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let probe = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if c.measure(&probe, size, weight) <= max_w || cur.is_empty() {
            cur = probe;
        } else {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.len() > max_lines {
        let rest = lines[max_lines - 1..].join(" ");
        lines.truncate(max_lines - 1);
        lines.push(rest);
    }
    lines.into_iter().map(|l| c.ellipsized(&l, size, weight, max_w)).collect()
}

/// Quebra `text` em até `max_lines` linhas que cabem em `max_w` (a última com reticências).
pub fn wrap(text: &str, max_w: f32, max_lines: usize, c: &Canvas) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    let mut truncated = false;
    for word in text.split_whitespace() {
        if lines.len() == max_lines {
            truncated = true;
            break;
        }
        let probe = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if c.measure(&probe, 12.0, Weight::Regular) <= max_w || cur.is_empty() {
            cur = probe;
        } else {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
        }
    }
    if lines.len() < max_lines && !cur.is_empty() {
        lines.push(cur);
    } else if !cur.is_empty() {
        truncated = true;
    }
    if let Some(last) = lines.last_mut() {
        if truncated {
            last.push_str(" …");
        }
        *last = c.ellipsized(last, 12.0, Weight::Regular, max_w);
    }
    lines.truncate(max_lines);
    lines
}

/// file:///a%20b → /a b
pub fn uri_to_path(u: &str) -> String {
    let p = u.strip_prefix("file://").unwrap_or(u);
    let b = p.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&p[i + 1..i + 3], 16) {
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

/// /a b → /a%20b (para text/uri-list)
pub fn path_to_uri(p: &str) -> String {
    let mut out = String::new();
    for b in p.bytes() {
        if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
