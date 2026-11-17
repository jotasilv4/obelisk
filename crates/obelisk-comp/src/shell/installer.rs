//! Instalador gráfico do Obelisk (como o "Instalar macOS"): idioma, teclado e fuso, disco
//! (com criptografia opcional), conta, resumo, progresso e fim. Roda no sistema live; a parte
//! pesada é o `obelisk-install-auto` (root, via sudo), que informa o progresso por linhas.

use resvg::tiny_skia::Pixmap;

use super::layer::Layer;
use crate::shell::settings_model::{q, sh};
use crate::ui::canvas::Canvas;
use crate::ui::text::Weight;
use crate::ui::text_glyph;
use crate::ui::theme::{Rgba, Theme};

#[derive(Clone, Debug, PartialEq)]
pub enum InstHit {
    Next,
    Back,
    Lang(usize),
    Keymap(usize),
    Zone(String),
    Disk(usize),
    Encrypt,
    Field(usize),
    Network,
    Reboot,
    Retry,
    Log,
    /// fechar e experimentar o sistema live
    Try,
}

pub const LANGS: [(&str, &str); 2] = [("Português (Brasil)", "pt_BR.UTF-8"), ("English (US)", "en_US.UTF-8")];
pub const KEYMAPS: [(&str, &str); 3] = [("ABNT2", "br"), ("EUA", "us"), ("EUA internacional", "us-intl")];
const FIELDS: [&str; 5] = ["Nome completo", "Nome da conta", "Senha", "Confirmar senha", "Nome do computador"];

#[derive(Clone, Debug)]
pub struct Disk {
    pub path: String,
    pub model: String,
    pub size_gb: f64,
    pub kind: String,
}

const W: f32 = 760.0;
const H: f32 = 560.0;

pub struct Installer {
    pub open: bool,
    pub step: usize,
    pub lang: usize,
    pub keymap: usize,
    pub zone: String,
    pub zone_query: String,
    zones: Vec<String>,
    pub disks: Vec<Disk>,
    pub disk: Option<usize>,
    pub encrypt: bool,
    pub uefi: bool,
    pub fields: [String; 5],
    pub user_edited: bool,
    /// campo com o cursor (5 = busca do fuso)
    pub focus: Option<usize>,
    pub progress: (f32, String),
    pub error: Option<String>,
    pub done: bool,
    pub started: Option<std::time::Instant>,
    pub layer: Layer,
    hits: Vec<(InstHit, (f32, f32, f32, f32))>,
    key: Option<String>,
}

impl Default for Installer {
    fn default() -> Self {
        Installer {
            open: false,
            step: 0,
            lang: 0,
            keymap: 0,
            zone: "America/Sao_Paulo".into(),
            zone_query: String::new(),
            zones: vec![],
            disks: vec![],
            disk: None,
            encrypt: false,
            uefi: std::path::Path::new("/sys/firmware/efi").exists(),
            fields: Default::default(),
            user_edited: false,
            focus: None,
            progress: (0.0, String::new()),
            error: None,
            done: false,
            started: None,
            layer: Layer::default(),
            hits: vec![],
            key: None,
        }
    }
}

/// Estamos no sistema live (a ISO)?
pub fn is_live() -> bool {
    std::path::Path::new("/run/archiso").exists()
}

/// Discos onde dá para instalar (sem o pendrive do live, sem loop/zram/cd).
pub fn list_disks() -> Vec<Disk> {
    let live = sh("findmnt -no SOURCE /run/archiso/bootmnt 2>/dev/null | xargs -r lsblk -no PKNAME 2>/dev/null").trim().to_string();
    sh("lsblk -dpnbo NAME,SIZE,TYPE,TRAN,MODEL")
        .lines()
        .filter_map(|l| {
            let mut f = l.split_whitespace();
            let (name, size, ty) = (f.next()?, f.next()?.parse::<f64>().ok()?, f.next()?);
            let rest: Vec<&str> = f.collect();
            let (tran, model) = match rest.split_first() {
                Some((t, m)) if matches!(*t, "sata" | "nvme" | "usb" | "ata" | "scsi" | "virtio" | "mmc") => (t.to_string(), m.join(" ")),
                _ => (String::new(), rest.join(" ")),
            };
            if ty != "disk" || name.contains("zram") || name.contains("loop") || (!live.is_empty() && name.ends_with(&live)) || size < 20e9 {
                return None;
            }
            let kind = match tran.as_str() {
                "nvme" => "SSD NVMe",
                "usb" => "Disco USB",
                "virtio" => "Disco virtual",
                _ if name.contains("nvme") => "SSD NVMe",
                _ => "Disco",
            };
            Some(Disk { path: name.into(), model: if model.is_empty() { name.into() } else { model }, size_gb: size / 1e9, kind: kind.into() })
        })
        .collect()
}

/// "João da Silva" → "joao"
fn login_from(name: &str) -> String {
    let first = name.split_whitespace().next().unwrap_or("").to_lowercase();
    first
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'ê' | 'è' => 'e',
            'í' | 'î' => 'i',
            'ó' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        })
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        .collect()
}

impl Installer {
    pub fn show(&mut self) {
        self.open = true;
        self.disks = list_disks();
        if self.disk.is_none() && self.disks.len() == 1 {
            self.disk = Some(0);
        }
        if self.zones.is_empty() {
            self.zones = sh("timedatectl list-timezones").lines().map(str::to_string).collect();
        }
        if self.fields[4].is_empty() {
            self.fields[4] = "obelisk".into();
        }
        self.key = None;
    }

    pub fn hit(&self, x: f64, y: f64) -> Option<InstHit> {
        let (x, y) = (x as f32, y as f32);
        self.hits.iter().rev().find(|(_, r)| x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3).map(|(h, _)| h.clone())
    }

    /// Texto digitado no campo com o cursor.
    pub fn type_text(&mut self, s: &str) {
        match self.focus {
            Some(5) => self.zone_query.push_str(s),
            Some(i) if i < 5 => {
                self.fields[i].push_str(s);
                if i == 1 {
                    self.user_edited = true;
                }
                if i == 0 && !self.user_edited {
                    self.fields[1] = login_from(&self.fields[0]);
                }
            }
            _ => {}
        }
        self.key = None;
    }

    pub fn backspace(&mut self) {
        match self.focus {
            Some(5) => {
                self.zone_query.pop();
            }
            Some(i) if i < 5 => {
                self.fields[i].pop();
                if i == 0 && !self.user_edited {
                    self.fields[1] = login_from(&self.fields[0]);
                }
            }
            _ => {}
        }
        self.key = None;
    }

    /// Tab: próximo campo da tela de conta.
    pub fn next_field(&mut self) {
        if self.step == 3 {
            self.focus = Some(self.focus.map(|i| (i + 1) % 5).unwrap_or(0));
            self.key = None;
        }
    }

    /// O que impede continuar neste passo (None = pode).
    pub fn blocker(&self) -> Option<&'static str> {
        match self.step {
            2 if self.disk.is_none() => Some("Escolha o disco."),
            3 => {
                let user = &self.fields[1];
                if self.fields[0].trim().is_empty() {
                    Some("Digite seu nome.")
                } else if user.is_empty() || !user.chars().next().is_some_and(|c| c.is_ascii_lowercase()) || !user.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_') {
                    Some("O nome da conta usa só letras minúsculas e números.")
                } else if self.fields[2].chars().count() < 4 {
                    Some("A senha precisa de pelo menos 4 caracteres.")
                } else if self.fields[2] != self.fields[3] {
                    Some("As senhas não são iguais.")
                } else if self.fields[4].is_empty() || !self.fields[4].chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                    Some("Nome do computador: letras, números e hífen.")
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Arquivo de configuração para o `obelisk-install-auto` (apagado por ele ao ler).
    pub fn write_config(&self) -> std::io::Result<String> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let path = format!("/tmp/obelisk-install-{}.conf", std::process::id());
        let disk = self.disk.and_then(|i| self.disks.get(i)).map(|d| d.path.clone()).unwrap_or_default();
        let body = [
            ("DISK", disk),
            ("FULLNAME", self.fields[0].trim().to_string()),
            ("USERNAME", self.fields[1].clone()),
            ("PASSWORD", self.fields[2].clone()),
            ("HOSTNAME", self.fields[4].clone()),
            ("TIMEZONE", self.zone.clone()),
            ("KEYMAP", KEYMAPS[self.keymap].1.to_string()),
            ("LOCALE", LANGS[self.lang].1.to_string()),
            ("ENCRYPT", if self.encrypt { "1" } else { "0" }.to_string()),
        ]
        .iter()
        .map(|(k, v)| format!("{k}={}\n", q(v)))
        .collect::<String>();
        let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&path)?;
        f.write_all(body.as_bytes())?;
        Ok(path)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update(&mut self, theme: &Theme, network: Option<&str>, sw: i32, sh_: i32, scale: i32, backdrop: Option<&Pixmap>) {
        let elapsed = self.started.map(|t| t.elapsed().as_secs() / 5).unwrap_or(0);
        let key = format!(
            "{}|{}|{}|{}|{}|{:?}|{}|{:?}|{:?}|{:?}|{:?}|{:?}|{sw}|{sh_}|{scale}|{}|{elapsed}",
            self.step,
            self.lang,
            self.keymap,
            self.zone,
            self.zone_query,
            self.disk,
            self.encrypt,
            self.fields.iter().map(|f| f.len()).collect::<Vec<_>>(),
            self.focus,
            self.progress,
            self.error,
            network,
            self.done
        );
        if self.key.as_deref() == Some(key.as_str()) && self.layer.is_set() {
            return;
        }
        self.key = Some(key);
        self.hits.clear();
        let (w, h) = (sw as f32, sh_ as f32);
        let mut c = Canvas::new(sw, sh_, scale as f32).unwrap();
        c.fill_rrect(0.0, 0.0, w, h, 0.0, Rgba(11, 15, 31, 255));
        if let Some(bd) = backdrop {
            c.fill_rrect_backdrop(0.0, 0.0, w, h, 0.0, bd, (0.0, 0.0), (w, h));
        }
        c.fill_rrect(0.0, 0.0, w, h, 0.0, Rgba(0, 0, 0, 70));
        let (ox, oy) = (((w - W) / 2.0).round(), ((h - H) / 2.0).round().max(30.0));
        c.shadow_rrect_add(ox, oy + 16.0, W, H, 14.0, 40.0, 140);
        c.fill_rrect(ox, oy, W, H, 14.0, theme.window_bg);
        c.stroke_rrect(ox, oy, W, H, 14.0, 1.0, theme.fg(0.10));
        // barra de título
        let title = "Instalar o Obelisk";
        let tw = c.measure(title, 13.0, Weight::Semibold);
        c.text_vcenter(title, ox + (W - tw) / 2.0, oy, 36.0, 13.0, Weight::Semibold, theme.secondary);
        c.fill_rrect(ox, oy + 36.0, W, 1.0, 0.0, theme.separator);
        let cx = ox + W / 2.0;
        let (lx, lw) = (ox + 60.0, W - 120.0);
        let mut y = oy + 64.0;
        let center = |c: &mut Canvas, t: &str, y: f32, size: f32, wt: Weight, col: Rgba| {
            let tw = c.measure(t, size, wt);
            c.text(t, cx - tw / 2.0, y, size, wt, col);
        };
        let heading = |c: &mut Canvas, y: &mut f32, t: &str, sub: &str| {
            let tw = c.measure(t, 24.0, Weight::Bold);
            c.text(t, cx - tw / 2.0, *y, 24.0, Weight::Bold, theme.label);
            *y += 36.0;
            if !sub.is_empty() {
                let sw2 = c.measure(sub, 13.0, Weight::Regular);
                c.text(sub, cx - sw2 / 2.0, *y, 13.0, Weight::Regular, theme.secondary);
                *y += 32.0;
            }
        };
        let field = |c: &mut Canvas, hits: &mut Vec<(InstHit, (f32, f32, f32, f32))>, x: f32, y: f32, w: f32, label: &str, value: &str, secret: bool, focused: bool, idx: usize| {
            c.text(label, x, y, 12.0, Weight::Medium, theme.secondary);
            let fy = y + 18.0;
            c.fill_rrect(x, fy, w, 30.0, 7.0, theme.fg(0.06));
            c.stroke_rrect(x, fy, w, 30.0, 7.0, if focused { 2.0 } else { 1.0 }, if focused { theme.accent } else { theme.fg(0.14) });
            let shown = if secret { "●".repeat(value.chars().count()) } else { value.to_string() };
            let tw = c.text_vcenter(&c.ellipsized(&shown, 13.0, Weight::Regular, w - 20.0), x + 10.0, fy, 30.0, 13.0, Weight::Regular, theme.label);
            if focused {
                c.fill_rrect(x + 11.0 + tw, fy + 8.0, 1.5, 14.0, 0.0, theme.accent);
            }
            hits.push((InstHit::Field(idx), (x, fy, w, 30.0)));
        };
        let segmented = |c: &mut Canvas, hits: &mut Vec<(InstHit, (f32, f32, f32, f32))>, y: f32, opts: &[&str], sel: usize, mk: &dyn Fn(usize) -> InstHit| {
            let widths: Vec<f32> = opts.iter().map(|t| c.measure(t, 13.0, Weight::Medium) + 32.0).collect();
            let total: f32 = widths.iter().sum();
            let mut bx = cx - total / 2.0;
            c.fill_rrect(bx - 2.0, y - 2.0, total + 4.0, 34.0, 9.0, theme.fg(0.08));
            for (i, (t, bw)) in opts.iter().zip(widths).enumerate() {
                if i == sel {
                    c.shadow_rrect_add(bx, y + 1.0, bw, 30.0, 7.0, 4.0, 50);
                    c.fill_rrect(bx, y, bw, 30.0, 7.0, theme.window_bg.with_alpha(255));
                    c.fill_rrect(bx, y, bw, 30.0, 7.0, theme.fg(0.10));
                }
                c.text_vcenter(t, bx + 16.0, y, 30.0, 13.0, Weight::Medium, theme.label);
                hits.push((mk(i), (bx, y, bw, 30.0)));
                bx += bw;
            }
        };

        let mut show_buttons = true;
        match self.step {
            0 => {
                c.fill_rrect_vgradient(cx - 60.0, y + 10.0, 120.0, 120.0, 28.0, Rgba::rgb(0x4a, 0x6d, 0xf0), Rgba::rgb(0x17, 0x1d, 0x5c));
                text_glyph::obelisk_logo(&mut c, cx, y + 70.0, 76.0, Rgba::rgb(255, 255, 255));
                y += 156.0;
                center(&mut c, "Obelisk", y, 30.0, Weight::Bold, theme.label);
                y += 44.0;
                center(&mut c, "Rápido, bonito e feito para jogar.", y, 14.0, Weight::Regular, theme.secondary);
                y += 50.0;
                center(&mut c, "Idioma", y, 12.0, Weight::Semibold, theme.tertiary);
                y += 22.0;
                segmented(&mut c, &mut self.hits, y, &LANGS.iter().map(|l| l.0).collect::<Vec<_>>(), self.lang, &InstHit::Lang);
            }
            1 => {
                heading(&mut c, &mut y, "Região", "Teclado e fuso horário.");
                center(&mut c, "Teclado", y, 12.0, Weight::Semibold, theme.tertiary);
                y += 22.0;
                segmented(&mut c, &mut self.hits, y, &KEYMAPS.iter().map(|k| k.0).collect::<Vec<_>>(), self.keymap, &InstHit::Keymap);
                y += 56.0;
                let fw = 360.0;
                field(&mut c, &mut self.hits, cx - fw / 2.0, y, fw, &format!("Fuso horário: {}", self.zone.replace('_', " ")), &self.zone_query, false, self.focus == Some(5), 5);
                y += 58.0;
                let qy = self.zone_query.to_lowercase().replace(' ', "_");
                let list: Vec<&String> = if qy.len() >= 2 {
                    self.zones.iter().filter(|z| z.to_lowercase().contains(&qy)).take(5).collect()
                } else {
                    vec![]
                };
                for z in list {
                    let sel = *z == self.zone;
                    let r = (cx - fw / 2.0, y, fw, 26.0);
                    if sel {
                        c.fill_rrect(r.0, r.1, r.2, r.3, 6.0, theme.accent);
                    }
                    c.text_vcenter(&z.replace('_', " "), r.0 + 10.0, y, 26.0, 13.0, Weight::Regular, if sel { theme.text_on_accent } else { theme.label });
                    self.hits.push((InstHit::Zone(z.clone()), r));
                    y += 28.0;
                }
            }
            2 => {
                heading(&mut c, &mut y, "Disco", "Onde o Obelisk vai ficar.");
                if self.disks.is_empty() {
                    center(&mut c, "Nenhum disco com pelo menos 20 GB foi encontrado.", y + 20.0, 14.0, Weight::Regular, theme.red);
                }
                for (i, d) in self.disks.iter().enumerate().take(4) {
                    let r = (lx, y, lw, 56.0);
                    let sel = self.disk == Some(i);
                    c.fill_rrect(r.0, r.1, r.2, r.3, 10.0, if sel { theme.accent.with_alpha(40) } else { theme.fg(0.05) });
                    c.stroke_rrect(r.0, r.1, r.2, r.3, 10.0, if sel { 2.0 } else { 1.0 }, if sel { theme.accent } else { theme.fg(0.08) });
                    c.symbol("hard_drive", lx + 30.0, y + 28.0, 28.0, if sel { theme.accent } else { theme.secondary });
                    let m = c.ellipsized(&d.model, 14.0, Weight::Semibold, lw - 200.0);
                    c.text(&m, lx + 56.0, y + 10.0, 14.0, Weight::Semibold, theme.label);
                    c.text(&format!("{} · {}", d.kind, d.path), lx + 56.0, y + 30.0, 12.0, Weight::Regular, theme.secondary);
                    let size = format!("{:.0} GB", d.size_gb);
                    let sw2 = c.measure(&size, 14.0, Weight::Medium);
                    c.text_vcenter(&size, lx + lw - 16.0 - sw2, y, 56.0, 14.0, Weight::Medium, theme.label);
                    self.hits.push((InstHit::Disk(i), r));
                    y += 64.0;
                }
                y += 6.0;
                // criptografia (como o FileVault): pede a senha da conta ao ligar
                let r = (lx, y, lw, 50.0);
                c.fill_rrect(r.0, r.1, r.2, r.3, 10.0, theme.fg(0.05));
                c.text(if self.uefi { "Criptografar o disco" } else { "Criptografar o disco (só em UEFI)" }, lx + 16.0, y + 8.0, 13.0, Weight::Regular, theme.label);
                c.text("Pede a senha da sua conta ao ligar o computador.", lx + 16.0, y + 27.0, 11.0, Weight::Regular, theme.secondary);
                let (tx, ty) = (lx + lw - 16.0 - 40.0, y + 13.0);
                c.fill_rrect(tx, ty, 40.0, 24.0, 12.0, if self.encrypt { theme.accent } else { theme.fg(0.16) });
                c.fill_circle(if self.encrypt { tx + 28.0 } else { tx + 12.0 }, ty + 12.0, 10.0, Rgba::rgb(255, 255, 255));
                if self.uefi {
                    self.hits.push((InstHit::Encrypt, r));
                }
                y += 64.0;
                if self.disk.is_some() {
                    center(&mut c, "Tudo o que estiver neste disco será apagado.", y, 13.0, Weight::Semibold, theme.red);
                }
            }
            3 => {
                heading(&mut c, &mut y, "Sua conta", "");
                let fw = (lw - 20.0) / 2.0;
                let pos = [(lx, y), (lx + fw + 20.0, y), (lx, y + 62.0), (lx + fw + 20.0, y + 62.0), (lx, y + 124.0)];
                for (i, (fx, fy)) in pos.iter().enumerate() {
                    field(&mut c, &mut self.hits, *fx, *fy, fw, FIELDS[i], &self.fields[i], i == 2 || i == 3, self.focus == Some(i), i);
                }
                y += 196.0;
                if let Some(b) = self.blocker().filter(|_| !self.fields[0].is_empty()) {
                    center(&mut c, b, y, 12.0, Weight::Regular, theme.secondary);
                }
            }
            4 => {
                heading(&mut c, &mut y, "Tudo certo?", "Confira antes de instalar.");
                let disk = self.disk.and_then(|i| self.disks.get(i)).map(|d| format!("{} ({:.0} GB) — será apagado", d.model, d.size_gb)).unwrap_or_default();
                let rows = [
                    ("Disco", disk),
                    ("Criptografia", if self.encrypt { "Ligada".into() } else { "Desligada".into() }),
                    ("Conta", format!("{} ({})", self.fields[0].trim(), self.fields[1])),
                    ("Computador", self.fields[4].clone()),
                    ("Teclado e fuso", format!("{} · {}", KEYMAPS[self.keymap].0, self.zone.replace('_', " "))),
                    ("Internet", network.map(str::to_string).unwrap_or_else(|| "Sem conexão".into())),
                ];
                c.fill_rrect(lx, y, lw, rows.len() as f32 * 40.0, 10.0, theme.fg(0.05));
                for (i, (k, v)) in rows.iter().enumerate() {
                    let ry = y + i as f32 * 40.0;
                    c.text_vcenter(k, lx + 16.0, ry, 40.0, 13.0, Weight::Regular, theme.label);
                    let vv = c.ellipsized(v, 13.0, Weight::Regular, lw - 180.0);
                    let vw = c.measure(&vv, 13.0, Weight::Regular);
                    let col = if *k == "Internet" && network.is_none() { theme.red } else { theme.secondary };
                    c.text_vcenter(&vv, lx + lw - 16.0 - vw, ry, 40.0, 13.0, Weight::Regular, col);
                    if i + 1 < rows.len() {
                        c.fill_rrect(lx + 16.0, ry + 39.5, lw - 32.0, 1.0, 0.0, theme.separator);
                    }
                }
                y += rows.len() as f32 * 40.0 + 16.0;
                if network.is_none() {
                    let label = "Conectar a uma rede…";
                    let bw = c.measure(label, 13.0, Weight::Medium) + 28.0;
                    c.fill_rrect(cx - bw / 2.0, y, bw, 28.0, 7.0, theme.fg(0.10));
                    c.text_vcenter(label, cx - bw / 2.0 + 14.0, y, 28.0, 13.0, Weight::Medium, theme.label);
                    self.hits.push((InstHit::Network, (cx - bw / 2.0, y, bw, 28.0)));
                }
            }
            5 => {
                show_buttons = false;
                c.fill_rrect_vgradient(cx - 44.0, y + 20.0, 88.0, 88.0, 20.0, Rgba::rgb(0x4a, 0x6d, 0xf0), Rgba::rgb(0x17, 0x1d, 0x5c));
                text_glyph::obelisk_logo(&mut c, cx, y + 64.0, 56.0, Rgba::rgb(255, 255, 255));
                y += 140.0;
                center(&mut c, "Instalando o Obelisk", y, 22.0, Weight::Bold, theme.label);
                y += 50.0;
                let (bw, bx) = (lw - 80.0, lx + 40.0);
                c.fill_rrect(bx, y, bw, 6.0, 3.0, theme.fg(0.12));
                c.fill_rrect(bx, y, (bw * self.progress.0 / 100.0).max(6.0), 6.0, 3.0, theme.accent);
                y += 22.0;
                let msg = c.ellipsized(&self.progress.1, 13.0, Weight::Regular, lw);
                center(&mut c, &msg, y, 13.0, Weight::Regular, theme.secondary);
                y += 24.0;
                let mins = self.started.map(|t| t.elapsed().as_secs() / 60).unwrap_or(0);
                center(&mut c, &format!("{mins} min · não desligue o computador"), y, 12.0, Weight::Regular, theme.tertiary);
            }
            _ => {
                show_buttons = false;
                let ok = self.error.is_none();
                c.fill_circle(cx, y + 64.0, 46.0, if ok { theme.accent } else { theme.red });
                c.symbol(if ok { "check" } else { "close" }, cx, y + 64.0, 52.0, Rgba::rgb(255, 255, 255));
                y += 136.0;
                center(&mut c, if ok { "O Obelisk foi instalado" } else { "A instalação não terminou" }, y, 24.0, Weight::Bold, theme.label);
                y += 42.0;
                let sub = match &self.error {
                    None => "Tire o pendrive e reinicie para começar.".to_string(),
                    Some(e) => e.clone(),
                };
                for line in super::notch::wrap(&sub, lw, 3, &c) {
                    center(&mut c, &line, y, 13.0, Weight::Regular, theme.secondary);
                    y += 18.0;
                }
                let (label, hit) = if ok { ("Reiniciar", InstHit::Reboot) } else { ("Tentar de novo", InstHit::Retry) };
                let bw = c.measure(label, 14.0, Weight::Semibold) + 48.0;
                let (bx, by) = (cx - bw / 2.0, oy + H - 70.0);
                c.fill_rrect(bx, by, bw, 34.0, 9.0, theme.accent);
                c.text_vcenter(label, bx + 24.0, by, 34.0, 14.0, Weight::Semibold, theme.text_on_accent);
                self.hits.push((hit, (bx, by, bw, 34.0)));
                if !ok {
                    let t = "Ver o registro";
                    let tw2 = c.measure(t, 13.0, Weight::Medium);
                    c.text_vcenter(t, ox + 30.0, by, 34.0, 13.0, Weight::Medium, theme.accent);
                    self.hits.push((InstHit::Log, (ox + 20.0, by, tw2 + 20.0, 34.0)));
                }
            }
        }

        if show_buttons {
            let blocked = self.blocker().is_some() || (self.step == 4 && network.is_none());
            let label = if self.step == 4 { "Instalar" } else { "Continuar" };
            let bw = c.measure(label, 14.0, Weight::Semibold) + 48.0;
            let (bx, by) = (ox + W - 28.0 - bw, oy + H - 56.0);
            let a = if blocked { 110 } else { 255 };
            c.fill_rrect(bx, by, bw, 34.0, 9.0, theme.accent.with_alpha(a));
            c.text_vcenter(label, bx + 24.0, by, 34.0, 14.0, Weight::Semibold, theme.text_on_accent.with_alpha(a));
            if !blocked {
                self.hits.push((InstHit::Next, (bx, by, bw, 34.0)));
            }
            if self.step == 0 {
                let t = "Experimentar primeiro";
                let tw2 = c.measure(t, 14.0, Weight::Medium);
                c.text_vcenter(t, ox + 30.0, by, 34.0, 14.0, Weight::Medium, theme.accent);
                self.hits.push((InstHit::Try, (ox + 20.0, by, tw2 + 20.0, 34.0)));
            }
            if self.step > 0 {
                let t = "Voltar";
                let tw2 = c.measure(t, 14.0, Weight::Medium);
                c.text_vcenter(t, ox + 30.0, by, 34.0, 14.0, Weight::Medium, theme.accent);
                self.hits.push((InstHit::Back, (ox + 20.0, by, tw2 + 20.0, 34.0)));
            }
            // passos
            for i in 0..5 {
                c.fill_circle(cx - 28.0 + i as f32 * 14.0, oy + H - 39.0, 3.5, if i == self.step { theme.label } else { theme.fg(0.2) });
            }
        }
        self.layer.set(&c);
    }
}
