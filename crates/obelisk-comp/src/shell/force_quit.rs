//! Forçar Encerrar Aplicativos (⌥⌘⎋), como no macOS: lista dos apps abertos com ícone,
//! texto de ajuda e botão "Forçar Encerrar". ↑/↓ escolhe, Enter encerra, Esc fecha.

use resvg::tiny_skia::Pixmap;

use super::layer::Layer;
use crate::ui::canvas::Canvas;
use crate::ui::text::Weight;
use crate::ui::theme::{Appearance, Rgba, Theme};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FqHit {
    Row(usize),
    Kill,
    Close,
}

pub struct FqEntry {
    pub name: String,
    pub icon: Pixmap,
    /// app sem resposta (o compositor percebe pelo ping do xdg-shell não respondido)
    pub hung: bool,
}

const W: f32 = 420.0;
const ROW_H: f32 = 30.0;
const ROWS: usize = 8;
const SHADOW: f32 = 44.0;

#[derive(Default)]
pub struct ForceQuit {
    pub open: bool,
    pub entries: Vec<FqEntry>,
    pub selected: usize,
    pub layer: Layer,
    pub pos: (f32, f32),
    pub size: (f32, f32),
    hits: Vec<(FqHit, (f32, f32, f32, f32))>,
    key: Option<String>,
}

impl ForceQuit {
    pub fn show(&mut self, entries: Vec<FqEntry>) {
        self.entries = entries;
        self.selected = 0;
        self.open = true;
        self.key = None;
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        self.open && (x as f32) >= self.pos.0 && (x as f32) < self.pos.0 + self.size.0 && (y as f32) >= self.pos.1 && (y as f32) < self.pos.1 + self.size.1
    }

    pub fn hit(&self, x: f64, y: f64) -> Option<FqHit> {
        let (x, y) = (x as f32, y as f32);
        self.hits.iter().find(|(_, r)| x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3).map(|(h, _)| *h)
    }

    pub fn step(&mut self, d: i32) {
        if !self.entries.is_empty() {
            self.selected = (self.selected as i32 + d).rem_euclid(self.entries.len() as i32) as usize;
        }
    }

    pub fn update(&mut self, theme: &Theme, sw: i32, sh: i32, scale: i32, backdrop: Option<&Pixmap>) {
        let rows = self.entries.len().clamp(1, ROWS) as f32;
        let list_h = rows * ROW_H + 8.0;
        let h = 52.0 + 40.0 + list_h + 40.0 + 52.0;
        self.size = (W, h);
        self.pos = (((sw as f32 - W) / 2.0).round(), ((sh as f32 - h) / 3.0).round());
        let key = format!("{}|{}|{}|{}|{sw}|{scale}", self.entries.len(), self.selected, theme.appearance as u8, theme.accent.0);
        if self.key.as_deref() == Some(key.as_str()) && self.layer.is_set() {
            return;
        }
        self.key = Some(key);
        self.hits.clear();
        let dark = theme.appearance == Appearance::Dark;
        let (sx, sy) = (self.pos.0 - SHADOW, self.pos.1 - SHADOW);
        let to_screen = |r: (f32, f32, f32, f32)| (r.0 + sx, r.1 + sy, r.2, r.3);
        let mut c = Canvas::new((W + 2.0 * SHADOW) as i32, (h + 2.0 * SHADOW) as i32, scale as f32).unwrap();
        let (ox, oy) = (SHADOW, SHADOW);
        c.shadow_rrect(ox, oy + 14.0, W, h, 12.0, 40.0, 130);
        if let Some(bd) = backdrop {
            c.fill_rrect_backdrop(ox, oy, W, h, 12.0, bd, (sx, sy), (sw as f32, sh as f32));
        }
        c.fill_rrect(ox, oy, W, h, 12.0, theme.window_bg);
        c.stroke_rrect(ox, oy, W, h, 12.0, 1.0, theme.fg(0.12));

        // barra de título com os semáforos (só fechar ativo) e título centrado
        for (i, col) in [theme.tl_close, theme.fg(0.15), theme.fg(0.15)].iter().enumerate() {
            c.fill_circle(ox + 18.0 + i as f32 * 20.0, oy + 18.0, 6.0, *col);
        }
        self.hits.push((FqHit::Close, to_screen((ox + 10.0, oy + 10.0, 16.0, 16.0))));
        let title = "Forçar Encerrar Aplicativos";
        let tw = c.measure(title, 13.0, Weight::Semibold);
        c.text_vcenter(title, ox + (W - tw) / 2.0, oy + 4.0, 28.0, 13.0, Weight::Semibold, theme.label);

        let mut y = oy + 44.0;
        for line in ["Se um aplicativo não responder, selecione o nome", "dele e clique em Forçar Encerrar."] {
            c.text(line, ox + 20.0, y, 12.0, Weight::Regular, theme.label);
            y += 17.0;
        }
        y += 14.0;

        // lista (fundo branco/escuro com borda, como uma tabela do AppKit)
        let (lx, lw) = (ox + 20.0, W - 40.0);
        c.fill_rrect(lx, y, lw, list_h, 6.0, if dark { Rgba(0, 0, 0, 60) } else { Rgba::rgb(255, 255, 255) });
        c.stroke_rrect(lx, y, lw, list_h, 6.0, 1.0, theme.fg(0.12));
        let mut ry = y + 4.0;
        if self.entries.is_empty() {
            c.text_vcenter("Nenhum aplicativo aberto", lx + 12.0, ry, ROW_H, 13.0, Weight::Regular, theme.secondary);
        }
        for (i, e) in self.entries.iter().take(ROWS).enumerate() {
            let sel = i == self.selected;
            if sel {
                c.fill_rrect(lx + 4.0, ry, lw - 8.0, ROW_H, 5.0, theme.accent);
            }
            c.blit(&e.icon, lx + 10.0, ry + 3.0, 24.0, 24.0, 1.0);
            let col = if sel { theme.text_on_accent } else { theme.label };
            let name = c.ellipsized(&e.name, 13.0, Weight::Regular, lw - 150.0);
            c.text_vcenter(&name, lx + 42.0, ry, ROW_H, 13.0, Weight::Regular, col);
            if e.hung {
                let t = "(sem resposta)";
                let w = c.measure(t, 12.0, Weight::Regular);
                c.text_vcenter(t, lx + lw - 14.0 - w, ry, ROW_H, 12.0, Weight::Regular, if sel { col } else { theme.red });
            }
            self.hits.push((FqHit::Row(i), to_screen((lx + 4.0, ry, lw - 8.0, ROW_H))));
            ry += ROW_H;
        }
        y += list_h + 12.0;
        c.text("Você também pode abrir esta janela com ⌥⌘⎋.", ox + 20.0, y, 11.0, Weight::Regular, theme.secondary);

        // botão padrão (azul) à direita, como no macOS
        let label = "Forçar Encerrar";
        let bw = c.measure(label, 13.0, Weight::Medium) + 28.0;
        let (bx, by) = (ox + W - 20.0 - bw, oy + h - 20.0 - 26.0);
        let enabled = !self.entries.is_empty();
        let a = if enabled { 255 } else { 100 };
        c.fill_rrect_vgradient(bx, by, bw, 26.0, 7.0, theme.accent.with_alpha(a), theme.accent.with_alpha(a));
        c.text_vcenter(label, bx + 14.0, by, 26.0, 13.0, Weight::Medium, theme.text_on_accent.with_alpha(a));
        if enabled {
            self.hits.push((FqHit::Kill, to_screen((bx, by, bw, 26.0))));
        }
        self.layer.set(&c);
    }
}
