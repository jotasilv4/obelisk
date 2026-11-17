//! Controlador dos Ajustes: monta páginas numa thread, executa ações (comandos, root via polkit,
//! campos de texto, deslizantes) e aplica os ajustes internos do compositor.

use std::collections::HashMap;
use std::time::Instant;

use smithay::reexports::calloop::{channel, LoopHandle};

use crate::shell::settings_model::{q, Act, Ctx, Group, Row};
use crate::state::Obelisk;

#[derive(Default)]
pub struct SettingsCtl {
    pub models: HashMap<String, Vec<Group>>,
    pub fields: HashMap<String, String>,
    pub focus: Option<String>,
    pub expanded: Option<String>,
    pub tx: Option<channel::Sender<(u64, String, Vec<Group>)>>,
    /// cada montagem leva um número; respostas antigas que chegam depois são descartadas
    pub gen: u64,
    pub applied: u64,
    pub refreshed: Option<Instant>,
    /// deslizante sendo arrastado: (template, root, x0, largura)
    pub drag: Option<(String, bool, f32, f32)>,
    pub last_slider: Option<(String, u32)>,
    pub wallpapers: Option<Vec<(String, resvg::tiny_skia::Pixmap)>>,
    pub avatar: Option<Option<resvg::tiny_skia::Pixmap>>,
    /// arrastando uma tela no arranjo: (nome, ponteiro inicial, posição inicial)
    pub display_drag: Option<(String, (f64, f64), (i32, i32))>,
    pub display_preview: Option<(String, (i32, i32))>,
    /// tarefa longa em andamento: (título, última linha, %, rodando)
    pub job: Option<(String, String, f32, bool)>,
    /// atualizações encontradas na última verificação (pacman + Flatpak) e quando foi
    pub updates: usize,
    pub updates_checked: Option<Instant>,
}

/// Imagem qualquer (PNG/JPG/WebP) → Pixmap reduzido para `max` px de largura.
pub fn load_image(path: &str, max: u32) -> Option<resvg::tiny_skia::Pixmap> {
    let img = image::open(path).ok()?;
    let img = if img.width() > max { img.thumbnail(max, max * 2) } else { img };
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let mut data = rgba.into_raw();
    for px in data.chunks_exact_mut(4) {
        let a = px[3] as u16;
        for c in &mut px[..3] {
            *c = ((*c as u16 * a) / 255) as u8;
        }
    }
    resvg::tiny_skia::Pixmap::from_vec(data, resvg::tiny_skia::IntSize::from_wh(w, h)?)
}

fn wallpaper_files() -> Vec<String> {
    let home = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let mut dirs = vec![];
    if let Some(root) = crate::assets::root() {
        dirs.push(root.join("wallpapers"));
    }
    for d in ["Imagens/Papéis de Parede", "Imagens/Wallpapers", "Pictures/Wallpapers", ".local/share/backgrounds"] {
        dirs.push(home.join(d));
    }
    dirs.push("/usr/share/backgrounds".into());
    let mut out = Vec::new();
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        let mut v: Vec<String> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| matches!(e.to_string_lossy().to_lowercase().as_str(), "png" | "jpg" | "jpeg" | "webp")))
            .filter(|p| !p.to_string_lossy().contains("1080p") && !p.to_string_lossy().contains("login"))
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        v.sort();
        out.extend(v);
    }
    out.truncate(24);
    out
}

impl Obelisk {
    pub fn settings_init(&mut self, handle: &LoopHandle<'static, Obelisk>) {
        let (tx, rx) = channel::channel::<(u64, String, Vec<Group>)>();
        let _ = handle.insert_source(rx, |ev, _, state| {
            if let channel::Event::Msg((gen, page, groups)) = ev {
                if gen < state.setctl.applied {
                    return;
                }
                state.setctl.applied = gen;
                state.setctl.models.insert(page, groups);
                state.dirty = true;
            }
        });
        self.setctl.tx = Some(tx);
    }

    pub fn settings_ctx(&self) -> Ctx {
        let mut settings = self.settings.as_map();
        settings.insert("dnd".into(), self.shell.cc.focus_on.to_string());
        settings.insert("stage_manager".into(), self.stage_manager.to_string());
        settings.insert("store_query".into(), self.shell.settings.query.clone());
        for (k, v) in &self.setctl.fields {
            settings.insert(format!("field:{k}"), v.clone());
        }
        let outputs = self
            .output
            .iter()
            .map(|o| {
                let m = o.current_mode();
                (o.name(), m.map(|m| format!("{}×{} a {} Hz", m.size.w, m.size.h, (m.refresh as f32 / 1000.0).round())).unwrap_or_default())
            })
            .collect();
        Ctx {
            settings,
            expanded: self.setctl.expanded.clone(),
            nested: self.backend == crate::state::BackendKind::Nested,
            night: self.night_shift,
            outputs,
            scale: self.scale,
            fscale: self.fscale,
            modes: self.output_modes.clone(),
        }
    }

    /// Remonta a página atual numa thread.
    pub fn refresh_settings_page(&mut self) {
        let page = self.shell.settings.page.to_string();
        let ctx = self.settings_ctx();
        let Some(tx) = self.setctl.tx.clone() else { return };
        self.setctl.refreshed = Some(Instant::now());
        self.setctl.gen += 1;
        let gen = self.setctl.gen;
        std::thread::spawn(move || {
            let groups = crate::shell::settings_pages::build(&page, &ctx);
            let _ = tx.send((gen, page, groups));
        });
    }

    /// Roda um comando e depois remonta a página.
    fn run_then_refresh(&mut self, cmd: String) {
        tracing::info!("ajustes: {cmd}");
        let page = self.shell.settings.page.to_string();
        let ctx = self.settings_ctx();
        let Some(tx) = self.setctl.tx.clone() else { return };
        self.setctl.gen += 1;
        let gen = self.setctl.gen;
        std::thread::spawn(move || {
            let _ = std::process::Command::new("sh").arg("-c").arg(&cmd).stdin(std::process::Stdio::null()).status();
            let groups = crate::shell::settings_pages::build(&page, &ctx);
            let _ = tx.send((gen, page, groups));
        });
    }

    pub fn settings_act(&mut self, act: Act) {
        match act {
            Act::Nothing => {}
            Act::Page(p) => {
                let id = match p.as_str() {
                    "users" => Some("users"),
                    "s:busca" => Some("s:busca"),
                    _ => crate::shell::settings_pages::PAGES.iter().chain(crate::shell::store_pages::STORE_PAGES.iter()).find(|x| x.0 == p).map(|x| x.0),
                };
                if let Some(id) = id {
                    self.shell.settings.set_page(id);
                    self.refresh_settings_page();
                }
            }
            Act::Focus(id) => self.setctl.focus = Some(id),
            Act::Job(title, cmd) => self.start_job(title, cmd),
            Act::DisplayGrab(name) => {
                if let Some(d) = self.display_list().into_iter().find(|d| d.name == name) {
                    let p = self.last_pointer;
                    self.setctl.display_drag = Some((name.clone(), (p.x, p.y), (d.rect.loc.x, d.rect.loc.y)));
                    self.setctl.display_preview = Some((name, (d.rect.loc.x, d.rect.loc.y)));
                }
            }
            Act::Expand(k) => {
                self.setctl.expanded = if self.setctl.expanded.as_deref() == Some(k.as_str()) { None } else { Some(k) };
                self.refresh_settings_page();
            }
            Act::Set(k, v) => {
                if k == "console" {
                    crate::apps::spawn("gamescope -f -e -- steam -gamepadui");
                    return;
                }
                self.settings.set(&k, &v);
                self.settings.save();
                if k.starts_with("mode:") || k == "scale" || k == "primary" || k == "mirror" {
                    self.outputs_changed = true;
                }
                self.apply_settings();
                self.refresh_settings_page();
            }
            Act::Run(cmd) => {
                // terminais e janelas abrem sem esperar
                if cmd.starts_with("foot ") || cmd.starts_with("system-config-printer") || cmd.contains("xdg-open") {
                    crate::apps::spawn(&cmd);
                } else {
                    self.run_then_refresh(cmd);
                }
            }
            Act::Root(cmd) => self.run_then_refresh(format!("pkexec sh -c {}", q(&cmd))),
            Act::Submit { template, root } => {
                let mut cmd = template.clone();
                let ids: Vec<String> = self.setctl.fields.keys().cloned().collect();
                for id in ids {
                    let v = self.setctl.fields.get(&id).cloned().unwrap_or_default();
                    cmd = cmd.replace(&format!("{{{id}}}"), &q(&v));
                }
                // campo não preenchido vira vazio
                while let (Some(a), Some(b)) = (cmd.find('{'), cmd.find('}')) {
                    if b > a && !cmd[a..b].contains(' ') {
                        cmd.replace_range(a..=b, "''");
                    } else {
                        break;
                    }
                }
                self.setctl.fields.retain(|k, _| !(k.contains("pw") || k.contains("pass")));
                self.setctl.focus = None;
                if root {
                    self.run_then_refresh(format!("pkexec sh -c {}", q(&cmd)));
                } else {
                    self.run_then_refresh(cmd);
                }
            }
        }
        self.dirty = true;
    }

    /// Deslizante: valor 0..1 vindo da posição do mouse.
    pub fn settings_slider(&mut self, template: &str, root: bool, value: f32) {
        let v = value.clamp(0.0, 1.0);
        let pct = (v * 100.0).round() as u32;
        if self.setctl.last_slider.as_ref() == Some(&(template.to_string(), pct)) {
            return;
        }
        self.setctl.last_slider = Some((template.to_string(), pct));
        if let Some(rest) = template.strip_prefix("set:") {
            let key = rest.split(':').next().unwrap_or("");
            let val = match key {
                "kb_rate" => format!("{}", (v * 60.0).round().max(5.0)),
                "kb_delay" => format!("{}", (v * 1000.0).round().max(150.0)),
                "pointer_speed" => format!("{:.2}", v * 2.0 - 1.0),
                _ => format!("{v:.2}"),
            };
            self.settings.set(key, &val);
            self.settings.save();
            self.apply_settings();
            // atualiza o valor mostrado sem esperar a thread
            self.refresh_settings_page();
            return;
        }
        let cmd = template.replace("{v}", &pct.to_string());
        if root {
            crate::apps::spawn(&format!("pkexec sh -c {}", q(&cmd)));
        } else {
            crate::apps::spawn(&cmd);
        }
        if let Some(groups) = self.setctl.models.get_mut(self.shell.settings.page) {
            for g in groups {
                for r in &mut g.rows {
                    if let Row::Slider { template: t, value, .. } = r {
                        if t == template {
                            *value = v;
                        }
                    }
                }
            }
        }
        self.dirty = true;
    }

    /// Enter num campo: executa o primeiro botão da página que usa esse campo.
    pub fn settings_submit_focused(&mut self) {
        let Some(id) = self.setctl.focus.clone() else { return };
        let needle = format!("{{{id}}}");
        let act = self.setctl.models.get(self.shell.settings.page).and_then(|gs| {
            gs.iter().flat_map(|g| g.rows.iter()).find_map(|r| match r {
                Row::Button { act: a @ Act::Submit { template, .. }, .. } if template.contains(&needle) => Some(a.clone()),
                Row::Switch { act: a @ Act::Submit { template, .. }, .. } if template.contains(&needle) => Some(a.clone()),
                _ => None,
            })
        });
        match act {
            Some(a) => self.settings_act(a),
            None => {
                self.setctl.focus = None;
                self.refresh_settings_page();
            }
        }
    }

    /// Ajustes que mudam o próprio compositor (teclado, escala, papel de parede…).
    pub fn apply_internal_settings(&mut self) {
        // teclado: layout e repetição
        if let Some(kb) = self.seat.get_keyboard() {
            if let Some(layout) = self.settings.get("kb_layout").map(str::to_string) {
                let (l, variant) = match layout.split_once('(') {
                    Some((l, v)) => (l.to_string(), v.trim_end_matches(')').to_string()),
                    None => (layout.clone(), String::new()),
                };
                if self.applied_layout.as_deref() != Some(layout.as_str()) {
                    let cfg = smithay::input::keyboard::XkbConfig { layout: &l, variant: &variant, ..Default::default() };
                    if let Err(e) = kb.set_xkb_config(self, cfg) {
                        tracing::warn!("layout {layout}: {e:?}");
                    }
                    self.applied_layout = Some(layout);
                }
            }
            let rate: i32 = self.settings.get("kb_rate").and_then(|v| v.parse::<f32>().ok()).map(|v| v as i32).unwrap_or(30);
            let delay: i32 = self.settings.get("kb_delay").and_then(|v| v.parse::<f32>().ok()).map(|v| v as i32).unwrap_or(300);
            kb.change_repeat_info(rate, delay);
        }
        self.natural_scroll = self.settings.get("natural_scroll") == Some("true");
        self.apply_pointer_settings();
        self.stage_manager = self.settings.get("stage_manager") == Some("true");
        if let Some(d) = self.settings.get("dnd") {
            self.shell.cc.focus_on = d == "true";
        }
        if let Some(n) = self.settings.get("night") {
            self.night_shift = n == "true";
        }
        // escala da interface
        if let Some(s) = self.settings.get("scale").and_then(|v| v.parse::<f64>().ok()) {
            self.set_output_scale(s);
        }
        // papel de parede escolhido
        let wp = self.settings.get("wallpaper").map(str::to_string);
        if self.shell.wallpaper.custom != wp {
            self.shell.wallpaper.set_custom(wp);
        }
    }

    /// Aceleração, velocidade e rolagem natural nos dispositivos do libinput (sessão nativa).
    pub fn apply_pointer_settings(&mut self) {
        use smithay::reexports::input::AccelProfile;
        let accel = self.settings.get("pointer_accel") == Some("true");
        let speed: f64 = self.settings.get("pointer_speed").and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let natural = self.natural_scroll;
        for d in &mut self.input_devices {
            if d.config_accel_is_available() {
                let _ = d.config_accel_set_profile(if accel { AccelProfile::Adaptive } else { AccelProfile::Flat });
                let _ = d.config_accel_set_speed(speed.clamp(-1.0, 1.0));
            }
            if d.config_scroll_has_natural_scroll() {
                let _ = d.config_scroll_set_natural_scroll_enabled(natural);
            }
        }
    }

    /// Dados das partes com desenho próprio (foto, papéis de parede).
    pub fn settings_special(&mut self) -> crate::shell::settings_app::Special {
        if self.shell.settings.page == "wallpaper" && self.setctl.wallpapers.is_none() {
            let list = wallpaper_files().into_iter().filter_map(|p| load_image(&p, 240).map(|pm| (p, pm))).collect();
            self.setctl.wallpapers = Some(list);
        }
        if self.setctl.avatar.is_none() {
            let face = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".face");
            self.setctl.avatar = Some(load_image(&face.to_string_lossy(), 96).map(circle));
        }
        let gecos = unsafe {
            let pw = libc::getpwuid(libc::getuid());
            if pw.is_null() || (*pw).pw_gecos.is_null() {
                String::new()
            } else {
                std::ffi::CStr::from_ptr((*pw).pw_gecos).to_string_lossy().split(',').next().unwrap_or("").to_string()
            }
        };
        let gecos = crate::lock::demo().map(|d| d.1).unwrap_or(gecos);
        crate::shell::settings_app::Special {
            user: if gecos.trim().is_empty() { crate::lock::current_user() } else { gecos },
            avatar: self.setctl.avatar.clone().flatten(),
            appearance: self.settings.appearance.clone(),
            accent: self.settings.accent.clone(),
            wallpapers: if self.shell.settings.page == "wallpaper" { self.setctl.wallpapers.clone().unwrap_or_default() } else { vec![] },
            wallpaper: self.settings.get("wallpaper").unwrap_or("").to_string(),
            displays: self.display_list().into_iter().map(|d| (d.name, (d.rect.loc.x, d.rect.loc.y, d.rect.size.w, d.rect.size.h), d.primary)).collect(),
            drag: self.setctl.display_preview.clone(),
            job: self.setctl.job.clone(),
        }
    }
}

/// Recorta a foto em círculo.
fn circle(mut pm: resvg::tiny_skia::Pixmap) -> resvg::tiny_skia::Pixmap {
    let (w, h) = (pm.width() as f32, pm.height() as f32);
    let (cx, cy, r) = (w / 2.0, h / 2.0, w.min(h) / 2.0);
    let width = pm.width() as usize;
    for (i, px) in pm.pixels_mut().iter_mut().enumerate() {
        let (x, y) = ((i % width) as f32 + 0.5, (i / width) as f32 + 0.5);
        let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
        if d > r {
            *px = resvg::tiny_skia::PremultipliedColorU8::TRANSPARENT;
        }
    }
    pm
}

impl Obelisk {
    /// Arrastando uma tela na prévia: converte o deslocamento do mouse em pontos lógicos.
    pub fn display_drag_motion(&mut self, x: f64, y: f64) {
        let Some((name, (px, py), (sx, sy))) = self.setctl.display_drag.clone() else { return };
        let list: Vec<_> = self.display_list().into_iter().map(|d| (d.name, (d.rect.loc.x, d.rect.loc.y, d.rect.size.w, d.rect.size.h), d.primary)).collect();
        let (f, _) = crate::shell::settings_app::arrange_geometry(&list, 900.0 - 250.0 - 56.0);
        let nx = sx + ((x - px) as f32 / f) as i32;
        let ny = sy + ((y - py) as f32 / f) as i32;
        self.setctl.display_preview = Some((name, (nx, ny)));
        self.dirty = true;
    }

    /// Soltou a tela arrastada: encaixa e salva.
    pub fn display_drag_end(&mut self) {
        let Some((name, ..)) = self.setctl.display_drag.take() else { return };
        if let Some((_, (x, y))) = self.setctl.display_preview.take() {
            self.place_display(&name, x, y);
        }
        self.refresh_settings_page();
        self.dirty = true;
    }
}

impl Obelisk {
    /// Tarefa longa (instalar app, atualizar): roda numa thread, a última linha e o % vão para a
    /// faixa da página; ao terminar, avisa e recarrega a página.
    pub fn start_job(&mut self, title: String, cmd: String) {
        if self.setctl.job.as_ref().is_some_and(|j| j.3) {
            self.notify_local("Aguarde", "Já há uma tarefa em andamento.", false);
            return;
        }
        self.setctl.job = Some((title.clone(), "Começando…".into(), 0.0, true));
        let (tx, rx) = channel::channel::<(String, f32, Option<bool>)>();
        let _ = self.loop_handle.insert_source(rx, move |ev, _, state| {
            if let channel::Event::Msg((line, pct, done)) = ev {
                if let Some(j) = &mut state.setctl.job {
                    if !line.is_empty() {
                        j.1 = line;
                    }
                    j.2 = pct.max(j.2);
                    if let Some(ok) = done {
                        j.3 = false;
                        j.2 = if ok { 100.0 } else { j.2 };
                        let msg = if ok { "Concluído." } else { "Não terminou. Veja os detalhes nos Ajustes." };
                        let t = j.0.clone();
                        state.notify_local(&t, msg, !ok);
                        state.setctl.updates_checked = None; // reavalia as atualizações
                        state.refresh_settings_page();
                    }
                }
                state.dirty = true;
            }
        });
        std::thread::spawn(move || {
            let child = std::process::Command::new("sh")
                .arg("-c")
                .arg(format!("{cmd} 2>&1"))
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .spawn();
            let Ok(mut child) = child else {
                let _ = tx.send(("Não deu para iniciar.".into(), 0.0, Some(false)));
                return;
            };
            let mut pct = 0.0f32;
            if let Some(mut out) = child.stdout.take() {
                // flatpak/pacman reescrevem a linha com \r (sem \n): lê em blocos e separa nos dois
                use std::io::Read;
                let mut chunk = [0u8; 4096];
                let mut pending: Vec<u8> = Vec::new();
                loop {
                    let n = match out.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => n,
                    };
                    pending.extend_from_slice(&chunk[..n]);
                    while let Some(i) = pending.iter().position(|b| *b == b'\n' || *b == b'\r') {
                        let raw: Vec<u8> = pending.drain(..=i).collect();
                        let line = String::from_utf8_lossy(&raw).trim().to_string();
                        if line.is_empty() {
                            continue;
                        }
                        // "(12/80)" do pacman ou "45%" do flatpak
                        if let Some(p) = line.split_whitespace().filter(|w| w.ends_with('%')).filter_map(|w| w.trim_end_matches('%').parse::<f32>().ok()).last() {
                            pct = p.clamp(0.0, 99.0);
                        } else if let (Some(a), Some(b)) = (line.find('('), line.find(')')) {
                            if let Some((n, t)) = line.get(a + 1..b).and_then(|x| x.split_once('/')) {
                                if let (Ok(n), Ok(t)) = (n.trim().parse::<f32>(), t.trim().parse::<f32>()) {
                                    pct = (n / t.max(1.0) * 99.0).clamp(0.0, 99.0);
                                }
                            }
                        }
                        let _ = tx.send((line, pct, None));
                    }
                }
            }
            let ok = child.wait().map(|s| s.success()).unwrap_or(false);
            let _ = tx.send((String::new(), pct, Some(ok)));
        });
        self.dirty = true;
    }

    /// Verifica atualizações (pacman e Flatpak) a cada 6 h, fora do Modo Jogo; avisa quando há.
    pub fn check_updates(&mut self) {
        if self.game.active || self.greeter || crate::shell::installer::is_live() {
            return;
        }
        let due = self.setctl.updates_checked.is_none_or(|t| t.elapsed().as_secs() > 6 * 3600);
        // a primeira verificação espera 2 min depois de entrar (não pesa no início da sessão)
        if !due || self.start_time.elapsed().as_secs() < 120 && self.setctl.updates_checked.is_none() && !self.force_update_check {
            return;
        }
        self.force_update_check = false;
        self.setctl.updates_checked = Some(Instant::now());
        let (tx, rx) = channel::channel::<usize>();
        let _ = self.loop_handle.insert_source(rx, |ev, _, state| {
            if let channel::Event::Msg(n) = ev {
                let before = state.setctl.updates;
                state.setctl.updates = n;
                if n > 0 && n != before {
                    state.notify_local("Atualizações disponíveis", &format!("{n} atualizações para o sistema e os apps. Clique para ver."), false);
                }
                state.dirty = true;
            }
        });
        std::thread::spawn(move || {
            use crate::shell::settings_model::{has, sh_t};
            let sys = if has("checkupdates") { sh_t("checkupdates 2>/dev/null", 120_000).lines().filter(|l| !l.is_empty()).count() } else { 0 };
            let apps = if has("flatpak") { sh_t("flatpak remote-ls --updates --app --columns=application 2>/dev/null", 60_000).lines().filter(|l| !l.is_empty()).count() } else { 0 };
            let _ = tx.send(sys + apps);
        });
    }
}
