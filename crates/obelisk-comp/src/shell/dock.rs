//! Dock (docs/03-especificacao-kyro.md §5): Launchpad · fixados · [abertos não fixados] ·
//! minimizados · Lixo. Magnificação gaussiana calculada sobre as posições *sem* ampliação,
//! ícones crescem para fora da placa, salto ao abrir, ponto de app aberto e balão com o nome.

use std::collections::HashMap;
use std::time::Instant;

use resvg::tiny_skia::Pixmap;

use super::layer::Layer;
use crate::apps::App;
use crate::ui::canvas::Canvas;
use crate::ui::icons::IconStore;
use crate::ui::text::Weight;
use crate::ui::theme::{Rgba, Theme};

pub const S: f32 = 52.0; // tamanho do ícone
const MAX_SCALE: f32 = 1.6;
const PAD: f32 = 6.0;
const SPACING: f32 = 4.0;
const SEP_W: f32 = 13.0;
pub const PLATE_H: f32 = S + 2.0 * PAD; // 64
const PLATE_R: f32 = 20.0;
pub const BOTTOM: f32 = 6.0;
const SHADOW: f32 = 24.0;
const TOP_ROOM: f32 = S * (MAX_SCALE - 1.0) + 0.35 * S + 44.0; // ampliação + salto + balão

#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Launchpad,
    App { app: App, pinned: bool, running: bool },
    Separator,
    /// Índice em `Obelisk::minimized`.
    Minimized { index: usize, label: String, icon: String },
    Trash { full: bool },
}

impl Entry {
    fn id(&self) -> String {
        match self {
            Entry::Launchpad => "launchpad".into(),
            Entry::App { app, .. } => format!("app:{}", app.id),
            Entry::Separator => "sep".into(),
            Entry::Minimized { index, .. } => format!("min:{index}"),
            Entry::Trash { .. } => "trash".into(),
        }
    }
    pub fn label(&self) -> String {
        match self {
            Entry::Launchpad => "Launchpad".into(),
            Entry::App { app, .. } => app.name.clone(),
            Entry::Separator => String::new(),
            Entry::Minimized { label, .. } => label.clone(),
            Entry::Trash { .. } => "Lixo".into(),
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Anim {
    scale: f32,
    bounce_start: Option<Instant>,
}

pub struct Dock {
    pub entries: Vec<Entry>,
    anims: HashMap<String, Anim>,
    pub layer: Layer,
    pub hover: Option<usize>,
    pub menu_open: bool,
    /// (x0, largura) de cada item em coordenadas de tela, do último quadro.
    slots: Vec<(f32, f32)>,
    /// Placa em coordenadas de tela (x, y, w, h).
    pub panel: (f32, f32, f32, f32),
    pub buf_pos: (i32, i32),
    last: Option<Instant>,
    animating: bool,
    key: Option<String>,
    launchpad_icon: Option<Pixmap>,
    /// esconder (ocultar automático ou janela por cima) — o Dock desliza para baixo da tela
    pub hide: bool,
    /// 1 = visível, 0 = escondido
    pub reveal: f32,
    /// placa na posição visível (para saber se uma janela está por cima)
    pub shown_rect: (f32, f32, f32, f32),
}

impl Dock {
    pub fn new() -> Self {
        Dock {
            entries: vec![Entry::Launchpad, Entry::Separator, Entry::Trash { full: false }],
            anims: HashMap::new(),
            layer: Layer::default(),
            hover: None,
            menu_open: false,
            slots: Vec::new(),
            panel: (0.0, 0.0, 0.0, 0.0),
            buf_pos: (0, 0),
            last: None,
            animating: false,
            key: None,
            launchpad_icon: None,
            hide: false,
            reveal: 1.0,
            shown_rect: (0.0, 0.0, 0.0, 0.0),
        }
    }

    pub fn set_entries(&mut self, entries: Vec<Entry>) {
        if entries != self.entries {
            self.entries = entries;
            self.key = None;
        }
    }

    pub fn start_bounce(&mut self, idx: usize) {
        if let Some(e) = self.entries.get(idx) {
            let a = self.anims.entry(e.id()).or_default();
            a.bounce_start = Some(Instant::now());
        }
    }

    pub fn stop_bounce_for(&mut self, app_id: &str) {
        self.anims.retain(|k, a| {
            if k == &format!("app:{app_id}") {
                a.bounce_start = None;
            }
            true
        });
    }

    pub fn needs_redraw(&self) -> bool {
        self.animating
    }

    fn base_width(entries: &[Entry]) -> f32 {
        let n = entries.len() as f32;
        entries.iter().map(|e| if *e == Entry::Separator { SEP_W } else { S }).sum::<f32>() + SPACING * (n - 1.0) + 2.0 * PAD
    }

    /// Item sob o ponteiro (tela). Considera o espaço dos ícones ampliados acima da placa.
    pub fn item_at(&self, x: f64, y: f64) -> Option<usize> {
        if !self.contains(x, y) {
            return None;
        }
        self.slots.iter().enumerate().find_map(|(i, (x0, w))| {
            let hit = (x as f32) >= x0 - SPACING / 2.0 && (x as f32) < x0 + w + SPACING / 2.0;
            (hit && self.entries[i] != Entry::Separator).then_some(i)
        })
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        if self.reveal < 0.5 {
            return false;
        }
        let (px, py, pw, ph) = self.panel;
        let top = py - S * (MAX_SCALE - 1.0);
        (x as f32) >= px && (x as f32) <= px + pw && (y as f32) >= top && (y as f32) <= py + ph + BOTTOM
    }

    /// Centro x (tela) e topo do item, para posicionar o menu de contexto.
    pub fn slot_x(&self, idx: usize) -> Option<f32> {
        self.slots.get(idx).map(|(x, _)| *x)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        theme: &Theme,
        icons: &mut IconStore,
        screen_w: i32,
        screen_h: i32,
        scale: i32,
        pointer: Option<(f64, f64)>,
        backdrop: Option<&Pixmap>,
        now: Instant,
    ) {
        let dt = self.last.map(|l| now.duration_since(l).as_secs_f32()).unwrap_or(0.016).min(0.05);
        self.last = Some(now);

        let base_w = Self::base_width(&self.entries);
        let max_w = base_w + S * (MAX_SCALE - 1.0) * 3.0;
        let buf_w = (max_w + 2.0 * SHADOW).ceil();
        let buf_h = (PLATE_H + TOP_ROOM + SHADOW).ceil();
        let buf_x = ((screen_w as f32 - buf_w) / 2.0).round();
        let buf_y = screen_h as f32 - BOTTOM - PLATE_H - TOP_ROOM;
        self.buf_pos = (buf_x as i32, buf_y as i32);

        // ---- magnificação (posições-base sem ampliação)
        let base_left = (screen_w as f32 - base_w) / 2.0;
        let hovering = !self.menu_open && pointer.is_some_and(|(px, py)| self.contains(px, py));
        let k = 1.0 - (-dt / 0.03).exp(); // ≈ 90 ms
        let mut animating = false;
        let mut bx = PAD;
        for e in &self.entries {
            let w = if *e == Entry::Separator { SEP_W } else { S };
            let center = base_left + bx + w / 2.0;
            bx += w + SPACING;
            let target = match (e, hovering, pointer) {
                (Entry::Separator, _, _) => 1.0,
                (_, true, Some((mx, _))) => {
                    let d = mx as f32 - center;
                    let sigma = 1.25 * S;
                    1.0 + (MAX_SCALE - 1.0) * (-(d * d) / (2.0 * sigma * sigma)).exp()
                }
                _ => 1.0,
            };
            let a = self.anims.entry(e.id()).or_insert(Anim { scale: 1.0, bounce_start: None });
            a.scale += (target - a.scale) * k;
            if (a.scale - target).abs() > 0.002 {
                animating = true;
            } else {
                a.scale = target;
            }
            if let Some(t0) = a.bounce_start {
                if now.duration_since(t0).as_secs_f32() > 0.96 {
                    a.bounce_start = None;
                } else {
                    animating = true;
                }
            }
        }
        // ---- mostrar/esconder (desliza ~180 ms)
        let target = if self.hide { 0.0 } else { 1.0 };
        self.reveal += (target - self.reveal) * (1.0 - (-dt / 0.06).exp());
        if (self.reveal - target).abs() > 0.003 {
            animating = true;
        } else {
            self.reveal = target;
        }
        self.animating = animating;

        // ---- layout final: a placa só alarga
        let sizes: Vec<f32> = self
            .entries
            .iter()
            .map(|e| if *e == Entry::Separator { SEP_W } else { S * self.anims.get(&e.id()).map(|a| a.scale).unwrap_or(1.0) })
            .collect();
        let row_w: f32 = sizes.iter().sum::<f32>() + SPACING * (sizes.len() as f32 - 1.0);
        let plate_w = row_w + 2.0 * PAD;
        let plate_x = (buf_w - plate_w) / 2.0;
        let plate_y = TOP_ROOM;
        self.panel = (buf_x + plate_x, buf_y + plate_y, plate_w, PLATE_H);
        self.shown_rect = self.panel;
        let slide = (1.0 - self.reveal) * (PLATE_H + BOTTOM + 14.0);
        self.panel.1 += slide;
        self.buf_pos.1 = (buf_y + slide).round() as i32;
        self.slots.clear();
        let mut x = plate_x + PAD;
        for w in &sizes {
            self.slots.push((buf_x + x, *w));
            x += w + SPACING;
        }

        let key = format!("{:?}|{}|{}|{}|{}|{}", self.hover, screen_w, screen_h, scale, theme.appearance as u8, self.entries.len());
        if !self.animating && self.key.as_deref() == Some(key.as_str()) && self.layer.is_set() {
            return;
        }
        self.key = Some(key);

        // ---- desenho
        let mut c = Canvas::new(buf_w as i32, buf_h as i32, scale as f32).unwrap();
        c.shadow_rrect(plate_x, plate_y + 4.0, plate_w, PLATE_H, PLATE_R, 16.0, 70);
        if let Some(bd) = backdrop {
            c.fill_rrect_backdrop(plate_x, plate_y, plate_w, PLATE_H, PLATE_R, bd, (buf_x, buf_y), (screen_w as f32, screen_h as f32));
        }
        c.fill_rrect(plate_x, plate_y, plate_w, PLATE_H, PLATE_R, theme.dock_bg);
        c.stroke_rrect(plate_x, plate_y, plate_w, PLATE_H, PLATE_R, 1.0, theme.fg(0.16));

        let icon_bottom = plate_y + PLATE_H - PAD;
        let mut tooltip: Option<(String, f32, f32)> = None;
        for (i, e) in self.entries.iter().enumerate() {
            let (sx, w) = self.slots[i];
            let lx = sx - buf_x;
            if *e == Entry::Separator {
                c.fill_rrect(lx + w / 2.0 - 0.5, plate_y + (PLATE_H - 0.75 * S) / 2.0, 1.0, 0.75 * S, 0.0, theme.fg(0.22));
                continue;
            }
            let bounce = self.anims.get(&e.id()).and_then(|a| a.bounce_start).map(|t0| bounce_offset(now.duration_since(t0).as_secs_f32())).unwrap_or(0.0);
            let top = icon_bottom - w - bounce;
            match e {
                Entry::Launchpad => {
                    let pm = self.launchpad_icon.get_or_insert_with(|| launchpad_tile(192));
                    c.blit(pm, lx, top, w, w, 1.0);
                }
                Entry::App { app, running, .. } => {
                    let pm = icons.get_theme(&app.icon, &app.name).clone();
                    c.blit(&pm, lx, top, w, w, 1.0);
                    if *running {
                        c.fill_circle(lx + w / 2.0, icon_bottom + 3.0, 2.0, theme.fg(0.75));
                    }
                }
                Entry::Minimized { icon, label, .. } => {
                    // miniatura simplificada: cartão da janela + ícone do app no canto
                    let inset = w * 0.06;
                    c.fill_rrect(lx + inset, top + inset, w - 2.0 * inset, w - 2.0 * inset, 8.0, theme.window_bg);
                    c.stroke_rrect(lx + inset, top + inset, w - 2.0 * inset, w - 2.0 * inset, 8.0, 1.0, theme.fg(0.16));
                    c.fill_rrect(lx + inset, top + inset, w - 2.0 * inset, w * 0.16, 8.0, theme.fg(0.08));
                    let pm = icons.get_theme(icon, label).clone();
                    let is = w * 0.42;
                    c.blit(&pm, lx + w - is, top + w - is, is, is, 1.0);
                }
                Entry::Trash { full } => {
                    let name = if *full { "user-trash-full" } else { "user-trash" };
                    let pm = icons.get_theme(name, "Lixo").clone();
                    c.blit(&pm, lx, top, w, w, 1.0);
                }
                Entry::Separator => {}
            }
            if self.hover == Some(i) && !self.menu_open {
                tooltip = Some((e.label(), lx + w / 2.0, top));
            }
        }

        if let Some((label, cx, top)) = tooltip {
            let tw = c.measure(&label, 13.0, Weight::Medium);
            let (w, h) = (tw + 22.0, 26.0);
            let tx = (cx - w / 2.0).clamp(2.0, buf_w - w - 2.0);
            let ty = (top - 10.0 - h).max(0.0);
            c.fill_rrect(tx, ty, w, h, 8.0, theme.menu_bg_opaque());
            c.stroke_rrect(tx, ty, w, h, 8.0, 1.0, theme.fg(0.14));
            c.text_vcenter(&label, tx + 11.0, ty, h, 13.0, Weight::Medium, theme.label);
        }
        self.layer.set(&c);
    }
}

/// Salto: sobe 0,35·S em 220 ms (OutQuad) e cai em 260 ms (OutBounce), duas vezes.
fn bounce_offset(t: f32) -> f32 {
    let cycle = 0.48;
    if t >= 2.0 * cycle {
        return 0.0;
    }
    let t = t % cycle;
    let h = 0.35 * S;
    if t < 0.22 {
        let p = t / 0.22;
        h * (1.0 - (1.0 - p) * (1.0 - p))
    } else {
        let p = (t - 0.22) / 0.26;
        h * (1.0 - out_bounce(p))
    }
}

fn out_bounce(mut t: f32) -> f32 {
    let (n1, d1) = (7.5625, 2.75);
    if t < 1.0 / d1 {
        n1 * t * t
    } else if t < 2.0 / d1 {
        t -= 1.5 / d1;
        n1 * t * t + 0.75
    } else if t < 2.5 / d1 {
        t -= 2.25 / d1;
        n1 * t * t + 0.9375
    } else {
        t -= 2.625 / d1;
        n1 * t * t + 0.984375
    }
}

/// Tile do Launchpad (desenhado, como no kyro): placa clara com grade 3×3 colorida.
pub fn launchpad_tile(size: u32) -> Pixmap {
    let mut c = Canvas::new(size as i32, size as i32, 1.0).unwrap();
    let sz = size as f32;
    let inset = sz * 0.07;
    let tw = sz - 2.0 * inset;
    let r = tw * 0.225;
    c.fill_rrect_vgradient(inset, inset, tw, tw, r, Rgba::rgb(0xfd, 0xfd, 0xff), Rgba::rgb(0xd5, 0xd6, 0xde));
    c.stroke_rrect(inset, inset, tw, tw, r, (tw * 0.012).max(1.0), Rgba(255, 255, 255, 178));
    c.fill_rrect_vgradient(inset + 1.0, inset + 1.0, tw - 2.0, tw / 2.0, r, Rgba(255, 255, 255, 140), Rgba(255, 255, 255, 0));
    let colors = [
        (0xff, 0x42, 0x45), (0xff, 0x92, 0x30), (0xff, 0xd6, 0x00),
        (0x30, 0xd1, 0x58), (0x00, 0xd2, 0xe0), (0x00, 0x91, 0xff),
        (0x6d, 0x7c, 0xff), (0xdb, 0x34, 0xf2), (0xff, 0x37, 0x5f),
    ];
    let sq = tw * 0.18;
    let gap = tw * 0.075;
    let grid = 3.0 * sq + 2.0 * gap;
    let gx = inset + (tw - grid) / 2.0;
    for (i, (r_, g, b)) in colors.iter().enumerate() {
        let (col, row) = ((i % 3) as f32, (i / 3) as f32);
        let x = gx + col * (sq + gap);
        let y = gx + row * (sq + gap);
        let base = Rgba::rgb(*r_, *g, *b);
        let light = Rgba::rgb((*r_ as f32 * 1.15).min(255.0) as u8, (*g as f32 * 1.15).min(255.0) as u8, (*b as f32 * 1.15).min(255.0) as u8);
        c.fill_rrect_vgradient(x, y, sq, sq, sq * 0.32, light, base);
    }
    c.pm
}
