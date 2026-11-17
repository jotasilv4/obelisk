//! Ícones na mesa (como o Finder): os arquivos da pasta "Área de Trabalho" em colunas a partir do
//! canto superior direito. Clique seleciona, duplo clique abre, botão direito abre o menu.
//! Relê a pasta a cada 2 s (só um `read_dir`); redesenha só quando algo muda.

use std::path::PathBuf;
use std::time::Instant;

use resvg::tiny_skia::Pixmap;

use super::layer::Layer;
use crate::ui::canvas::Canvas;
use crate::ui::icons::IconStore;
use crate::ui::text::Weight;
use crate::ui::theme::{Rgba, Theme};

pub const CELL_W: f32 = 96.0;
pub const CELL_H: f32 = 100.0;
const ICON: f32 = 56.0;

#[derive(Clone)]
pub struct DeskItem {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    icon: Option<Pixmap>,
}

#[derive(Default)]
pub struct Desktop {
    pub items: Vec<DeskItem>,
    pub selected: Option<usize>,
    pub layer: Layer,
    /// posição do layer na tela e retângulos (tela) de cada ícone
    pub pos: (f32, f32),
    pub rects: Vec<(f32, f32, f32, f32)>,
    pub last_click: Option<(usize, Instant)>,
    /// ícone sendo arrastado (fica esmaecido no lugar)
    pub dragging: Option<usize>,
    /// renomeando: (índice, texto)
    pub renaming: Option<(usize, String)>,
    /// posições escolhidas pelo usuário (nome → canto do ícone), salvas em ~/.config/obelisk/desktop
    pub positions: std::collections::HashMap<String, (f32, f32)>,
    positions_loaded: bool,
    scanned: Option<Instant>,
    names: Vec<String>,
    key: Option<String>,
}

/// Pasta da mesa (xdg-user-dirs: "Área de Trabalho", "Desktop"…).
pub fn desktop_dir() -> PathBuf {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    if let Ok(cfg) = std::fs::read_to_string(home.join(".config/user-dirs.dirs")) {
        for l in cfg.lines() {
            if let Some(v) = l.strip_prefix("XDG_DESKTOP_DIR=") {
                let v = v.trim_matches('"').replace("$HOME", &home.to_string_lossy());
                return PathBuf::from(v);
            }
        }
    }
    ["Área de Trabalho", "Desktop"].iter().map(|d| home.join(d)).find(|p| p.is_dir()).unwrap_or(home.join("Desktop"))
}

fn icon_name(path: &std::path::Path, is_dir: bool) -> &'static str {
    if is_dir {
        return "folder";
    }
    match path.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref() {
        Some("pdf") => "application-pdf",
        Some("png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "svg") => "image-x-generic",
        Some("mp3" | "flac" | "ogg" | "wav" | "m4a" | "opus") => "audio-x-generic",
        Some("mp4" | "mkv" | "webm" | "mov" | "avi") => "video-x-generic",
        Some("zip" | "tar" | "gz" | "xz" | "7z" | "rar" | "zst") => "package-x-generic",
        Some("sh" | "appimage" | "exe") => "application-x-executable",
        Some("odt" | "doc" | "docx") => "x-office-document",
        Some("ods" | "xls" | "xlsx" | "csv") => "x-office-spreadsheet",
        _ => "text-x-generic",
    }
}

fn positions_file() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config/obelisk/desktop")
}

impl Desktop {
    fn load_positions(&mut self) {
        if self.positions_loaded {
            return;
        }
        self.positions_loaded = true;
        for l in std::fs::read_to_string(positions_file()).unwrap_or_default().lines() {
            if let Some((name, xy)) = l.rsplit_once('=') {
                if let Some((x, y)) = xy.split_once(',') {
                    if let (Ok(x), Ok(y)) = (x.parse(), y.parse()) {
                        self.positions.insert(name.to_string(), (x, y));
                    }
                }
            }
        }
    }

    fn save_positions(&self) {
        let p = positions_file();
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        let text: String = self.positions.iter().map(|(k, (x, y))| format!("{k}={x:.0},{y:.0}\n")).collect();
        let _ = std::fs::write(p, text);
    }

    /// Solta o ícone `i` com o ponteiro em (x, y): fica na célula da grade mais próxima.
    pub fn move_icon(&mut self, i: usize, x: f32, y: f32, top: f32) {
        let Some(it) = self.items.get(i) else { return };
        let gx = ((x - CELL_W / 2.0) / CELL_W).round() * CELL_W;
        let gy = (((y - top - CELL_H / 2.0) / CELL_H).round() * CELL_H + top).max(top);
        // não empilha: se a célula está ocupada, desce até achar uma livre
        let mut gy = gy;
        while self.rects.iter().enumerate().any(|(j, r)| j != i && (r.0 - 4.0 - gx).abs() < 1.0 && (r.1 - gy).abs() < 1.0) {
            gy += CELL_H;
        }
        self.positions.insert(it.name.clone(), (gx, gy));
        self.save_positions();
        self.key = None;
    }

    /// Termina de renomear: muda o nome do arquivo e mantém a posição.
    pub fn commit_rename(&mut self) {
        let Some((i, new)) = self.renaming.take() else { return };
        let new = new.trim().replace('/', "-");
        self.key = None;
        let Some(it) = self.items.get(i) else { return };
        if new.is_empty() || new == it.name {
            return;
        }
        let from = std::path::Path::new(&it.path);
        let to = from.with_file_name(&new);
        if to.exists() {
            return;
        }
        if std::fs::rename(from, &to).is_ok() {
            if let Some(p) = self.positions.remove(&it.name) {
                self.positions.insert(new, p);
                self.save_positions();
            }
            self.scanned = None;
        }
    }

    pub fn icon_of(&self, i: usize) -> Option<&Pixmap> {
        self.items.get(i).and_then(|it| it.icon.as_ref())
    }

    /// Relê a pasta (no máximo a cada 2 s). Devolve `true` se a lista mudou.
    pub fn scan(&mut self, icons: &mut IconStore) -> bool {
        if self.scanned.is_some_and(|t| t.elapsed().as_secs() < 2) {
            return false;
        }
        self.scanned = Some(Instant::now());
        let dir = desktop_dir();
        let mut list: Vec<(String, String, bool)> = std::fs::read_dir(&dir)
            .map(|rd| {
                rd.flatten()
                    .filter_map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        (!name.starts_with('.')).then(|| (e.path().to_string_lossy().to_string(), name, e.path().is_dir()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        list.sort_by(|a, b| b.2.cmp(&a.2).then(a.1.to_lowercase().cmp(&b.1.to_lowercase())));
        let names: Vec<String> = list.iter().map(|x| x.0.clone()).collect();
        if names == self.names {
            return false;
        }
        self.names = names;
        self.items = list
            .into_iter()
            .map(|(path, name, is_dir)| {
                let p = std::path::Path::new(&path);
                // .desktop: ícone e nome do app; imagem: miniatura
                let (name, icon) = if p.extension().is_some_and(|e| e == "desktop") {
                    let text = std::fs::read_to_string(p).unwrap_or_default();
                    let get = |k: &str| text.lines().find_map(|l| l.strip_prefix(k)).map(str::to_string);
                    let n = get("Name=").unwrap_or(name);
                    let ic = get("Icon=").unwrap_or_default();
                    (n.clone(), Some(icons.get_theme(&ic, &n).clone()))
                } else if matches!(p.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref(), Some("png" | "jpg" | "jpeg" | "webp")) {
                    (name, crate::settings_ctl::load_image(&path, 128))
                } else {
                    let ic = icon_name(p, is_dir);
                    (name.clone(), Some(icons.get_theme(ic, &name).clone()))
                };
                DeskItem { path, name, is_dir, icon }
            })
            .collect();
        self.selected = None;
        self.key = None;
        true
    }

    pub fn hit(&self, x: f64, y: f64) -> Option<usize> {
        let (x, y) = (x as f32, y as f32);
        self.rects.iter().position(|r| x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3)
    }

    /// `right`: borda direita disponível (os widgets ficam à direita dos ícones).
    #[allow(clippy::too_many_arguments)]
    pub fn update(&mut self, theme: &Theme, sw: i32, sh: i32, scale: i32, right: f32, top: f32) {
        self.load_positions();
        let rows = (((sh as f32 - top - 90.0) / CELL_H).floor() as usize).max(1);
        self.pos = (0.0, 0.0);
        let key = format!(
            "{}|{:?}|{}|{right}|{sw}|{sh}|{scale}|{}|{:?}|{:?}|{}",
            self.names.len(),
            self.selected,
            theme.appearance as u8,
            theme.accent.0,
            self.dragging,
            self.renaming,
            self.positions.len()
        );
        if self.key.as_deref() == Some(key.as_str()) && self.layer.is_set() {
            return;
        }
        self.key = Some(key);
        self.rects.clear();
        if self.items.is_empty() {
            self.layer = Layer::default();
            return;
        }
        // posições: as escolhidas pelo usuário; o resto em colunas a partir da direita
        let mut cells: Vec<(f32, f32)> = Vec::new();
        let mut auto = 0usize;
        for it in &self.items {
            if let Some(&(x, y)) = self.positions.get(&it.name) {
                cells.push((x.clamp(0.0, sw as f32 - CELL_W), y.clamp(top, sh as f32 - CELL_H)));
                continue;
            }
            loop {
                let (col, row) = (auto / rows, auto % rows);
                auto += 1;
                let cell = (right - (col as f32 + 1.0) * CELL_W, top + row as f32 * CELL_H);
                let taken = self.positions.values().any(|&(x, y)| (x - cell.0).abs() < CELL_W / 2.0 && (y - cell.1).abs() < CELL_H / 2.0);
                if !taken {
                    cells.push(cell);
                    break;
                }
            }
        }
        let Some(mut c) = Canvas::new(sw, sh, scale as f32) else { return };
        for (i, (it, &(x, y))) in self.items.iter().zip(cells.iter()).enumerate() {
            let sel = self.selected == Some(i);
            let alpha = if self.dragging == Some(i) { 0.4 } else { 1.0 };
            let ix = x + (CELL_W - ICON) / 2.0;
            if sel {
                c.fill_rrect(ix - 4.0, y + 2.0, ICON + 8.0, ICON + 8.0, 8.0, Rgba(0, 0, 0, 70));
            }
            if let Some(pm) = &it.icon {
                let (pw, ph) = (pm.width() as f32, pm.height() as f32);
                let s = (ICON / pw).min(ICON / ph);
                let (dw, dh) = (pw * s, ph * s);
                c.blit(pm, ix + (ICON - dw) / 2.0, y + 6.0 + (ICON - dh) / 2.0, dw, dh, alpha);
            }
            let ly = y + ICON + 12.0;
            match &self.renaming {
                // renomeando: campo branco com o texto e o cursor (como no Finder)
                Some((ri, text)) if *ri == i => {
                    let tw = c.measure(text, 12.0, Weight::Medium).max(20.0);
                    let fw = (tw + 12.0).min(CELL_W + 40.0);
                    let fx = x + (CELL_W - fw) / 2.0;
                    c.fill_rrect(fx, ly - 2.0, fw, 18.0, 4.0, Rgba(255, 255, 255, 255));
                    c.stroke_rrect(fx, ly - 2.0, fw, 18.0, 4.0, 2.0, theme.accent);
                    let shown = c.ellipsized(text, 12.0, Weight::Medium, fw - 12.0);
                    let w2 = c.text(&shown, fx + 6.0, ly, 12.0, Weight::Medium, Rgba(0, 0, 0, 255));
                    c.fill_rrect(fx + 6.0 + w2 + 1.0, ly + 1.0, 1.5, 13.0, 0.0, theme.accent);
                }
                _ => {
                    let mut ly = ly;
                    for line in super::notch::wrap(&it.name, CELL_W - 8.0, 2, &c) {
                        let lw = c.measure(&line, 12.0, Weight::Medium);
                        let lx = x + (CELL_W - lw) / 2.0;
                        if sel {
                            c.fill_rrect(lx - 4.0, ly - 1.0, lw + 8.0, 16.0, 4.0, theme.accent);
                        } else {
                            c.text(&line, lx, ly + 1.0, 12.0, Weight::Medium, Rgba(0, 0, 0, (150.0 * alpha) as u8));
                        }
                        c.text(&line, lx, ly, 12.0, Weight::Medium, Rgba(255, 255, 255, (255.0 * alpha) as u8));
                        ly += 15.0;
                    }
                }
            }
            self.rects.push((x + 4.0, y, CELL_W - 8.0, CELL_H - 4.0));
        }
        self.layer.set(&c);
    }
}
