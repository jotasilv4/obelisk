//! Central de Notificações (clique no relógio): cartões das notificações recebidas.

use resvg::tiny_skia::Pixmap;

use super::layer::Layer;
use crate::notifications::Notif;
use crate::ui::canvas::Canvas;
use crate::ui::icons::IconStore;
use crate::ui::text::Weight;
use crate::ui::theme::Theme;

pub const WIDTH: f32 = 344.0;
const CARD_H: f32 = 76.0;
const GAP: f32 = 10.0;
const SMALL: f32 = 160.0;
const MEDIUM: f32 = 160.0;
const SHADOW: f32 = 28.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NcHit {
    Clear,
    Close(usize),
    Card(usize),
}

#[derive(Default)]
pub struct NotifCenter {
    pub open: bool,
    pub layer: Layer,
    pub pos: (f32, f32),
    pub size: (f32, f32),
    hits: Vec<(NcHit, (f32, f32, f32, f32))>,
    key: Option<String>,
}

impl NotifCenter {
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.key = None;
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        self.open
            && (x as f32) >= self.pos.0
            && (x as f32) < self.pos.0 + self.size.0
            && (y as f32) >= self.pos.1
            && (y as f32) < self.pos.1 + self.size.1
    }

    pub fn hit(&self, x: f64, y: f64) -> Option<NcHit> {
        let (x, y) = (x as f32, y as f32);
        self.hits.iter().find(|(_, r)| x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3).map(|(h, _)| *h)
    }

    pub fn buf_pos(&self) -> (i32, i32) {
        ((self.pos.0 - SHADOW) as i32, (self.pos.1 - SHADOW) as i32)
    }

    pub fn update(&mut self, theme: &Theme, icons: &mut IconStore, list: &[Notif], sw: i32, sh: i32, scale: i32, backdrop: Option<&Pixmap>) {
        let n = list.len().min(6);
        // macOS Sonoma: notificações em cima, widgets embaixo (sem título solto)
        let notif_h = if n == 0 { 0.0 } else { 34.0 + n as f32 * (CARD_H + GAP) + 6.0 };
        let h = notif_h + SMALL + GAP * 2.0 + MEDIUM;
        self.size = (WIDTH, h);
        self.pos = (sw as f32 - 6.0 - WIDTH, 35.0);
        let now = chrono::Local::now();
        let key = format!("{}|{}|{}|{}|{}|{}", now.format("%H:%M"), theme.accent.0, list.iter().map(|x| x.id.to_string()).collect::<Vec<_>>().join(","), theme.appearance as u8, sw, scale);
        if self.key.as_deref() == Some(key.as_str()) && self.layer.is_set() {
            return;
        }
        self.key = Some(key);
        self.hits.clear();
        let (ox, oy) = (SHADOW, SHADOW);
        let (sx, sy) = (self.pos.0 - SHADOW, self.pos.1 - SHADOW);
        let mut c = Canvas::new((WIDTH + 2.0 * SHADOW) as i32, (h + 2.0 * SHADOW) as i32, scale as f32).unwrap();
        let to_screen = |r: (f32, f32, f32, f32)| (r.0 + sx, r.1 + sy, r.2, r.3);

        let mut y = oy;
        if n > 0 {
            // cabeçalho discreto com "Limpar" (pílula de vidro)
            let lw = c.measure("Limpar", 12.0, Weight::Semibold) + 20.0;
            let lx = ox + WIDTH - lw;
            if let Some(bd) = backdrop {
                c.fill_rrect_backdrop(lx, oy + 2.0, lw, 24.0, 12.0, bd, (sx, sy), (sw as f32, sh as f32));
            }
            c.fill_rrect(lx, oy + 2.0, lw, 24.0, 12.0, theme.menu_bg);
            c.text_vcenter("Limpar", lx + 10.0, oy + 2.0, 24.0, 12.0, Weight::Semibold, theme.label);
            self.hits.push((NcHit::Clear, to_screen((lx, oy + 2.0, lw, 24.0))));
            y += 34.0;
        }
        for (i, nt) in list.iter().take(6).enumerate() {
            c.shadow_rrect_add(ox, y + 4.0, WIDTH, CARD_H, 18.0, 12.0, 60);
            if let Some(bd) = backdrop {
                c.fill_rrect_backdrop(ox, y, WIDTH, CARD_H, 18.0, bd, (sx, sy), (sw as f32, sh as f32));
            }
            c.fill_rrect(ox, y, WIDTH, CARD_H, 18.0, theme.menu_bg);
            c.stroke_rrect(ox, y, WIDTH, CARD_H, 18.0, 1.0, theme.menu_border);
            let icon = if nt.icon.is_empty() { nt.app.to_lowercase() } else { nt.icon.clone() };
            let pm = icons.get_theme(&icon, &nt.app).clone();
            c.blit(&pm, ox + 12.0, y + 14.0, 36.0, 36.0, 1.0);
            let tx = ox + 60.0;
            let tmax = WIDTH - 60.0 - 34.0;
            let app = c.ellipsized(if nt.app.is_empty() { "Notificação" } else { &nt.app }, 11.0, Weight::Semibold, tmax - 60.0);
            c.text(&app, tx, y + 10.0, 11.0, Weight::Semibold, if nt.critical { theme.red } else { theme.secondary });
            let secs = nt.at.elapsed().as_secs();
            let when = if secs < 60 { "agora".to_string() } else if secs < 3600 { format!("há {} min", secs / 60) } else { format!("há {} h", secs / 3600) };
            let ww = c.measure(&when, 11.0, Weight::Regular);
            c.text(&when, ox + WIDTH - 34.0 - ww, y + 10.0, 11.0, Weight::Regular, theme.tertiary);
            let t = c.ellipsized(&nt.summary, 13.0, Weight::Bold, tmax);
            c.text(&t, tx, y + 26.0, 13.0, Weight::Bold, theme.label);
            for (k, line) in super::notch::wrap(&nt.body, tmax, 2, &c).iter().enumerate() {
                c.text(line, tx, y + 44.0 + k as f32 * 15.0, 12.0, Weight::Regular, theme.secondary);
            }
            c.symbol("close", ox + WIDTH - 18.0, y + 16.0, 14.0, theme.tertiary);
            self.hits.push((NcHit::Close(i), to_screen((ox + WIDTH - 30.0, y + 4.0, 26.0, 26.0))));
            self.hits.push((NcHit::Card(i), to_screen((ox, y, WIDTH, CARD_H))));
            y += CARD_H + GAP;
        }
        if n > 0 {
            y += 6.0;
        }

        // ---------------- widgets
        let card = |c: &mut Canvas, x: f32, y: f32, w: f32, h: f32| {
            c.shadow_rrect_add(x, y + 4.0, w, h, 22.0, 12.0, 50);
            if let Some(bd) = backdrop {
                c.fill_rrect_backdrop(x, y, w, h, 22.0, bd, (sx, sy), (sw as f32, sh as f32));
            }
            c.fill_rrect(x, y, w, h, 22.0, theme.menu_bg);
            c.stroke_rrect(x, y, w, h, 22.0, 1.0, theme.menu_border);
        };
        let half = (WIDTH - GAP) / 2.0;
        // calendário pequeno: dia da semana em vermelho e o dia grande
        use chrono::Datelike;
        const DIAS: [&str; 7] = ["SEGUNDA", "TERÇA", "QUARTA", "QUINTA", "SEXTA", "SÁBADO", "DOMINGO"];
        card(&mut c, ox, y, half, SMALL);
        c.text(DIAS[now.weekday().num_days_from_monday() as usize], ox + 16.0, y + 16.0, 11.0, Weight::Bold, theme.red);
        c.text(&now.day().to_string(), ox + 14.0, y + 30.0, 46.0, Weight::Regular, theme.label);
        c.text("Nenhum evento hoje", ox + 16.0, y + SMALL - 28.0, 11.0, Weight::Medium, theme.secondary);
        // relógio: hora grande e cidade
        let rx = ox + half + GAP;
        card(&mut c, rx, y, half, SMALL);
        let tz = std::fs::read_link("/etc/localtime").ok().map(|p| p.to_string_lossy().rsplit('/').next().unwrap_or("").replace('_', " ")).unwrap_or_default();
        c.text(&tz, rx + 16.0, y + 16.0, 11.0, Weight::Bold, theme.secondary);
        c.text(&now.format("%H:%M").to_string(), rx + 14.0, y + 34.0, 40.0, Weight::Semibold, theme.label);
        c.text("Hoje", rx + 16.0, y + SMALL - 28.0, 11.0, Weight::Medium, theme.secondary);
        y += SMALL + GAP;
        // calendário do mês (médio)
        card(&mut c, ox, y, WIDTH, MEDIUM);
        const MESES: [&str; 12] = ["JANEIRO", "FEVEREIRO", "MARÇO", "ABRIL", "MAIO", "JUNHO", "JULHO", "AGOSTO", "SETEMBRO", "OUTUBRO", "NOVEMBRO", "DEZEMBRO"];
        c.text(MESES[now.month0() as usize], ox + 18.0, y + 14.0, 11.0, Weight::Bold, theme.red);
        let first = now.date_naive().with_day(1).unwrap();
        let offset = first.weekday().num_days_from_sunday() as f32;
        let days = if now.month() == 12 { 31 } else { (chrono::NaiveDate::from_ymd_opt(now.year(), now.month() + 1, 1).unwrap() - first).num_days() };
        let (gx, gy, cw, rh) = (ox + 18.0, y + 36.0, (WIDTH - 36.0) / 7.0, 18.0);
        for (i, d) in ["D", "S", "T", "Q", "Q", "S", "S"].iter().enumerate() {
            let w = c.measure(d, 10.0, Weight::Bold);
            c.text(d, gx + i as f32 * cw + (cw - w) / 2.0, gy, 10.0, Weight::Bold, theme.tertiary);
        }
        for d in 1..=days {
            let k = offset + (d - 1) as f32;
            let (col, row) = (k % 7.0, (k / 7.0).floor());
            let t = d.to_string();
            let w = c.measure(&t, 11.0, Weight::Semibold);
            let (cx0, cy0) = (gx + col * cw + cw / 2.0, gy + 16.0 + row * rh);
            let today = d as u32 == now.day();
            if today {
                c.fill_circle(cx0, cy0 + 7.0, 9.0, theme.red);
            }
            c.text(&t, cx0 - w / 2.0, cy0, 11.0, Weight::Semibold, if today { crate::ui::theme::Rgba::rgb(255, 255, 255) } else if col == 0.0 || col == 6.0 { theme.secondary } else { theme.label });
        }
        self.layer.set(&c);
    }
}
