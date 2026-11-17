//! Ajustes do Sistema (layout do macOS Ventura+ e do kyro). Desenha qualquer página a partir do
//! modelo (`settings_model`): barra lateral com busca, cartão do usuário e categorias que recolhem;
//! grupos com chaves, deslizantes, segmentados, listas, campos de texto e botões.

use std::collections::{HashMap, HashSet};

use resvg::tiny_skia::Pixmap;

use super::layer::Layer;
use super::settings_model::{Act, Group, Row};
use super::settings_pages::{PAGES, SECTIONS};
use super::store_pages::{STORE_PAGES, STORE_SECTIONS};

type Catalog = &'static [(&'static str, &'static str, &'static str, (u8, u8, u8), &'static str, &'static str)];
type Sections = &'static [(&'static str, &'static [&'static str])];
use crate::ui::canvas::Canvas;
use crate::ui::text::Weight;
use crate::ui::text_glyph;
use crate::ui::theme::{Appearance, Rgba, Theme};

/// Clique em algo dos Ajustes.
#[derive(Clone, Debug, PartialEq)]
pub enum SetAction {
    Page(&'static str),
    Section(&'static str),
    Act(Act),
    /// deslizante: (template, root, x0 e largura na tela) — o valor vem da posição do clique
    Slider(String, bool, f32, f32),
    Search,
    Close,
}

const W: f32 = 900.0;
const H: f32 = 620.0;
const SIDE: f32 = 250.0;
const SHADOW: f32 = 44.0;
const ROW_H: f32 = 46.0;

pub struct SettingsApp {
    pub open: bool,
    pub page: &'static str,
    pub query: String,
    pub hover: Option<(f32, f32)>,
    pub collapsed: HashSet<&'static str>,
    pub scroll: f32,
    pub layer: Layer,
    pub pos: (f32, f32),
    hits: Vec<(SetAction, (f32, f32, f32, f32))>,
    key: Option<String>,
    content_h: f32,
    /// janela no modo App Store (mesmo layout, catálogo da loja)
    pub store: bool,
    /// ícones dos apps da loja (arquivo → imagem)
    icons: HashMap<String, Option<Pixmap>>,
    pub side_scroll: f32,
    side_h: f32,
}

impl Default for SettingsApp {
    fn default() -> Self {
        SettingsApp {
            open: false,
            page: "wifi",
            query: String::new(),
            hover: None,
            collapsed: HashSet::new(),
            scroll: 0.0,
            layer: Layer::default(),
            pos: (0.0, 0.0),
            hits: vec![],
            key: None,
            content_h: 0.0,
            side_scroll: 0.0,
            side_h: 0.0,
            store: false,
            icons: HashMap::new(),
        }
    }
}

fn tint((r, g, b): (u8, u8, u8), f: f32) -> Rgba {
    Rgba::rgb((r as f32 * f).min(255.0) as u8, (g as f32 * f).min(255.0) as u8, (b as f32 * f).min(255.0) as u8)
}

impl SettingsApp {
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.query.clear();
        self.scroll = 0.0;
        self.key = None;
    }

    pub fn set_page(&mut self, p: &'static str) {
        self.page = p;
        self.scroll = 0.0;
        self.key = None;
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        self.open && (x as f32) >= self.pos.0 && (x as f32) < self.pos.0 + W && (y as f32) >= self.pos.1 && (y as f32) < self.pos.1 + H
    }

    pub fn in_content(&self, x: f64) -> bool {
        (x as f32) >= self.pos.0 + SIDE
    }

    pub fn hit(&self, x: f64, y: f64) -> Option<SetAction> {
        let (x, y) = (x as f32, y as f32);
        self.hits.iter().rev().find(|(_, r)| x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3).map(|(h, _)| h.clone())
    }

    pub fn scroll_by(&mut self, d: f32) {
        let max = (self.content_h - (H - 70.0)).max(0.0);
        self.scroll = (self.scroll + d).clamp(0.0, max);
    }

    pub fn scroll_side(&mut self, d: f32) {
        let max = (self.side_h - (H - 152.0)).max(0.0);
        self.side_scroll = (self.side_scroll + d).clamp(0.0, max);
        self.key = None;
    }

    pub fn pages(&self) -> Catalog {
        if self.store { &STORE_PAGES } else { &PAGES }
    }

    fn sections(&self) -> Sections {
        if self.store { &STORE_SECTIONS } else { &SECTIONS }
    }

    pub fn visible_pages(&self) -> Vec<&'static str> {
        if self.store {
            // na loja a busca procura apps, não páginas
            return self.pages().iter().map(|p| p.0).collect();
        }
        let q = self.query.trim().to_lowercase();
        self.pages().iter().filter(|p| q.is_empty() || p.1.to_lowercase().contains(&q) || p.5.contains(&q)).map(|p| p.0).collect()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        theme: &Theme,
        groups: &[Group],
        fields: &HashMap<String, String>,
        focus: Option<&str>,
        special: &Special,
        sw: i32,
        sh: i32,
        scale: i32,
        backdrop: Option<&Pixmap>,
    ) {
        self.pos = (((sw as f32 - W) / 2.0).round(), ((sh as f32 - H) / 2.0).round().max(40.0));
        let key = format!(
            "{}|{:?}|{}|{}|{sw}|{scale}|{:?}|{}|{:?}|{:?}|{:?}|{:.0}|{:?}",
            self.page,
            groups.iter().map(|g| format!("{:?}", g.rows)).collect::<Vec<_>>(),
            theme.appearance as u8,
            self.query,
            self.hover.map(|h| (h.0 as i32 / 4, h.1 as i32 / 4)),
            theme.accent.0,
            fields,
            focus,
            self.collapsed,
            self.scroll + self.side_scroll * 10000.0,
            special.key()
        );
        if self.key.as_deref() == Some(key.as_str()) && self.layer.is_set() {
            return;
        }
        self.key = Some(key);
        self.hits.clear();
        let (sx, sy) = (self.pos.0 - SHADOW, self.pos.1 - SHADOW);
        let to_screen = |r: (f32, f32, f32, f32)| (r.0 + sx, r.1 + sy, r.2, r.3);
        let hv = self.hover;
        let hovering = |r: (f32, f32, f32, f32)| hv.is_some_and(|(x, y)| x >= r.0 + sx && x < r.0 + sx + r.2 && y >= r.1 + sy && y < r.1 + sy + r.3);
        let dark = theme.appearance == Appearance::Dark;
        let mut c = Canvas::new((W + 2.0 * SHADOW) as i32, (H + 2.0 * SHADOW) as i32, scale as f32).unwrap();
        let (ox, oy) = (SHADOW, SHADOW);
        c.shadow_rrect(ox, oy + 12.0, W, H, 14.0, 40.0, 120);
        if let Some(bd) = backdrop {
            c.fill_rrect_backdrop(ox, oy, SIDE, H, 14.0, bd, (sx, sy), (sw as f32, sh as f32));
        }
        c.fill_rrect(ox, oy, W, H, 14.0, theme.window_bg);
        c.fill_rrect_corners(ox, oy, SIDE, H, [14.0, 0.0, 0.0, 14.0], theme.sidebar_bg);
        c.fill_rrect(ox + SIDE, oy, 1.0, H, 0.0, theme.separator);

        // ================= conteúdo (desenhado primeiro, recortado depois pela moldura)
        // num canvas à parte, recortado pela área do conteúdo (páginas longas não vazam da janela)
        let mut frame = std::mem::replace(&mut c, Canvas::new((W + 2.0 * SHADOW) as i32, (H + 2.0 * SHADOW) as i32, scale as f32).unwrap());
        let cx = ox + SIDE + 28.0;
        let cw = W - SIDE - 56.0;
        let top = oy + 50.0;
        let mut y = oy + 66.0 - self.scroll;
        let start_y = y;
        y = special.draw(&mut c, self.page, cx, y, cw, theme, &mut |a, r| self.hits.push((SetAction::Act(a), to_screen(r))));

        for g in groups {
            if !g.title.is_empty() {
                c.text(&g.title, cx + 2.0, y, 13.0, Weight::Bold, theme.label);
                y += 24.0;
            }
            if g.rows.is_empty() {
                continue;
            }
            let rh_of = |r: &Row, c: &Canvas| match r {
                Row::Note(t) => (crate::shell::notch::wrap(t, cw - 32.0, 6, c).len() as f32 * 16.0 + 22.0).max(40.0),
                Row::Info { .. } => 40.0,
                Row::App { .. } => 64.0,
                Row::Slider { .. } => 58.0,
                _ => ROW_H,
            };
            let gh: f32 = g.rows.iter().map(|r| rh_of(r, &c)).sum();
            c.fill_rrect(cx, y, cw, gh, 10.0, if dark { Rgba(255, 255, 255, 13) } else { Rgba::rgb(255, 255, 255) });
            c.stroke_rrect(cx, y, cw, gh, 10.0, 1.0, theme.fg(0.06));
            let mut ry = y;
            let n = g.rows.len();
            for (i, row) in g.rows.iter().enumerate() {
                let rh = rh_of(row, &c);
                let visible = ry + rh > top && ry < oy + H;
                let mut text_x = cx + 16.0;
                let tile = |c: &mut Canvas, icon: &str, col: (u8, u8, u8)| {
                    c.fill_rrect_vgradient(cx + 14.0, ry + (rh - 24.0) / 2.0, 24.0, 24.0, 6.0, tint(col, 1.12), tint(col, 1.0));
                    c.symbol(icon, cx + 26.0, ry + rh / 2.0, 16.0, Rgba::rgb(255, 255, 255));
                };
                let row_hover = hovering((cx, ry, cw, rh)) && visible;
                match row {
                    Row::Switch { icon, color, label, sub, on, act } => {
                        tile(&mut c, icon, *color);
                        text_x = cx + 50.0;
                        if sub.is_empty() {
                            c.text_vcenter(label, text_x, ry, rh, 13.0, Weight::Regular, theme.label);
                        } else {
                            c.text(label, text_x, ry + 7.0, 13.0, Weight::Regular, theme.label);
                            let s = c.ellipsized(sub, 11.0, Weight::Regular, cw - 130.0);
                            c.text(&s, text_x, ry + 25.0, 11.0, Weight::Regular, theme.secondary);
                        }
                        let (tx, ty) = (cx + cw - 16.0 - 40.0, ry + (rh - 24.0) / 2.0);
                        c.fill_rrect(tx, ty, 40.0, 24.0, 12.0, if *on { theme.accent } else { theme.fg(0.16) });
                        let kx = if *on { tx + 28.0 } else { tx + 12.0 };
                        c.fill_circle(kx, ty + 12.5, 10.5, Rgba(0, 0, 0, 40));
                        c.fill_circle(kx, ty + 12.0, 10.0, Rgba::rgb(255, 255, 255));
                        if visible {
                            self.hits.push((SetAction::Act(act.clone()), to_screen((tx - 8.0, ry, 56.0, rh))));
                        }
                    }
                    Row::Slider { icon, color, label, value, template, root } => {
                        tile(&mut c, icon, *color);
                        text_x = cx + 50.0;
                        c.text(label, text_x, ry + 8.0, 13.0, Weight::Regular, theme.label);
                        let pct = format!("{:.0}%", value * 100.0);
                        let pw = c.measure(&pct, 12.0, Weight::Regular);
                        c.text(&pct, cx + cw - 16.0 - pw, ry + 8.0, 12.0, Weight::Regular, theme.secondary);
                        let (tx, ty, tw) = (text_x, ry + 34.0, cx + cw - 16.0 - text_x);
                        c.fill_rrect(tx, ty, tw, 5.0, 2.5, theme.fg(0.14));
                        c.fill_rrect(tx, ty, (tw * value).max(5.0), 5.0, 2.5, theme.accent);
                        c.fill_circle(tx + tw * value, ty + 2.5, 9.0, Rgba(0, 0, 0, 40));
                        c.fill_circle(tx + tw * value, ty + 2.0, 8.5, Rgba::rgb(255, 255, 255));
                        if visible {
                            let r = to_screen((tx - 6.0, ry + 22.0, tw + 12.0, 28.0));
                            self.hits.push((SetAction::Slider(template.clone(), *root, r.0 + 6.0, tw), r));
                        }
                    }
                    Row::Choice { icon, color, label, options, selected } => {
                        tile(&mut c, icon, *color);
                        c.text_vcenter(label, cx + 50.0, ry, rh, 13.0, Weight::Regular, theme.label);
                        let widths: Vec<f32> = options.iter().map(|(t, _)| c.measure(t, 12.0, Weight::Medium) + 20.0).collect();
                        let total: f32 = widths.iter().sum();
                        let mut bx = cx + cw - 16.0 - total;
                        let by = ry + (rh - 26.0) / 2.0;
                        c.fill_rrect(bx - 2.0, by - 2.0, total + 4.0, 30.0, 8.0, theme.fg(0.08));
                        for (k, ((t, act), w)) in options.iter().zip(widths).enumerate() {
                            if k == *selected {
                                c.shadow_rrect_add(bx, by + 1.0, w, 26.0, 6.0, 4.0, 50);
                                c.fill_rrect(bx, by, w, 26.0, 6.0, if dark { Rgba::rgb(0x63, 0x63, 0x66) } else { Rgba::rgb(255, 255, 255) });
                            }
                            c.text_vcenter(t, bx + 10.0, by, 26.0, 12.0, Weight::Medium, theme.label);
                            if visible {
                                self.hits.push((SetAction::Act(act.clone()), to_screen((bx, by, w, 26.0))));
                            }
                            bx += w;
                        }
                    }
                    Row::Item { icon, color, title, sub, trailing, checked, act } => {
                        if row_hover && *act != Act::Nothing {
                            c.fill_rrect(cx + 4.0, ry + 3.0, cw - 8.0, rh - 6.0, 7.0, theme.fg(0.05));
                        }
                        tile(&mut c, icon, *color);
                        text_x = cx + 50.0;
                        let tmax = cw - 50.0 - 40.0 - c.measure(trailing, 12.0, Weight::Regular);
                        let t = c.ellipsized(title, 13.0, Weight::Regular, tmax);
                        if sub.is_empty() {
                            c.text_vcenter(&t, text_x, ry, rh, 13.0, Weight::Regular, theme.label);
                        } else {
                            c.text(&t, text_x, ry + 7.0, 13.0, Weight::Regular, theme.label);
                            let s = c.ellipsized(sub, 11.0, Weight::Regular, tmax);
                            c.text(&s, text_x, ry + 25.0, 11.0, Weight::Regular, theme.secondary);
                        }
                        let mut rx = cx + cw - 16.0;
                        if *checked {
                            c.symbol("check", rx - 8.0, ry + rh / 2.0, 16.0, theme.accent);
                            rx -= 24.0;
                        }
                        if !trailing.is_empty() {
                            let tw = c.measure(trailing, 12.0, Weight::Regular);
                            c.text_vcenter(trailing, rx - tw, ry, rh, 12.0, Weight::Regular, theme.secondary);
                        }
                        if visible && *act != Act::Nothing {
                            self.hits.push((SetAction::Act(act.clone()), to_screen((cx, ry, cw, rh))));
                        }
                    }
                    Row::Field { id, label, placeholder, secret } => {
                        c.text_vcenter(label, cx + 16.0, ry, rh, 13.0, Weight::Regular, theme.label);
                        let (fx, fw) = (cx + cw * 0.42, cw * 0.58 - 16.0);
                        let fy = ry + (rh - 28.0) / 2.0;
                        let focused = focus == Some(id.as_str());
                        c.fill_rrect(fx, fy, fw, 28.0, 7.0, if dark { Rgba(255, 255, 255, 20) } else { Rgba::rgb(0xf2, 0xf2, 0xf4) });
                        c.stroke_rrect(fx, fy, fw, 28.0, 7.0, if focused { 2.0 } else { 1.0 }, if focused { theme.accent } else { theme.fg(0.12) });
                        let val = fields.get(id).cloned().unwrap_or_default();
                        let shown = if *secret { "●".repeat(val.chars().count()) } else { val.clone() };
                        let tw = if shown.is_empty() {
                            c.text_vcenter(placeholder, fx + 10.0, fy, 28.0, 12.0, Weight::Regular, theme.tertiary);
                            0.0
                        } else {
                            let t = c.ellipsized(&shown, 12.0, Weight::Regular, fw - 20.0);
                            c.text_vcenter(&t, fx + 10.0, fy, 28.0, 12.0, Weight::Regular, theme.label)
                        };
                        if focused {
                            c.fill_rrect(fx + 11.0 + tw, fy + 7.0, 1.5, 14.0, 0.0, theme.accent);
                        }
                        if visible {
                            self.hits.push((SetAction::Act(Act::Focus(id.clone())), to_screen((fx, fy, fw, 28.0))));
                        }
                    }
                    Row::Button { label, button, act, primary } => {
                        c.text_vcenter(label, cx + 16.0, ry, rh, 13.0, Weight::Regular, theme.label);
                        let bw = c.measure(button, 12.0, Weight::Medium) + 26.0;
                        let (bx, by) = (cx + cw - 16.0 - bw, ry + (rh - 26.0) / 2.0);
                        let (bg, tc) = if *primary { (theme.accent, theme.text_on_accent) } else if dark { (Rgba::rgb(0x55, 0x55, 0x58), theme.label) } else { (Rgba::rgb(255, 255, 255), theme.label) };
                        c.fill_rrect(bx, by, bw, 26.0, 7.0, bg);
                        if !*primary {
                            c.stroke_rrect(bx, by, bw, 26.0, 7.0, 1.0, theme.fg(0.10));
                        }
                        c.text_vcenter(button, bx + 13.0, by, 26.0, 12.0, Weight::Medium, tc);
                        if visible {
                            self.hits.push((SetAction::Act(act.clone()), to_screen((bx, by, bw, 26.0))));
                        }
                    }
                    Row::App { icon, title, sub, button, act } => {
                        // ícone do app (do appstream), nome, descrição e o botão em pílula
                        let pm = self.icons.entry(icon.clone()).or_insert_with(|| if icon.is_empty() { None } else { crate::settings_ctl::load_image(icon, 96) }).clone();
                        match pm {
                            Some(pm) => c.blit(&pm, cx + 14.0, ry + 10.0, 44.0, 44.0, 1.0),
                            None => {
                                c.fill_rrect(cx + 14.0, ry + 10.0, 44.0, 44.0, 10.0, theme.fg(0.10));
                                c.symbol("apps", cx + 36.0, ry + 32.0, 24.0, theme.secondary);
                            }
                        }
                        text_x = cx + 70.0;
                        let bw = c.measure(button, 12.0, Weight::Bold) + 30.0;
                        let tmax = cw - 70.0 - bw - 30.0;
                        let t = c.ellipsized(title, 14.0, Weight::Semibold, tmax);
                        c.text(&t, text_x, ry + 13.0, 14.0, Weight::Semibold, theme.label);
                        let sub = c.ellipsized(sub, 12.0, Weight::Regular, tmax);
                        c.text(&sub, text_x, ry + 33.0, 12.0, Weight::Regular, theme.secondary);
                        let (bx, by) = (cx + cw - 16.0 - bw, ry + (rh - 26.0) / 2.0);
                        c.fill_rrect(bx, by, bw, 26.0, 13.0, theme.fg(0.10));
                        c.text_vcenter(button, bx + 15.0, by, 26.0, 12.0, Weight::Bold, theme.accent);
                        if visible {
                            self.hits.push((SetAction::Act(act.clone()), to_screen((bx, by, bw, 26.0))));
                        }
                    }
                    Row::Info { label, value } => {
                        c.text_vcenter(label, text_x, ry, rh, 13.0, Weight::Regular, theme.label);
                        let v = c.ellipsized(value, 13.0, Weight::Regular, cw * 0.6);
                        let vw = c.measure(&v, 13.0, Weight::Regular);
                        c.text_vcenter(&v, cx + cw - 16.0 - vw, ry, rh, 13.0, Weight::Regular, theme.secondary);
                    }
                    Row::Note(t) => {
                        let mut ty = ry + 11.0;
                        for line in crate::shell::notch::wrap(t, cw - 32.0, 6, &c) {
                            c.text(&line, cx + 16.0, ty, 12.0, Weight::Regular, theme.secondary);
                            ty += 16.0;
                        }
                    }
                }
                if i + 1 < n {
                    c.fill_rrect(text_x, ry + rh - 0.5, cx + cw - 16.0 - text_x, 1.0, 0.0, theme.separator);
                }
                ry += rh;
            }
            y += gh + 24.0;
        }
        self.content_h = y - start_y;
        // hits do conteúdo que ficaram sob o cabeçalho não valem
        {
            let f = scale as f32;
            if let (Some(mut mask), Some(path)) = (
                resvg::tiny_skia::Mask::new(frame.pm.width(), frame.pm.height()),
                crate::ui::canvas::rrect_path(ox + SIDE + 1.0, oy, W - SIDE - 1.0, H, [0.0, 14.0, 14.0, 0.0]),
            ) {
                mask.fill_path(&path, resvg::tiny_skia::FillRule::Winding, true, resvg::tiny_skia::Transform::from_scale(f, f));
                frame.pm.draw_pixmap(0, 0, c.pm.as_ref(), &resvg::tiny_skia::PixmapPaint::default(), resvg::tiny_skia::Transform::identity(), Some(&mask));
            }
        }
        let mut c = frame;
        // hits do conteúdo abaixo da janela não valem
        let window_bottom = oy + H + sy;
        self.hits.retain(|(_, r)| r.1 < window_bottom);
        let header_bottom = oy + 50.0 + sy;
        self.hits.retain(|(_, r)| r.1 + r.3 > header_bottom);

        // ================= cabeçalho (cobre o conteúdo rolado)
        c.fill_rrect_corners(ox + SIDE + 1.0, oy, W - SIDE - 1.0, 50.0, [0.0, 14.0, 0.0, 0.0], theme.window_bg);
        c.fill_rrect(ox + SIDE + 1.0, oy + H - 6.0, W - SIDE - 1.0, 6.0, 0.0, theme.window_bg);
        c.symbol("chevron_left", ox + SIDE + 24.0, oy + 25.0, 20.0, theme.tertiary);
        c.symbol("chevron_right", ox + SIDE + 48.0, oy + 25.0, 20.0, theme.tertiary);
        let title = self.pages().iter().find(|p| p.0 == self.page).map(|p| p.1).unwrap_or(match self.page {
            "users" => "Usuários",
            "s:busca" => "Busca",
            _ => "",
        });
        c.text_vcenter(title, ox + SIDE + 66.0, oy + 9.0, 32.0, 15.0, Weight::Bold, theme.label);
        c.fill_rrect(ox + SIDE + 1.0, oy + 49.0, W - SIDE - 1.0, 1.0, 0.0, theme.separator);

        // ================= barra lateral
        for (i, col) in [theme.tl_close, theme.tl_min, theme.tl_max].iter().enumerate() {
            c.fill_circle(ox + 20.0 + i as f32 * 20.0, oy + 20.0, 6.0, *col);
        }
        self.hits.push((SetAction::Close, to_screen((ox + 12.0, oy + 12.0, 16.0, 16.0))));
        let (fx, fy, fw) = (ox + 12.0, oy + 42.0, SIDE - 24.0);
        c.fill_rrect(fx, fy, fw, 28.0, 8.0, theme.fg(0.08));
        c.symbol("search", fx + 15.0, fy + 14.0, 15.0, theme.secondary);
        if self.query.is_empty() {
            c.text_vcenter("Buscar", fx + 28.0, fy, 28.0, 13.0, Weight::Regular, theme.tertiary);
        } else {
            let tw = c.text_vcenter(&self.query, fx + 28.0, fy, 28.0, 13.0, Weight::Regular, theme.label);
            c.fill_rrect(fx + 29.0 + tw, fy + 7.0, 1.5, 14.0, 0.0, theme.accent);
        }
        self.hits.push((SetAction::Search, to_screen((fx, fy, fw, 28.0))));

        let (ux, uy) = (ox + 10.0, oy + 80.0);
        let card = (ux, uy, SIDE - 20.0, 54.0);
        if !self.store {
        let sel_user = self.page == "users";
        if sel_user {
            c.fill_rrect(card.0, card.1, card.2, card.3, 9.0, theme.accent);
        } else if hovering(card) {
            c.fill_rrect(card.0, card.1, card.2, card.3, 9.0, theme.fg(0.06));
        }
        match &special.avatar {
            Some(pm) => c.blit(pm, ux + 8.0, uy + 9.0, 36.0, 36.0, 1.0),
            None => {
                c.fill_circle(ux + 26.0, uy + 27.0, 18.0, Rgba::rgb(0x0b, 0x0f, 0x1f));
                text_glyph::obelisk_logo(&mut c, ux + 26.0, uy + 27.0, 20.0, Rgba::rgb(0xee, 0xf1, 0xff));
            }
        }
        let name = c.ellipsized(&special.user, 13.0, Weight::Semibold, SIDE - 90.0);
        let (nc, sc) = if sel_user { (theme.text_on_accent, theme.text_on_accent.with_alpha(220)) } else { (theme.label, theme.secondary) };
        c.text(&name, ux + 54.0, uy + 10.0, 13.0, Weight::Semibold, nc);
        c.text("Usuário e foto", ux + 54.0, uy + 29.0, 11.0, Weight::Regular, sc);
        self.hits.push((SetAction::Page("users"), to_screen(card)));
        }

        let visible = self.visible_pages();
        let list_top = if self.store { oy + 86.0 } else { oy + 144.0 };
        let list_bottom = oy + H - 8.0;
        let mut y = list_top + 2.0 - self.side_scroll;
        let y0 = y;
        let shown = |y: f32, h: f32| y >= list_top - 1.0 && y + h <= list_bottom;
        let searching = !self.query.trim().is_empty();
        for (title, pages) in self.sections() {
            let items: Vec<_> = self.pages().iter().filter(|p| pages.contains(&p.0) && visible.contains(&p.0)).collect();
            if items.is_empty() {
                continue;
            }
            let collapsed = self.collapsed.contains(title) && !searching;
            if shown(y, 24.0) {
                c.text_vcenter(title, ox + 20.0, y, 24.0, 11.0, Weight::Bold, theme.tertiary);
                c.symbol(if collapsed { "chevron_right" } else { "expand_more" }, ox + SIDE - 26.0, y + 12.0, 14.0, theme.tertiary);
                self.hits.push((SetAction::Section(title), to_screen((ox + 10.0, y, SIDE - 20.0, 24.0))));
            }
            y += 24.0;
            if collapsed {
                continue;
            }
            for p in items {
                if !shown(y, 28.0) {
                    y += 30.0;
                    continue;
                }
                let sel = p.0 == self.page;
                let rect = (ox + 10.0, y, SIDE - 20.0, 28.0);
                if sel {
                    c.fill_rrect(rect.0, rect.1, rect.2, rect.3, 7.0, theme.accent);
                } else if hovering(rect) {
                    c.fill_rrect(rect.0, rect.1, rect.2, rect.3, 7.0, theme.fg(0.06));
                }
                c.fill_rrect_vgradient(ox + 18.0, y + 3.0, 22.0, 22.0, 6.0, tint(p.3, 1.12), tint(p.3, 1.0));
                c.symbol(p.2, ox + 29.0, y + 14.0, 15.0, Rgba::rgb(255, 255, 255));
                c.text_vcenter(p.1, ox + 50.0, y, 28.0, 13.0, Weight::Regular, if sel { theme.text_on_accent } else { theme.label });
                self.hits.push((SetAction::Page(p.0), to_screen(rect)));
                y += 30.0;
            }
        }
        self.side_h = y - y0;
        // indicação de que há mais itens abaixo/acima
        if self.side_h > list_bottom - list_top {
            let track = list_bottom - list_top;
            let frac = track / self.side_h;
            let pos = self.side_scroll / self.side_h * track;
            c.fill_rrect(ox + SIDE - 6.0, list_top + pos, 3.0, (track * frac).max(20.0), 1.5, theme.fg(0.18));
        }
        c.stroke_rrect(ox, oy, W, H, 14.0, 1.0, theme.fg(0.10));
        self.layer.set(&c);
    }
}

/// Partes com desenho próprio: prévias de aparência, cores de destaque, papéis de parede,
/// cabeçalho do "Sobre" e foto do usuário.
#[derive(Default)]
pub struct Special {
    pub user: String,
    pub avatar: Option<Pixmap>,
    pub appearance: String,
    pub accent: String,
    pub wallpapers: Vec<(String, Pixmap)>,
    pub wallpaper: String,
    /// telas: (nome, x, y, l, a lógicos, principal) e a tela sendo arrastada com a posição prévia
    pub displays: Vec<(String, (i32, i32, i32, i32), bool)>,
    pub drag: Option<(String, (i32, i32))>,
    /// tarefa em andamento (título, última linha, %): faixa no topo da página
    pub job: Option<(String, String, f32, bool)>,
}

/// Altura da área do arranjo de telas.
pub const ARRANGE_H: f32 = 210.0;

/// Escala (px da prévia por ponto lógico) e origem da prévia para caber `displays` em `cw`.
pub fn arrange_geometry(displays: &[(String, (i32, i32, i32, i32), bool)], cw: f32) -> (f32, (f32, f32)) {
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for (_, (x, y, w, h), _) in displays {
        x0 = x0.min(*x);
        y0 = y0.min(*y);
        x1 = x1.max(x + w);
        y1 = y1.max(y + h);
    }
    if displays.is_empty() {
        return (0.1, (0.0, 0.0));
    }
    // folga para arrastar uma tela para qualquer lado
    let (bw, bh) = ((x1 - x0) as f32 * 1.9, (y1 - y0) as f32 * 1.9);
    let f = ((cw - 40.0) / bw).min((ARRANGE_H - 30.0) / bh);
    let ox = cw / 2.0 - ((x0 + x1) as f32 / 2.0) * f;
    let oy = ARRANGE_H / 2.0 - ((y0 + y1) as f32 / 2.0) * f;
    (f, (ox, oy))
}

impl Special {
    fn key(&self) -> String {
        format!("{:?}|{}|{}|{}|{}|{}|{:?}|{:?}", self.job, self.appearance, self.accent, self.wallpaper, self.wallpapers.len(), self.avatar.is_some(), self.displays, self.drag)
    }

    /// Desenha o topo especial da página; devolve o novo y.
    fn draw(&self, c: &mut Canvas, page: &str, cx: f32, mut y: f32, cw: f32, theme: &Theme, hit: &mut dyn FnMut(Act, (f32, f32, f32, f32))) -> f32 {
        if let Some((title, line, pct, running)) = &self.job {
            if page.starts_with("s:") || page == "updates" {
                c.fill_rrect(cx, y, cw, 64.0, 10.0, theme.accent.with_alpha(28));
                c.stroke_rrect(cx, y, cw, 64.0, 10.0, 1.0, theme.accent.with_alpha(90));
                let t = c.ellipsized(title, 13.0, Weight::Semibold, cw - 120.0);
                c.text(&t, cx + 16.0, y + 10.0, 13.0, Weight::Semibold, theme.label);
                let status = if *running { format!("{:.0}%", pct) } else { "Concluído".to_string() };
                let sw2 = c.measure(&status, 12.0, Weight::Regular);
                c.text(&status, cx + cw - 16.0 - sw2, y + 10.0, 12.0, Weight::Regular, theme.secondary);
                c.fill_rrect(cx + 16.0, y + 32.0, cw - 32.0, 5.0, 2.5, theme.fg(0.12));
                c.fill_rrect(cx + 16.0, y + 32.0, ((cw - 32.0) * pct / 100.0).max(5.0), 5.0, 2.5, theme.accent);
                let l = c.ellipsized(line, 11.0, Weight::Regular, cw - 32.0);
                c.text(&l, cx + 16.0, y + 44.0, 11.0, Weight::Regular, theme.tertiary);
                y += 64.0 + 20.0;
            }
        }
        match page {
            "appearance" => {
                c.text("Aparência", cx + 2.0, y, 13.0, Weight::Bold, theme.label);
                y += 24.0;
                let opts = [("Claro", "light"), ("Escuro", "dark"), ("Automático", "auto")];
                let (pw, ph) = (132.0, 88.0);
                let gap = (cw - 3.0 * pw) / 2.0;
                for (i, (label, key)) in opts.iter().enumerate() {
                    let px = cx + i as f32 * (pw + gap);
                    appearance_preview(c, px, y, pw, ph, key, theme);
                    if self.appearance == *key {
                        c.stroke_rrect(px - 4.0, y - 4.0, pw + 8.0, ph + 8.0, 14.0, 3.0, theme.accent);
                    }
                    let lw = c.measure(label, 12.0, Weight::Medium);
                    c.text(label, px + (pw - lw) / 2.0, y + ph + 10.0, 12.0, Weight::Medium, if self.appearance == *key { theme.label } else { theme.secondary });
                    hit(Act::Set("appearance".into(), key.to_string()), (px - 4.0, y - 4.0, pw + 8.0, ph + 30.0));
                }
                y += ph + 46.0;
                c.text("Cor de destaque", cx + 2.0, y, 13.0, Weight::Bold, theme.label);
                y += 24.0;
                let n = Theme::ACCENTS.len() as f32;
                let step = 36.0;
                c.fill_rrect(cx, y, cw, 56.0, 10.0, theme.fg(0.05));
                c.stroke_rrect(cx, y, cw, 56.0, 10.0, 1.0, theme.fg(0.06));
                let mut ax = cx + (cw - n * step) / 2.0 + step / 2.0;
                for (key, _l, (r, g, b)) in Theme::ACCENTS {
                    let col = Rgba::rgb(r, g, b);
                    if self.accent == key {
                        c.fill_circle(ax, y + 28.0, 14.0, col.with_alpha(90));
                    }
                    c.fill_circle(ax, y + 28.0, 10.0, col);
                    if self.accent == key {
                        c.fill_circle(ax, y + 28.0, 3.5, Rgba::rgb(255, 255, 255));
                    }
                    hit(Act::Set("accent".into(), key.into()), (ax - 15.0, y + 13.0, 30.0, 30.0));
                    ax += step;
                }
                y + 56.0 + 26.0
            }
            "displays" => {
                c.text("Organização", cx + 2.0, y, 13.0, Weight::Bold, theme.label);
                y += 24.0;
                c.fill_rrect(cx, y, cw, ARRANGE_H, 10.0, theme.fg(0.05));
                c.stroke_rrect(cx, y, cw, ARRANGE_H, 10.0, 1.0, theme.fg(0.06));
                let (f, (ox, oy)) = arrange_geometry(&self.displays, cw);
                // a arrastada por último (por cima)
                let mut list: Vec<_> = self.displays.iter().collect();
                list.sort_by_key(|d| self.drag.as_ref().is_some_and(|g| g.0 == d.0));
                for (name, (dx, dy, dw, dh), primary) in list {
                    let (dx, dy) = match &self.drag {
                        Some((n, p)) if n == name => *p,
                        _ => (*dx, *dy),
                    };
                    let (rx, ry, rw, rh) = (cx + ox + dx as f32 * f, y + oy + dy as f32 * f, *dw as f32 * f, *dh as f32 * f);
                    let dragging = self.drag.as_ref().is_some_and(|g| &g.0 == name);
                    if dragging {
                        c.shadow_rrect_add(rx, ry + 3.0, rw, rh, 6.0, 10.0, 90);
                    }
                    c.fill_rrect_vgradient(rx, ry, rw, rh, 6.0, Rgba::rgb(0x3b, 0x5b, 0xe0), Rgba::rgb(0x6a, 0x3f, 0xc0));
                    c.stroke_rrect(rx, ry, rw, rh, 6.0, if dragging { 2.0 } else { 1.0 }, if dragging { theme.accent } else { Rgba(255, 255, 255, 90) });
                    if *primary {
                        // a principal tem a barra de menus
                        c.fill_rrect_corners(rx + 1.0, ry + 1.0, rw - 2.0, (rh * 0.08).max(4.0), [5.0, 5.0, 0.0, 0.0], Rgba(255, 255, 255, 220));
                    }
                    let label = c.ellipsized(name, 11.0, Weight::Semibold, rw - 8.0);
                    let lw = c.measure(&label, 11.0, Weight::Semibold);
                    c.text(&label, rx + (rw - lw) / 2.0, ry + rh / 2.0 - 7.0, 11.0, Weight::Semibold, Rgba(255, 255, 255, 240));
                    hit(Act::DisplayGrab(name.clone()), (rx, ry, rw, rh));
                }
                let tip = if self.displays.len() > 1 { "Arraste as telas para a posição delas na sua mesa." } else { "Conecte outra tela para organizar." };
                let tw = c.measure(tip, 11.0, Weight::Regular);
                c.text(tip, cx + (cw - tw) / 2.0, y + ARRANGE_H - 20.0, 11.0, Weight::Regular, theme.secondary);
                y + ARRANGE_H + 26.0
            }
            "wallpaper" => {
                c.text("Papel de Parede", cx + 2.0, y, 13.0, Weight::Bold, theme.label);
                y += 24.0;
                let cols = 4.0;
                let gap = 14.0;
                let tw = (cw - gap * (cols - 1.0)) / cols;
                let th = tw * 10.0 / 16.0;
                for (i, (path, pm)) in self.wallpapers.iter().enumerate() {
                    let (col, row) = (i as f32 % cols, (i as f32 / cols).floor());
                    let (x, yy) = (cx + col * (tw + gap), y + row * (th + 30.0));
                    c.blit(pm, x, yy, tw, th, 1.0);
                    c.stroke_rrect(x, yy, tw, th, 8.0, 1.0, theme.fg(0.12));
                    if *path == self.wallpaper || (self.wallpaper.is_empty() && i == 0) {
                        c.stroke_rrect(x - 3.0, yy - 3.0, tw + 6.0, th + 6.0, 10.0, 3.0, theme.accent);
                    }
                    let name = std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    let name = c.ellipsized(&name, 11.0, Weight::Regular, tw);
                    c.text(&name, x, yy + th + 6.0, 11.0, Weight::Regular, theme.secondary);
                    hit(Act::Set("wallpaper".into(), path.clone()), (x, yy, tw, th));
                }
                let rows = (self.wallpapers.len() as f32 / cols).ceil();
                y + rows * (th + 30.0) + 10.0
            }
            "about" => {
                let lx = cx + cw / 2.0;
                c.fill_rrect_vgradient(lx - 44.0, y, 88.0, 88.0, 20.0, Rgba::rgb(0x1c, 0x24, 0x4a), Rgba::rgb(0x0b, 0x0f, 0x1f));
                text_glyph::obelisk_logo(c, lx, y + 44.0, 52.0, Rgba::rgb(0xee, 0xf1, 0xff));
                y += 100.0;
                let tw = c.measure("Obelisk", 26.0, Weight::Bold);
                c.text("Obelisk", lx - tw / 2.0, y, 26.0, Weight::Bold, theme.label);
                let v = format!("Versão {} · base Arch Linux", env!("CARGO_PKG_VERSION"));
                let vw = c.measure(&v, 12.0, Weight::Regular);
                c.text(&v, lx - vw / 2.0, y + 34.0, 12.0, Weight::Regular, theme.secondary);
                y + 70.0
            }
            _ => y,
        }
    }
}

/// Miniatura de tela para os cartões de aparência.
fn appearance_preview(c: &mut Canvas, x: f32, y: f32, w: f32, h: f32, mode: &str, theme: &Theme) {
    let paint = |c: &mut Canvas, x: f32, w: f32, dark: bool, rl: f32, rr: f32| {
        let (top, bottom) = if dark { (Rgba::rgb(0x1b, 0x22, 0x5a), Rgba::rgb(0x4b, 0x1f, 0x6e)) } else { (Rgba::rgb(0x9f, 0xc4, 0xff), Rgba::rgb(0xe9, 0xd1, 0xff)) };
        c.fill_rrect_corners(x, y, w, h, [rl, rr, rr, rl], bottom);
        c.fill_rrect_corners(x, y, w, h * 0.6, [rl, rr, 0.0, 0.0], top);
        let (win, bar) = if dark { (Rgba::rgb(0x2a, 0x2a, 0x2d), Rgba::rgb(0x3a, 0x3a, 0x3d)) } else { (Rgba::rgb(0xff, 0xff, 0xff), Rgba::rgb(0xec, 0xec, 0xee)) };
        c.fill_rrect(x + 10.0, y + 18.0, w * 0.75, h - 30.0, 5.0, win);
        c.fill_rrect_corners(x + 10.0, y + 18.0, w * 0.75, 10.0, [5.0, 5.0, 0.0, 0.0], bar);
        for (i, col) in [Rgba::rgb(0xff, 0x5f, 0x57), Rgba::rgb(0xfe, 0xbc, 0x2e), Rgba::rgb(0x28, 0xc8, 0x40)].iter().enumerate() {
            c.fill_circle(x + 16.0 + i as f32 * 6.0, y + 23.0, 2.0, *col);
        }
        c.fill_rrect(x, y, w, 7.0, 0.0, if dark { Rgba(0, 0, 0, 70) } else { Rgba(255, 255, 255, 120) });
    };
    match mode {
        "light" => paint(c, x, w, false, 10.0, 10.0),
        "dark" => paint(c, x, w, true, 10.0, 10.0),
        _ => {
            paint(c, x, w / 2.0, false, 10.0, 0.0);
            paint(c, x + w / 2.0, w / 2.0, true, 0.0, 10.0);
        }
    }
    c.stroke_rrect(x, y, w, h, 10.0, 1.0, theme.fg(0.12));
}
