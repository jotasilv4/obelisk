//! Widgets na mesa (macOS Sonoma): relógio, calendário do mês e estado do sistema.
//! Ficam sobre o papel de parede, abaixo das janelas; redesenham só quando o minuto muda.

use resvg::tiny_skia::Pixmap;

use super::layer::Layer;
use crate::services::SysStatus;
use crate::ui::canvas::Canvas;
use crate::ui::text::Weight;
use crate::ui::theme::{Rgba, Theme};

const W: f32 = 340.0;
const PAD: f32 = 16.0;

#[derive(Default)]
pub struct Widgets {
    pub layer: Layer,
    pub pos: (i32, i32),
    key: Option<String>,
}

impl Widgets {
    #[allow(clippy::too_many_arguments)]
    pub fn update(&mut self, theme: &Theme, st: &SysStatus, sw: i32, scale: i32, backdrop: Option<&Pixmap>, screen: (f32, f32), faded: bool) {
        use chrono::{Datelike, Local, NaiveDate, Timelike};
        let now = Local::now();
        let key = format!("{}|{:?}|{}|{sw}|{scale}|{faded}", now.format("%H:%M"), st.battery, theme.appearance as u8);
        let h = 170.0 + 16.0 + 230.0 + 16.0 + 90.0;
        self.pos = (sw - W as i32 - 24, 32 + 24);
        if self.key.as_deref() == Some(key.as_str()) && self.layer.is_set() {
            return;
        }
        self.key = Some(key);
        let mut c = Canvas::new(W as i32, h as i32, scale as f32).unwrap();
        let card = |c: &mut Canvas, y: f32, ch: f32| {
            if let Some(bd) = backdrop {
                c.fill_rrect_backdrop(0.0, y, W, ch, 22.0, bd, (self.pos.0 as f32, self.pos.1 as f32), screen);
            }
            c.fill_rrect(0.0, y, W, ch, 22.0, theme.menu_bg);
            c.stroke_rrect(0.0, y, W, ch, 22.0, 1.0, theme.menu_border);
        };
        // relógio
        card(&mut c, 0.0, 170.0);
        const DIAS: [&str; 7] = ["Segunda", "Terça", "Quarta", "Quinta", "Sexta", "Sábado", "Domingo"];
        const MESES: [&str; 12] = ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"];
        c.text(DIAS[now.weekday().num_days_from_monday() as usize], PAD, PAD, 13.0, Weight::Semibold, theme.red);
        let t = format!("{}:{:02}", now.hour(), now.minute());
        c.text(&t, PAD, PAD + 18.0, 72.0, Weight::Bold, theme.label);
        c.text(&format!("{} de {}", now.day(), MESES[now.month0() as usize]), PAD, 170.0 - PAD - 18.0, 15.0, Weight::Medium, theme.secondary);

        // calendário do mês
        let y0 = 186.0;
        card(&mut c, y0, 230.0);
        let mut mname = MESES[now.month0() as usize].to_string();
        mname.replace_range(0..1, &mname[0..1].to_uppercase());
        c.text(&mname, PAD, y0 + PAD, 13.0, Weight::Bold, theme.red);
        let cell = (W - 2.0 * PAD) / 7.0;
        for (i, d) in ["S", "T", "Q", "Q", "S", "S", "D"].iter().enumerate() {
            let x = PAD + i as f32 * cell + cell / 2.0 - 4.0;
            c.text(d, x, y0 + 40.0, 11.0, Weight::Semibold, theme.tertiary);
        }
        let first = NaiveDate::from_ymd_opt(now.year(), now.month(), 1).unwrap();
        let offset = first.weekday().num_days_from_monday() as usize;
        let days = (28..=31).rev().find(|d| NaiveDate::from_ymd_opt(now.year(), now.month(), *d).is_some()).unwrap_or(30);
        for d in 1..=days {
            let idx = offset + d as usize - 1;
            let (col, row) = (idx % 7, idx / 7);
            let cx = PAD + col as f32 * cell + cell / 2.0;
            let cy = y0 + 70.0 + row as f32 * 26.0;
            let today = d == now.day();
            if today {
                c.fill_circle(cx, cy, 11.0, theme.red);
            }
            let s = d.to_string();
            let w = c.measure(&s, 12.0, Weight::Medium);
            let col_c = if today { Rgba::rgb(255, 255, 255) } else if col >= 5 { theme.secondary } else { theme.label };
            c.text_vcenter(&s, cx - w / 2.0, cy - 10.0, 20.0, 12.0, Weight::Medium, col_c);
        }

        // sistema
        let y1 = y0 + 230.0 + 16.0;
        card(&mut c, y1, 90.0);
        let mut x = PAD;
        if let Some((pct, charging)) = st.battery {
            let ring = |c: &mut Canvas, x: f32, v: f32, label: &str, color: Rgba| {
                c.fill_circle(x + 24.0, y1 + 36.0, 22.0, theme.fg(0.10));
                c.fill_rrect(x + 4.0, y1 + 60.0, 40.0 * v, 4.0, 2.0, color);
                c.text_vcenter(label, x, y1 + 26.0, 20.0, 12.0, Weight::Semibold, theme.label);
            };
            ring(&mut c, x, pct as f32 / 100.0, &format!(" {pct}%"), if charging { theme.green } else { theme.label });
            c.text(if charging { "Carregando" } else { "Bateria" }, x, y1 + 70.0, 11.0, Weight::Regular, theme.secondary);
            x += 80.0;
        }
        let vol = st.volume.map(|v| format!("{:.0}%", v * 100.0)).unwrap_or_else(|| "—".into());
        c.symbol(crate::shell::menubar::volume_symbol(st), x + 12.0, y1 + 36.0, 22.0, theme.label);
        c.text(&vol, x, y1 + 62.0, 12.0, Weight::Semibold, theme.label);
        x += 80.0;
        let net = st.ssid.clone().unwrap_or_else(|| if st.wired { "Cabo".into() } else { "Offline".into() });
        c.symbol(crate::shell::menubar::network_symbol(st), x + 12.0, y1 + 36.0, 22.0, theme.label);
        let net = c.ellipsized(&net, 12.0, Weight::Semibold, W - x - PAD);
        c.text(&net, x, y1 + 62.0, 12.0, Weight::Semibold, theme.label);
        // Sonoma: com um app em foco, os widgets ficam monocromáticos e mais discretos
        if faded {
            for px in c.pm.pixels_mut() {
                let (r, g, b, a) = (px.red() as f32, px.green() as f32, px.blue() as f32, px.alpha() as f32);
                let l = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                let k = 0.75;
                let mix = |v: f32| ((v * 0.1 + l * 0.9) * k).min(a * k);
                if let Some(p) = resvg::tiny_skia::PremultipliedColorU8::from_rgba(mix(r) as u8, mix(g) as u8, mix(b) as u8, (a * k) as u8) {
                    *px = p;
                }
            }
        }
        self.layer.set(&c);
    }
}
