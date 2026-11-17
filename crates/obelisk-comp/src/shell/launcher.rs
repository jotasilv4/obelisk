//! Busca (docs/03-especificacao-kyro.md §6): painel de 680 a 22 % da altura, raio 24,
//! campo de 56 px com texto leve de 22 px, linhas de 40 px com seções:
//! Calculadora · Aplicativos · Ações · Arquivos · Web.

use std::path::PathBuf;

use resvg::tiny_skia::Pixmap;

use super::layer::Layer;
use crate::apps::{self, App};
use crate::calc;
use crate::ui::canvas::Canvas;
use crate::ui::icons::IconStore;
use crate::ui::text::Weight;
use crate::ui::theme::{metrics, Rgba, Theme};

#[derive(Clone, Debug, PartialEq)]
pub enum ShellAction {
    Launchpad,
    MissionControl,
    ControlCenter,
    ToggleAppearance,
    Screenshot,
    Settings,
    Logout,
    Lock,
    Activity,
    Console,
    Stage,
}

#[derive(Clone, Debug)]
pub enum Action {
    Run(String),
    Copy(String),
    Open(PathBuf),
    Shell(ShellAction),
    Nothing,
}

#[derive(Clone, Debug)]
enum RowIcon {
    App(String),
    Symbol(&'static str, Rgba),
}

#[derive(Clone, Debug)]
struct Row {
    section: &'static str,
    title: String,
    subtitle: String,
    icon: RowIcon,
    action: Action,
}

const ACTIONS: [(&str, &str, &str, ShellAction); 11] = [
    ("Stage Manager", "App no palco, outros ao lado", "view_carousel", ShellAction::Stage),
    ("Modo Console", "Steam Big Picture no gamescope", "sports_esports", ShellAction::Console),
    ("Monitor de Atividade", "Processos, CPU e memória", "memory", ShellAction::Activity),
    ("Bloquear Tela", "Pede a senha", "lock", ShellAction::Lock),
    ("Launchpad", "Mostrar todos os apps", "apps", ShellAction::Launchpad),
    ("Mission Control", "Ver janelas e mesas", "space_dashboard", ShellAction::MissionControl),
    ("Central de Controle", "Wi‑Fi, som, brilho", "tune", ShellAction::ControlCenter),
    ("Alternar Aparência", "Claro ou escuro", "dark_mode", ShellAction::ToggleAppearance),
    ("Captura de Tela", "Tela inteira", "screenshot_monitor", ShellAction::Screenshot),
    ("Ajustes do Sistema", "Preferências", "settings", ShellAction::Settings),
    ("Encerrar Sessão", "Sair do Obelisk", "logout", ShellAction::Logout),
];

const SHADOW: i32 = 44;
const W: f32 = metrics::LAUNCHER_W as f32;
const FIELD_H: f32 = 56.0;
const ROW_H: f32 = 40.0;
const HEAD_H: f32 = 26.0;

#[derive(Default)]
pub struct Launcher {
    pub open: bool,
    pub query: String,
    pub selected: usize,
    rows: Vec<Row>,
    pub layer: Layer,
    key: Option<String>,
    pub pos: (i32, i32),
    pub size: (i32, i32),
    /// topo (tela) de cada linha, para hit-test
    row_tops: Vec<f32>,
}

impl Launcher {
    pub fn toggle(&mut self, apps: &[App]) {
        if self.open {
            self.close();
        } else {
            self.open = true;
            self.query.clear();
            self.selected = 0;
            self.recompute(apps);
        }
    }

    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
        self.rows.clear();
        self.key = None;
    }

    pub fn push_str(&mut self, s: &str, apps: &[App]) {
        self.query.push_str(s);
        self.selected = 0;
        self.recompute(apps);
    }

    pub fn backspace(&mut self, apps: &[App]) {
        self.query.pop();
        self.selected = 0;
        self.recompute(apps);
    }

    /// ↑/↓ sem dar a volta (como no kyro).
    pub fn move_selection(&mut self, delta: i32) {
        if self.rows.is_empty() {
            return;
        }
        self.selected = (self.selected as i32 + delta).clamp(0, self.rows.len() as i32 - 1) as usize;
    }

    pub fn select_row(&mut self, i: usize) {
        if i < self.rows.len() {
            self.selected = i;
        }
    }

    pub fn activate(&mut self, idx: Option<usize>) -> Action {
        let i = idx.unwrap_or(self.selected);
        self.rows.get(i).map(|r| r.action.clone()).unwrap_or(Action::Nothing)
    }

    fn recompute(&mut self, apps: &[App]) {
        self.rows.clear();
        let q = self.query.trim();
        if q.is_empty() {
            return;
        }
        if let Some(cmd) = q.strip_prefix('>') {
            let cmd = cmd.trim();
            if !cmd.is_empty() {
                self.rows.push(Row {
                    section: "Comando",
                    title: format!("Executar: {cmd}"),
                    subtitle: "Terminal".into(),
                    icon: RowIcon::Symbol("terminal", Rgba::rgb(0x3a, 0x3a, 0x3c)),
                    action: Action::Run(cmd.to_string()),
                });
            }
            return;
        }
        if let Some(v) = calc::eval(q) {
            let s = calc::format(v);
            self.rows.push(Row {
                section: "Calculadora",
                title: format!("= {s}"),
                subtitle: "Enter copia".into(),
                icon: RowIcon::Symbol("calculate", Rgba::rgb(0xff, 0x92, 0x30)),
                action: Action::Copy(s.replace(',', ".")),
            });
        }
        let mut scored: Vec<(i32, &App)> = apps.iter().filter_map(|a| apps::score(a, q).map(|s| (s, a))).collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(&b.1.name)));
        for (_, a) in scored.into_iter().take(6) {
            self.rows.push(Row {
                section: "Aplicativos",
                title: a.name.clone(),
                subtitle: if a.generic.is_empty() { "Aplicativo".into() } else { a.generic.clone() },
                icon: RowIcon::App(a.icon.clone()),
                action: Action::Run(a.exec.clone()),
            });
        }
        let ql = fold(q);
        for (title, sub, sym, act) in ACTIONS.iter().filter(|(t, s, _, _)| fold(t).contains(&ql) || fold(s).contains(&ql)).take(3) {
            self.rows.push(Row {
                section: "Ações",
                title: title.to_string(),
                subtitle: sub.to_string(),
                icon: RowIcon::Symbol(sym, Rgba::rgb(0x6d, 0x7c, 0xff)),
                action: Action::Shell(act.clone()),
            });
        }
        if q.chars().count() >= 2 {
            for p in search_files(&ql, 8) {
                let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                let parent = p.parent().map(|d| d.to_string_lossy().replace(&std::env::var("HOME").unwrap_or_default(), "~")).unwrap_or_default();
                self.rows.push(Row {
                    section: "Arquivos",
                    title: name,
                    subtitle: parent,
                    icon: RowIcon::Symbol(if p.is_dir() { "folder" } else { "description" }, Rgba::rgb(0x00, 0x91, 0xff)),
                    action: Action::Open(p),
                });
            }
        }
        self.rows.push(Row {
            section: "Web",
            title: format!("Pesquisar “{q}” na web"),
            subtitle: "DuckDuckGo".into(),
            icon: RowIcon::Symbol("travel_explore", Rgba::rgb(0x00, 0xd2, 0xe0)),
            action: Action::Open(PathBuf::from(format!("https://duckduckgo.com/?q={}", url_encode(q)))),
        });
    }

    pub fn panel_size(&self) -> (f32, f32) {
        if self.rows.is_empty() {
            return (W, FIELD_H);
        }
        let mut h = 6.0;
        let mut last = "";
        for r in &self.rows {
            if r.section != last {
                h += HEAD_H;
                last = r.section;
            }
            h += ROW_H;
        }
        (W, FIELD_H + h.min(400.0) + 12.0)
    }

    pub fn row_at(&self, x: f64, y: f64) -> Option<usize> {
        if !self.contains(x, y) {
            return None;
        }
        self.row_tops.iter().position(|t| (y as f32) >= *t && (y as f32) < t + ROW_H)
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        let (w, h) = self.panel_size();
        x >= self.pos.0 as f64 && x < self.pos.0 as f64 + w as f64 && y >= self.pos.1 as f64 && y < self.pos.1 as f64 + h as f64
    }

    pub fn buf_pos(&self) -> (i32, i32) {
        (self.pos.0 - SHADOW, self.pos.1 - SHADOW)
    }

    pub fn update(&mut self, theme: &Theme, icons: &mut IconStore, sw: i32, sh: i32, scale: i32, backdrop: Option<&Pixmap>) {
        if !self.open {
            return;
        }
        let (w, h) = self.panel_size();
        let w = w.min(sw as f32 - 40.0);
        self.size = (w as i32, h as i32);
        self.pos = (((sw as f32 - w) / 2.0) as i32, (sh as f32 * 0.22) as i32);
        let key = format!("{}|{}|{}|{}|{}", self.query, self.selected, theme.appearance as u8, sw, scale);
        if self.key.as_deref() == Some(key.as_str()) && self.layer.is_set() {
            return;
        }
        self.key = Some(key);

        let s = SHADOW as f32;
        let mut c = Canvas::new((w + 2.0 * s) as i32, (h + 2.0 * s) as i32, scale as f32).unwrap();
        let (x, y) = (s, s);
        c.shadow_rrect(x, y + 12.0, w, h, 24.0, 40.0, 128);
        if let Some(bd) = backdrop {
            let o = self.buf_pos();
            c.fill_rrect_backdrop(x, y, w, h, 24.0, bd, (o.0 as f32, o.1 as f32), (sw as f32, sh as f32));
        }
        c.fill_rrect(x, y, w, h, 24.0, theme.menu_bg);
        c.stroke_rrect(x, y, w, h, 24.0, 1.0, theme.menu_border);

        c.symbol("search", x + 18.0 + 12.0, y + FIELD_H / 2.0, 24.0, theme.secondary);
        let tx = x + 18.0 + 24.0 + 12.0;
        if self.query.is_empty() {
            c.text_vcenter("Busca", tx, y, FIELD_H, 22.0, Weight::Light, theme.tertiary);
        } else {
            let t = c.ellipsized(&self.query, 22.0, Weight::Light, w - (tx - x) - 20.0);
            let tw = c.text_vcenter(&t, tx, y, FIELD_H, 22.0, Weight::Light, theme.label);
            c.fill_rrect(tx + tw + 1.0, y + 16.0, 2.0, FIELD_H - 32.0, 1.0, theme.accent);
        }

        self.row_tops.clear();
        if !self.rows.is_empty() {
            c.fill_rrect(x, y + FIELD_H, w, 1.0, 0.0, theme.separator);
            let mut ry = y + FIELD_H + 6.0;
            let mut last = "";
            let bottom = y + h - 6.0;
            for (i, row) in self.rows.iter().enumerate() {
                if row.section != last {
                    last = row.section;
                    if ry + HEAD_H > bottom {
                        break;
                    }
                    c.text_vcenter(row.section, x + 12.0, ry, HEAD_H, 11.0, Weight::Bold, theme.tertiary);
                    ry += HEAD_H;
                }
                if ry + ROW_H > bottom + 1.0 {
                    break;
                }
                self.row_tops.push(ry - s + self.pos.1 as f32);
                let sel = i == self.selected;
                if sel {
                    c.fill_rrect(x + 6.0, ry, w - 12.0, ROW_H, 9.0, theme.accent);
                }
                let (tc, sc) = if sel { (theme.text_on_accent, theme.text_on_accent.with_alpha(230)) } else { (theme.label, theme.tertiary) };
                let ix = x + 6.0 + 10.0;
                let iy = ry + (ROW_H - 28.0) / 2.0;
                match &row.icon {
                    RowIcon::App(name) => {
                        let pm = icons.get_theme(name, &row.title).clone();
                        c.blit(&pm, ix, iy, 28.0, 28.0, 1.0);
                    }
                    RowIcon::Symbol(sym, color) => {
                        c.fill_rrect(ix, iy, 28.0, 28.0, 7.0, *color);
                        c.symbol(sym, ix + 14.0, iy + 14.0, 18.0, Rgba::rgb(255, 255, 255));
                    }
                }
                let sub_w = c.measure(&row.subtitle, 12.0, Weight::Regular).min(260.0);
                let title_max = w - 12.0 - 10.0 - 28.0 - 10.0 - 12.0 - sub_w - 16.0;
                let t = c.ellipsized(&row.title, 14.0, Weight::Medium, title_max);
                c.text_vcenter(&t, ix + 28.0 + 10.0, ry, ROW_H, 14.0, Weight::Medium, tc);
                let st = c.ellipsized(&row.subtitle, 12.0, Weight::Regular, 260.0);
                let stw = c.measure(&st, 12.0, Weight::Regular);
                c.text_vcenter(&st, x + w - 6.0 - 12.0 - stw, ry, ROW_H, 12.0, Weight::Regular, sc);
                ry += ROW_H;
            }
        }
        self.layer.set(&c);
    }
}

/// minúsculas sem acento, para buscar "configuracoes" e achar "Configurações"
fn fold(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'ê' | 'è' => 'e',
            'í' | 'ì' => 'i',
            'ó' | 'ô' | 'õ' | 'ò' => 'o',
            'ú' | 'ü' | 'ù' => 'u',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

fn url_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            b' ' => "+".into(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Busca por nome nas pastas do usuário (limites do kyro: 6 níveis, 12 mil entradas, 1,2 s).
fn search_files(q: &str, max: usize) -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else { return vec![] };
    let dirs = ["Documentos", "Documents", "Downloads", "Área de trabalho", "Desktop", "Imagens", "Pictures", "Vídeos", "Videos", "Músicas", "Music"];
    let words: Vec<&str> = q.split_whitespace().collect();
    let start = std::time::Instant::now();
    let mut out = Vec::new();
    let mut seen = 0usize;
    let mut stack: Vec<(PathBuf, u32)> = dirs.iter().map(|d| (home.join(d), 0)).filter(|(p, _)| p.is_dir()).collect();
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            seen += 1;
            if seen > 12_000 || start.elapsed().as_millis() > 1200 || out.len() >= max {
                return out;
            }
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let ft = match e.file_type() {
                Ok(t) => t,
                Err(_) => continue,
            };
            let n = fold(&name);
            if words.iter().all(|w| n.contains(w)) {
                out.push(e.path());
            }
            if ft.is_dir() && !ft.is_symlink() && depth < 6 {
                stack.push((e.path(), depth + 1));
            }
        }
    }
    out
}
