//! Assistente de primeiro uso (como o "Olá" do macOS): boas-vindas, aparência, rede, jogos e pronto.
//! Aparece na primeira sessão (sem ~/.config/obelisk/welcome-done) por cima de tudo.

use resvg::tiny_skia::Pixmap;

use super::layer::Layer;
use crate::ui::canvas::Canvas;
use crate::ui::text::Weight;
use crate::ui::text_glyph;
use crate::ui::theme::{Rgba, Theme};

#[derive(Clone, Debug, PartialEq)]
pub enum WelcomeHit {
    Next,
    Back,
    Skip,
    Appearance(&'static str),
    Wifi,
    GameAuto,
    Steam,
}

pub const STEPS: usize = 5;
const W: f32 = 640.0;
const H: f32 = 460.0;

#[derive(Default)]
pub struct Welcome {
    pub open: bool,
    pub step: usize,
    pub layer: Layer,
    hits: Vec<(WelcomeHit, (f32, f32, f32, f32))>,
    key: Option<String>,
}

/// Estado do sistema mostrado nos passos.
pub struct WelcomeInfo<'a> {
    pub user: &'a str,
    pub appearance: &'a str,
    pub network: Option<String>,
    pub game_auto: bool,
    pub steam: bool,
}

pub fn done_file() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config/obelisk/welcome-done")
}

impl Welcome {
    pub fn hit(&self, x: f64, y: f64) -> Option<WelcomeHit> {
        let (x, y) = (x as f32, y as f32);
        self.hits.iter().rev().find(|(_, r)| x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3).map(|(h, _)| h.clone())
    }

    pub fn finish(&mut self) {
        self.open = false;
        let p = done_file();
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        let _ = std::fs::write(p, "1\n");
    }

    pub fn update(&mut self, theme: &Theme, info: &WelcomeInfo, sw: i32, sh: i32, scale: i32, backdrop: Option<&Pixmap>) {
        let key = format!("{}|{}|{}|{:?}|{}|{}|{sw}|{sh}|{scale}|{}", self.step, info.appearance, info.user, info.network, info.game_auto, info.steam, theme.accent.0);
        if self.key.as_deref() == Some(key.as_str()) && self.layer.is_set() {
            return;
        }
        self.key = Some(key);
        self.hits.clear();
        let (w, h) = (sw as f32, sh as f32);
        let mut c = Canvas::new(sw, sh, scale as f32).unwrap();
        // fundo: papel de parede desfocado e escurecido (tela inteira, como no macOS)
        c.fill_rrect(0.0, 0.0, w, h, 0.0, Rgba(11, 15, 31, 255));
        if let Some(bd) = backdrop {
            c.fill_rrect_backdrop(0.0, 0.0, w, h, 0.0, bd, (0.0, 0.0), (w, h));
        }
        c.fill_rrect(0.0, 0.0, w, h, 0.0, Rgba(0, 0, 0, 90));
        let (ox, oy) = (((w - W) / 2.0).round(), ((h - H) / 2.0).round());
        c.shadow_rrect_add(ox, oy + 16.0, W, H, 18.0, 40.0, 140);
        c.fill_rrect(ox, oy, W, H, 18.0, theme.window_bg);
        c.stroke_rrect(ox, oy, W, H, 18.0, 1.0, theme.fg(0.10));
        let cx = ox + W / 2.0;
        let center = |c: &mut Canvas, t: &str, y: f32, size: f32, wt: Weight, col: Rgba| {
            let tw = c.measure(t, size, wt);
            c.text(t, cx - tw / 2.0, y, size, wt, col);
        };

        match self.step {
            0 => {
                c.fill_rrect_vgradient(cx - 56.0, oy + 54.0, 112.0, 112.0, 26.0, Rgba::rgb(0x4a, 0x6d, 0xf0), Rgba::rgb(0x17, 0x1d, 0x5c));
                text_glyph::obelisk_logo(&mut c, cx, oy + 110.0, 70.0, Rgba::rgb(255, 255, 255));
                center(&mut c, "Olá", oy + 190.0, 44.0, Weight::Bold, theme.label);
                center(&mut c, &format!("Boas-vindas ao Obelisk, {}.", info.user), oy + 250.0, 15.0, Weight::Regular, theme.secondary);
                center(&mut c, "Vamos deixar tudo do seu jeito em poucos passos.", oy + 272.0, 15.0, Weight::Regular, theme.secondary);
            }
            1 => {
                center(&mut c, "Aparência", oy + 40.0, 26.0, Weight::Bold, theme.label);
                center(&mut c, "Escolha como o Obelisk deve ficar. Dá para mudar depois nos Ajustes.", oy + 80.0, 13.0, Weight::Regular, theme.secondary);
                let opts = [("Claro", "light"), ("Escuro", "dark"), ("Automático", "auto")];
                let (pw, ph, gap) = (160.0, 104.0, 24.0);
                let x0 = cx - (3.0 * pw + 2.0 * gap) / 2.0;
                for (i, (label, k)) in opts.iter().enumerate() {
                    let px = x0 + i as f32 * (pw + gap);
                    let py = oy + 130.0;
                    preview(&mut c, px, py, pw, ph, k);
                    if info.appearance == *k {
                        c.stroke_rrect(px - 4.0, py - 4.0, pw + 8.0, ph + 8.0, 14.0, 3.0, theme.accent);
                    }
                    let lw = c.measure(label, 13.0, Weight::Medium);
                    c.text(label, px + (pw - lw) / 2.0, py + ph + 14.0, 13.0, Weight::Medium, theme.label);
                    self.hits.push((WelcomeHit::Appearance(k), (px - 4.0, py - 4.0, pw + 8.0, ph + 40.0)));
                }
            }
            2 => {
                c.fill_circle(cx, oy + 104.0, 44.0, theme.accent.with_alpha(40));
                c.symbol("wifi", cx, oy + 104.0, 48.0, theme.accent);
                center(&mut c, "Rede", oy + 168.0, 26.0, Weight::Bold, theme.label);
                match &info.network {
                    Some(n) => {
                        center(&mut c, &format!("Conectado a {n}."), oy + 210.0, 15.0, Weight::Regular, theme.secondary);
                        center(&mut c, "Tudo certo para atualizações, Steam e a loja de apps.", oy + 232.0, 13.0, Weight::Regular, theme.tertiary);
                    }
                    None => {
                        center(&mut c, "Você ainda não está conectado à internet.", oy + 210.0, 15.0, Weight::Regular, theme.secondary);
                        let label = "Escolher rede Wi‑Fi…";
                        let bw = c.measure(label, 13.0, Weight::Medium) + 32.0;
                        let (bx, by) = (cx - bw / 2.0, oy + 250.0);
                        c.fill_rrect(bx, by, bw, 30.0, 8.0, theme.fg(0.10));
                        c.text_vcenter(label, bx + 16.0, by, 30.0, 13.0, Weight::Medium, theme.label);
                        self.hits.push((WelcomeHit::Wifi, (bx, by, bw, 30.0)));
                    }
                }
            }
            3 => {
                c.fill_circle(cx, oy + 100.0, 44.0, Rgba(0x30, 0xd1, 0x58, 45));
                c.symbol("sports_esports", cx, oy + 100.0, 48.0, Rgba::rgb(0x30, 0xd1, 0x58));
                center(&mut c, "Jogos", oy + 160.0, 26.0, Weight::Bold, theme.label);
                center(&mut c, "O Modo Jogo libera CPU e GPU, liga o VRR e silencia avisos.", oy + 200.0, 13.0, Weight::Regular, theme.secondary);
                // chave "ligar sozinho"
                let (rx, ry, rw) = (ox + 110.0, oy + 240.0, W - 220.0);
                c.fill_rrect(rx, ry, rw, 52.0, 10.0, theme.fg(0.05));
                c.text_vcenter("Ligar o Modo Jogo sozinho com jogos", rx + 16.0, ry, 52.0, 13.0, Weight::Regular, theme.label);
                let (tx, ty) = (rx + rw - 16.0 - 40.0, ry + 14.0);
                c.fill_rrect(tx, ty, 40.0, 24.0, 12.0, if info.game_auto { theme.accent } else { theme.fg(0.16) });
                c.fill_circle(if info.game_auto { tx + 28.0 } else { tx + 12.0 }, ty + 12.0, 10.0, Rgba::rgb(255, 255, 255));
                self.hits.push((WelcomeHit::GameAuto, (rx, ry, rw, 52.0)));
                let label = if info.steam { "Abrir a Steam" } else { "A Steam não está instalada" };
                let bw = c.measure(label, 13.0, Weight::Medium) + 32.0;
                let (bx, by) = (cx - bw / 2.0, oy + 312.0);
                c.fill_rrect(bx, by, bw, 30.0, 8.0, theme.fg(if info.steam { 0.10 } else { 0.05 }));
                c.text_vcenter(label, bx + 16.0, by, 30.0, 13.0, Weight::Medium, if info.steam { theme.label } else { theme.tertiary });
                if info.steam {
                    self.hits.push((WelcomeHit::Steam, (bx, by, bw, 30.0)));
                }
            }
            _ => {
                c.fill_circle(cx, oy + 110.0, 48.0, theme.accent);
                c.symbol("check", cx, oy + 110.0, 56.0, Rgba::rgb(255, 255, 255));
                center(&mut c, "Tudo pronto", oy + 184.0, 30.0, Weight::Bold, theme.label);
                center(&mut c, "⌘ Espaço busca apps e ⌘ Tab troca de app.", oy + 232.0, 13.0, Weight::Regular, theme.secondary);
                center(&mut c, "Passe o mouse no notch para ver música, agenda e o Modo Jogo.", oy + 254.0, 13.0, Weight::Regular, theme.secondary);
                center(&mut c, "Os Ajustes do Sistema ficam no menu do símbolo do Obelisk e no Dock.", oy + 276.0, 13.0, Weight::Regular, theme.secondary);
            }
        }

        // pontinhos do progresso
        let dots_w = STEPS as f32 * 14.0;
        for i in 0..STEPS {
            let col = if i == self.step { theme.label } else { theme.fg(0.2) };
            c.fill_circle(cx - dots_w / 2.0 + 7.0 + i as f32 * 14.0, oy + H - 74.0, 3.5, col);
        }
        // botões: Voltar · Continuar (e "Agora não" no primeiro passo)
        let label = if self.step + 1 == STEPS { "Começar" } else { "Continuar" };
        let bw = c.measure(label, 14.0, Weight::Semibold) + 48.0;
        let (bx, by) = (ox + W - 28.0 - bw, oy + H - 56.0);
        c.fill_rrect(bx, by, bw, 34.0, 9.0, theme.accent);
        c.text_vcenter(label, bx + 24.0, by, 34.0, 14.0, Weight::Semibold, theme.text_on_accent);
        self.hits.push((WelcomeHit::Next, (bx, by, bw, 34.0)));
        let (back, hit) = if self.step == 0 { ("Agora não", WelcomeHit::Skip) } else { ("Voltar", WelcomeHit::Back) };
        let lw = c.measure(back, 14.0, Weight::Medium);
        c.text_vcenter(back, ox + 30.0, by, 34.0, 14.0, Weight::Medium, theme.accent);
        self.hits.push((hit, (ox + 20.0, by, lw + 20.0, 34.0)));
        self.layer.set(&c);
    }
}

fn preview(c: &mut Canvas, x: f32, y: f32, w: f32, h: f32, mode: &str) {
    let paint = |c: &mut Canvas, x: f32, w: f32, dark: bool, rl: f32, rr: f32| {
        let (top, bottom) = if dark { (Rgba::rgb(0x1b, 0x22, 0x5a), Rgba::rgb(0x4b, 0x1f, 0x6e)) } else { (Rgba::rgb(0x9f, 0xc4, 0xff), Rgba::rgb(0xe9, 0xd1, 0xff)) };
        c.fill_rrect_corners(x, y, w, h, [rl, rr, rr, rl], bottom);
        c.fill_rrect_corners(x, y, w, h * 0.6, [rl, rr, 0.0, 0.0], top);
        let win = if dark { Rgba::rgb(0x2a, 0x2a, 0x2d) } else { Rgba::rgb(255, 255, 255) };
        c.fill_rrect(x + 12.0, y + 20.0, w * 0.72, h - 34.0, 6.0, win);
    };
    match mode {
        "light" => paint(c, x, w, false, 12.0, 12.0),
        "dark" => paint(c, x, w, true, 12.0, 12.0),
        _ => {
            paint(c, x, w / 2.0, false, 12.0, 0.0);
            paint(c, x + w / 2.0, w / 2.0, true, 0.0, 12.0);
        }
    }
}
