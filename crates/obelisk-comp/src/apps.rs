//! Leitura de arquivos `.desktop` (freedesktop) para o launcher.

use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct App {
    /// Nome do arquivo .desktop sem extensão (ex.: "org.gnome.Nautilus").
    pub id: String,
    pub wm_class: String,
    pub categories: String,
    pub name: String,
    pub exec: String,
    pub icon: String,
    pub keywords: String,
    pub generic: String,
    pub mime: String,
}

fn dirs() -> Vec<PathBuf> {
    let mut v = vec![PathBuf::from("/usr/share/applications"), PathBuf::from("/usr/local/share/applications")];
    if let Some(h) = std::env::var_os("HOME") {
        v.push(PathBuf::from(h).join(".local/share/applications"));
    }
    if let Some(xdg) = std::env::var_os("XDG_DATA_DIRS") {
        for d in std::env::split_paths(&xdg) {
            let p = d.join("applications");
            if !v.contains(&p) {
                v.push(p);
            }
        }
    }
    v
}

/// Remove códigos de campo (%f, %U...) do Exec.
fn clean_exec(exec: &str) -> String {
    let mut out = String::new();
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            match chars.next() {
                Some('%') => out.push('%'),
                _ => {}
            }
        } else {
            out.push(c);
        }
    }
    out.trim().to_string()
}

fn parse(path: &std::path::Path, lang: &str) -> Option<App> {
    let text = fs::read_to_string(path).ok()?;
    let mut in_entry = false;
    let (mut name, mut name_loc, mut exec, mut icon, mut kw, mut generic) =
        (None, None, None, String::new(), String::new(), String::new());
    let (mut hidden, mut is_app) = (false, false);
    let (mut wm_class, mut categories, mut mime) = (String::new(), String::new(), String::new());
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        match k {
            "Type" => is_app = v == "Application",
            "Name" => name = Some(v.to_string()),
            k if k == format!("Name[{lang}]") => name_loc = Some(v.to_string()),
            "Exec" => exec = Some(clean_exec(v)),
            "Icon" => icon = v.to_string(),
            "Keywords" => kw = v.replace(';', " "),
            "GenericName" => generic = v.to_string(),
            "StartupWMClass" => wm_class = v.to_string(),
            "Categories" => categories = v.to_string(),
            "MimeType" => mime = v.to_string(),
            "NoDisplay" | "Hidden" => hidden |= v == "true",
            _ => {}
        }
    }
    if !is_app || hidden {
        return None;
    }
    let id = path.file_stem()?.to_string_lossy().to_string();
    Some(App { id, wm_class, categories, name: name_loc.or(name)?, exec: exec?, icon, keywords: kw, generic, mime })
}

pub fn load() -> Vec<App> {
    let lang = std::env::var("LANG").unwrap_or_default();
    let lang = lang.split('.').next().unwrap_or("").to_string();
    let mut apps: Vec<App> = Vec::new();
    for d in dirs() {
        let Ok(rd) = fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "desktop") {
                if let Some(app) = parse(&p, &lang) {
                    if !apps.iter().any(|a| a.name == app.name) {
                        apps.push(app);
                    }
                }
            }
        }
    }
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}

/// Pontuação de busca (maior = melhor); `None` se não casa.
pub fn score(app: &App, query: &str) -> Option<i32> {
    let q = query.to_lowercase();
    let name = app.name.to_lowercase();
    if name == q {
        return Some(1000);
    }
    if name.starts_with(&q) {
        return Some(800 - name.len() as i32);
    }
    if name.split_whitespace().any(|w| w.starts_with(&q)) {
        return Some(600 - name.len() as i32);
    }
    if name.contains(&q) {
        return Some(400 - name.len() as i32);
    }
    let hay = format!("{} {} {}", app.keywords, app.generic, app.exec).to_lowercase();
    if hay.contains(&q) {
        return Some(200);
    }
    // subsequência
    let mut it = name.chars();
    if q.chars().all(|c| it.any(|n| n == c)) {
        return Some(100);
    }
    None
}

/// Primeiro executável da lista encontrado no PATH.
pub fn first_in_path(cands: &[&str]) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for c in cands {
        let bin = c.split_whitespace().next()?;
        if std::env::split_paths(&path).any(|d| d.join(bin).is_file()) {
            return Some(c.to_string());
        }
    }
    None
}

/// Pedidos para apps internos do Obelisk (Ajustes, App Store, Instalador) vindos de .desktop.
static INTERNAL: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

pub fn take_internal() -> Vec<String> {
    std::mem::take(&mut *INTERNAL.lock().unwrap())
}

pub fn spawn(cmd: &str) {
    if let Some(what) = cmd.trim().strip_prefix("obelisk-open ") {
        INTERNAL.lock().unwrap().push(what.trim().to_string());
        return;
    }
    tracing::info!("executando: {cmd}");
    if let Err(e) = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("exec {cmd}"))
        .stdin(std::process::Stdio::null())
        .spawn()
    {
        tracing::warn!("falha ao executar {cmd}: {e}");
    }
}

/// O app de uma janela, pelo `app_id` (id do .desktop, StartupWMClass, nome do binário ou nome).
pub fn for_app_id<'a>(apps: &'a [App], app_id: &str) -> Option<&'a App> {
    let a = app_id.to_lowercase();
    if a.is_empty() {
        return None;
    }
    apps.iter()
        .find(|x| x.id.to_lowercase() == a || x.wm_class.to_lowercase() == a)
        .or_else(|| apps.iter().find(|x| x.id.to_lowercase().rsplit('.').next() == Some(a.as_str())))
        .or_else(|| {
            apps.iter().find(|x| {
                let bin = x.exec.split_whitespace().next().unwrap_or("").rsplit('/').next().unwrap_or("").to_lowercase();
                bin == a || x.name.to_lowercase() == a
            })
        })
}

/// A janela pertence a este app?
pub fn matches(app: &App, app_id: &str) -> bool {
    let a = app_id.to_lowercase();
    if a.is_empty() {
        return false;
    }
    let bin = app.exec.split_whitespace().next().unwrap_or("").rsplit('/').next().unwrap_or("").to_lowercase();
    app.id.to_lowercase() == a
        || app.wm_class.to_lowercase() == a
        || app.id.to_lowercase().rsplit('.').next() == Some(a.as_str())
        || bin == a
}

/// Apps que declaram abrir esse tipo (para "Apps Padrão"). Para navegador, usa http/https.
pub fn handlers_for(mime: &str) -> Vec<App> {
    let major = mime.split('/').next().unwrap_or("");
    load()
        .into_iter()
        .filter(|a| a.mime.split(';').any(|m| m == mime || (major == "image" || major == "video" || major == "audio") && m.starts_with(&format!("{major}/"))))
        .collect()
}

/// Descrição (Comment) do .desktop do app, no idioma do sistema se houver.
pub fn desktop_comment(id: &str) -> String {
    let lang = std::env::var("LANG").unwrap_or_default();
    let lang = lang.split('.').next().unwrap_or("").to_string();
    for d in dirs() {
        let Ok(text) = fs::read_to_string(d.join(format!("{id}.desktop"))) else { continue };
        let loc = text.lines().find_map(|l| l.strip_prefix(&format!("Comment[{lang}]=")));
        let plain = text.lines().find_map(|l| l.strip_prefix("Comment="));
        return loc.or(plain).unwrap_or("").to_string();
    }
    String::new()
}
