//! Menu suspenso no estilo NSMenu (docs/03-especificacao-kyro.md §4):
//! raio 12, margem 5, linhas de 24 com raio 7, destaque azul, coluna de marca e atalhos.

use std::time::Instant;

use super::layer::Layer;
use crate::ui::canvas::Canvas;
use crate::ui::text::{self, Weight};
use crate::ui::theme::{metrics, Rgba, Theme};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuAction {
    About,
    Settings,
    ControlCenter,
    ToggleAppearance,
    Sleep,
    Restart,
    Shutdown,
    Logout,
    HideApp,
    QuitApp,
    Minimize,
    Zoom,
    Fullscreen,
    BringAllToFront,
    FocusWindow(usize),
    Workspace(usize),
    OpenPath(String),
    Launchpad,
    MissionControl,
    // menu de contexto do Dock
    DockOpen(usize),
    DockTogglePin(usize),
    DockQuit(usize),
    ForceQuitMenu,
    Lock,
    LowPower,
    OnTop,
    StageManager,
    ConsoleMode,
    TrayMenu(usize, i32),
    ActivityMonitor,
    AutoAppearance,
    NightShift,
    Expose(usize),
    ForceQuit(usize),
    /// atalho enviado ao app em foco: (modificadores, tecla) em keycodes xkb
    Keys(Vec<u32>, u32),
    /// Ctrl(+Shift nos terminais) + tecla: copiar/colar respeitando terminais
    Clip(u32),
    SettingsPage(String),
    Spotlight,
    Emoji,
    NewFolder,
    ToggleWidgets,
    DeskOpen(usize),
    DeskShow(usize),
    DeskTrash(usize),
    DeskRename(usize),
    AboutApp,
    Installer,
    Store,
}

#[derive(Clone, Debug)]
pub struct MenuItem {
    pub label: String,
    pub action: Option<MenuAction>, // None = separador
    pub enabled: bool,
    pub shortcut: Option<String>,
    pub checked: Option<bool>,
}

impl MenuItem {
    pub fn new(label: &str, action: MenuAction) -> Self {
        MenuItem { label: label.into(), action: Some(action), enabled: true, shortcut: None, checked: None }
    }
    pub fn sep() -> Self {
        MenuItem { label: String::new(), action: None, enabled: false, shortcut: None, checked: None }
    }
    pub fn key(mut self, s: &str) -> Self {
        self.shortcut = Some(s.into());
        self
    }
    pub fn check(mut self, on: bool) -> Self {
        self.checked = Some(on);
        self
    }
    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }
}

const ROW_H: f32 = 24.0;
const SEP_H: f32 = 11.0;
const PAD: f32 = 5.0;
const SHADOW: i32 = 28;

pub struct Menu {
    pub layer: Layer,
    pub items: Vec<MenuItem>,
    /// Canto superior esquerdo do painel (px lógicos).
    pub pos: (i32, i32),
    pub hover: Option<usize>,
    pub confirming: Option<usize>,
    /// De qual item da barra veio (para trocar de menu ao passar o mouse).
    pub origin: Option<super::menubar::BarItem>,
    pub opened: Instant,
    width: f32,
    key: Option<(Option<usize>, Option<usize>, u8)>,
}

impl Menu {
    pub fn new(items: Vec<MenuItem>, pos: (i32, i32), min_width: f32) -> Self {
        let checks = items.iter().any(|i| i.checked.is_some());
        let content = items
            .iter()
            .map(|i| {
                let lw = text::measure(&i.label, 13.0, Weight::Regular);
                let sw = i.shortcut.as_ref().map(|s| 24.0 + text::measure(s, 13.0, Weight::Regular)).unwrap_or(0.0);
                10.0 + if checks { 20.0 } else { 0.0 } + lw + sw + 10.0
            })
            .fold(0.0f32, f32::max);
        let width = (content + 2.0 * PAD).max(min_width).ceil();
        Menu { layer: Layer::default(), items, pos, hover: None, confirming: None, origin: None, opened: Instant::now(), width, key: None }
    }

    pub fn size(&self) -> (i32, i32) {
        let h: f32 = self.items.iter().map(|i| if i.action.is_none() { SEP_H } else { ROW_H }).sum::<f32>() + 2.0 * PAD;
        (self.width as i32, h.ceil() as i32)
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        let (w, h) = self.size();
        x >= self.pos.0 as f64 && x < (self.pos.0 + w) as f64 && y >= self.pos.1 as f64 && y < (self.pos.1 + h) as f64
    }

    pub fn item_at(&self, x: f64, y: f64) -> Option<usize> {
        if !self.contains(x, y) {
            return None;
        }
        let mut cy = self.pos.1 as f64 + PAD as f64;
        for (i, it) in self.items.iter().enumerate() {
            let h = if it.action.is_none() { SEP_H } else { ROW_H } as f64;
            if y >= cy && y < cy + h {
                return (it.action.is_some() && it.enabled).then_some(i);
            }
            cy += h;
        }
        None
    }

    pub fn buf_pos(&self) -> (i32, i32) {
        (self.pos.0 - SHADOW, self.pos.1 - SHADOW)
    }

    /// Opacidade da animação de abertura (90 ms).
    pub fn alpha(&self) -> f32 {
        (self.opened.elapsed().as_secs_f32() / 0.09).min(1.0)
    }

    pub fn update(&mut self, theme: &Theme, scale: i32, backdrop: Option<&resvg::tiny_skia::Pixmap>, screen: (f32, f32)) {
        let key = (self.hover, self.confirming, theme.appearance as u8);
        if self.key == Some(key) && self.layer.is_set() {
            return;
        }
        self.key = Some(key);
        let (w, h) = self.size();
        let (wf, hf) = (w as f32, h as f32);
        let s = SHADOW as f32;
        let mut c = Canvas::new(w + 2 * SHADOW, h + 2 * SHADOW, scale as f32).unwrap();
        c.shadow_rrect(s, s + 8.0, wf, hf, metrics::MENU_RADIUS, 22.0, 115);
        if let Some(bd) = backdrop {
            let o = self.buf_pos();
            c.fill_rrect_backdrop(s, s, wf, hf, metrics::MENU_RADIUS, bd, (o.0 as f32, o.1 as f32), screen);
        }
        c.fill_rrect(s, s, wf, hf, metrics::MENU_RADIUS, theme.menu_bg);
        c.stroke_rrect(s, s, wf, hf, metrics::MENU_RADIUS, 1.0, theme.menu_border);

        let checks = self.items.iter().any(|i| i.checked.is_some());
        let mut y = s + PAD;
        for (i, it) in self.items.iter().enumerate() {
            if it.action.is_none() {
                c.fill_rrect(s + 10.0, y + SEP_H / 2.0 - 0.5, wf - 20.0, 1.0, 0.0, theme.separator);
                y += SEP_H;
                continue;
            }
            let hovered = self.hover == Some(i) && it.enabled;
            if hovered {
                c.fill_rrect(s + PAD, y, wf - 2.0 * PAD, ROW_H, 7.0, theme.accent);
            }
            let (tc, kc) = if hovered {
                (theme.text_on_accent, theme.text_on_accent)
            } else if it.enabled {
                (theme.label, theme.tertiary)
            } else {
                (theme.tertiary, theme.tertiary)
            };
            let mut tx = s + PAD + 10.0;
            if checks {
                if it.checked == Some(true) {
                    c.symbol("check", tx + 7.0, y + ROW_H / 2.0, 14.0, tc);
                }
                tx += 20.0;
            }
            let label = if self.confirming == Some(i) { "Clique de novo para confirmar".to_string() } else { it.label.clone() };
            let lc = if self.confirming == Some(i) && !hovered { theme.red } else { tc };
            c.text_vcenter(&label, tx, y, ROW_H, 13.0, Weight::Regular, lc);
            if let Some(k) = &it.shortcut {
                let kw = c.measure(k, 13.0, Weight::Regular);
                c.text_vcenter(k, s + wf - PAD - 10.0 - kw, y, ROW_H, 13.0, Weight::Regular, kc);
            }
            y += ROW_H;
        }
        self.layer.set(&c);
    }
}

#[allow(dead_code)]
fn _unused(_: Rgba) {}
