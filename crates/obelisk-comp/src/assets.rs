//! Localização dos ativos de marca (ícones, wallpapers).
//!
//! Ordem de busca: `$OBELISK_ASSETS`, `./branding`, `<exe>/../../../branding` (desenvolvimento
//! dentro do workspace) e `/usr/share/obelisk` (instalado).

use std::path::{Path, PathBuf};

pub fn root() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(p) = std::env::var("OBELISK_ASSETS") {
        candidates.push(PathBuf::from(p));
    }
    candidates.push(PathBuf::from("branding"));
    if let Ok(exe) = std::env::current_exe() {
        for up in 1..=4 {
            if let Some(dir) = exe.ancestors().nth(up) {
                candidates.push(dir.join("branding"));
            }
        }
    }
    candidates.push(PathBuf::from("/usr/share/obelisk"));
    candidates.into_iter().find(|p| p.join("svg").is_dir())
}

pub fn icon_svg(name: &str) -> Option<PathBuf> {
    let p = root()?.join("svg/icons").join(format!("{name}.svg"));
    p.is_file().then_some(p)
}

#[allow(dead_code)]
pub fn mark_svg() -> Option<PathBuf> {
    let p = root()?.join("svg/obelisk-mark.svg");
    p.is_file().then_some(p)
}

/// Escolhe o wallpaper PNG mais adequado à largura da saída.
pub fn wallpaper(dark: bool, width: i32) -> Option<PathBuf> {
    let dir = root()?.join("wallpapers");
    let base = if dark { "obelisk-dark" } else { "obelisk-light" };
    let candidates: [(&str, i32); 2] = [(".png", 3840), ("-1080p.png", 1920)];
    // prefere a menor variante que ainda cubra a largura
    let mut best: Option<PathBuf> = None;
    for (suffix, w) in candidates.iter().rev() {
        let p = dir.join(format!("{base}{suffix}"));
        if p.is_file() {
            best = Some(p);
            if *w >= width {
                break;
            }
        }
    }
    best
}

#[allow(dead_code)]
pub fn exists(p: &Path) -> bool {
    p.exists()
}
