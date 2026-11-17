//! Tela de bloqueio (docs/03-especificacao-kyro.md §13) com autenticação PAM.
//! A verificação roda numa thread: senha errada demora (atraso do PAM) sem travar o compositor.
//! Serviço PAM: `obelisk-lock` (instalado pela ISO); sem ele, usa `login`.

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::time::Instant;

use resvg::tiny_skia::Pixmap;
use smithay::reexports::calloop::{channel, LoopHandle};

use crate::shell::layer::Layer;
use crate::state::Obelisk;
use crate::ui::canvas::Canvas;
use crate::ui::text::Weight;
use crate::ui::text_glyph;
use crate::ui::theme::Rgba;

// ---------------------------------------------------------------- PAM (FFI mínima)

#[repr(C)]
struct PamMessage {
    msg_style: c_int,
    msg: *const c_char,
}

#[repr(C)]
struct PamResponse {
    resp: *mut c_char,
    resp_retcode: c_int,
}

#[repr(C)]
struct PamConv {
    conv: extern "C" fn(c_int, *mut *const PamMessage, *mut *mut PamResponse, *mut c_void) -> c_int,
    appdata_ptr: *mut c_void,
}

#[link(name = "pam")]
extern "C" {
    fn pam_start(service: *const c_char, user: *const c_char, conv: *const PamConv, pamh: *mut *mut c_void) -> c_int;
    fn pam_authenticate(pamh: *mut c_void, flags: c_int) -> c_int;
    fn pam_acct_mgmt(pamh: *mut c_void, flags: c_int) -> c_int;
    fn pam_end(pamh: *mut c_void, status: c_int) -> c_int;
}

const PAM_SUCCESS: c_int = 0;
const PAM_BUF_ERR: c_int = 5;
const PAM_PROMPT_ECHO_OFF: c_int = 1;
const PAM_PROMPT_ECHO_ON: c_int = 2;

extern "C" fn conversation(n: c_int, msgs: *mut *const PamMessage, resp: *mut *mut PamResponse, data: *mut c_void) -> c_int {
    unsafe {
        let out = libc::calloc(n as usize, std::mem::size_of::<PamResponse>()) as *mut PamResponse;
        if out.is_null() {
            return PAM_BUF_ERR;
        }
        let password = &*(data as *const CString);
        for i in 0..n as usize {
            let m = &**msgs.add(i);
            if m.msg_style == PAM_PROMPT_ECHO_OFF || m.msg_style == PAM_PROMPT_ECHO_ON {
                (*out.add(i)).resp = libc::strdup(password.as_ptr());
            }
        }
        *resp = out;
    }
    PAM_SUCCESS
}

/// Verifica a senha de `user` pelo PAM (bloqueante).
pub fn authenticate(user: &str, password: &str) -> bool {
    let service = if std::path::Path::new("/etc/pam.d/obelisk-lock").exists() { "obelisk-lock" } else { "login" };
    let (Ok(svc), Ok(usr), Ok(pw)) = (CString::new(service), CString::new(user), CString::new(password)) else {
        return false;
    };
    let pw = Box::new(pw);
    let conv = PamConv { conv: conversation, appdata_ptr: &*pw as *const CString as *mut c_void };
    let mut h: *mut c_void = std::ptr::null_mut();
    unsafe {
        if pam_start(svc.as_ptr(), usr.as_ptr(), &conv, &mut h) != PAM_SUCCESS {
            return false;
        }
        let mut r = pam_authenticate(h, 0);
        if r == PAM_SUCCESS {
            r = pam_acct_mgmt(h, 0);
        }
        pam_end(h, r);
        r == PAM_SUCCESS
    }
}

pub fn current_user() -> String {
    std::env::var("USER").ok().filter(|u| !u.is_empty()).unwrap_or_else(|| unsafe {
        let pw = libc::getpwuid(libc::getuid());
        if pw.is_null() {
            "usuário".into()
        } else {
            CStr::from_ptr((*pw).pw_name).to_string_lossy().into_owned()
        }
    })
}

/// Nome completo (campo GECOS) ou o login.
fn display_name(user: &str) -> String {
    unsafe {
        let pw = libc::getpwuid(libc::getuid());
        if !pw.is_null() && !(*pw).pw_gecos.is_null() {
            let g = CStr::from_ptr((*pw).pw_gecos).to_string_lossy();
            let n = g.split(',').next().unwrap_or("").trim().to_string();
            if !n.is_empty() {
                return n;
            }
        }
    }
    user.to_string()
}

// ---------------------------------------------------------------- tela de login (greetd)

/// Usuários que podem entrar: uid 1000–59999 com shell de login.
pub fn login_users() -> Vec<(String, String)> {
    std::fs::read_to_string("/etc/passwd")
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split(':').collect();
            let uid: u32 = f.get(2)?.parse().ok()?;
            let shell = f.get(6).copied().unwrap_or("");
            if !(1000..60000).contains(&uid) || shell.ends_with("nologin") || shell.ends_with("false") {
                return None;
            }
            let full = f.get(4).and_then(|g| g.split(',').next()).map(str::trim).filter(|n| !n.is_empty()).unwrap_or(f[0]);
            Some((f[0].to_string(), full.to_string()))
        })
        .collect()
}

fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// Uma ida e volta no protocolo do greetd: tamanho (u32 nativo) + JSON.
fn greetd_call(sock: &mut std::os::unix::net::UnixStream, msg: &str) -> std::io::Result<String> {
    use std::io::{Read, Write};
    sock.write_all(&(msg.len() as u32).to_ne_bytes())?;
    sock.write_all(msg.as_bytes())?;
    let mut len = [0u8; 4];
    sock.read_exact(&mut len)?;
    let mut buf = vec![0u8; u32::from_ne_bytes(len) as usize];
    sock.read_exact(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn json_field<'a>(resp: &'a str, key: &str) -> Option<&'a str> {
    let k = format!("\"{key}\":\"");
    let i = resp.find(&k)? + k.len();
    resp[i..].split('"').next()
}

/// Entra pelo greetd: cria a sessão, responde à senha e pede para iniciar `obelisk-session`.
/// Em caso de erro, cancela (o greetd volta a esperar).
pub fn greetd_login(user: &str, password: &str) -> bool {
    let Ok(path) = std::env::var("GREETD_SOCK") else {
        tracing::warn!("GREETD_SOCK ausente: não é o greeter do greetd");
        return false;
    };
    let Ok(mut sock) = std::os::unix::net::UnixStream::connect(path) else { return false };
    let mut resp = match greetd_call(&mut sock, &format!("{{\"type\":\"create_session\",\"username\":{}}}", json_str(user))) {
        Ok(r) => r,
        Err(_) => return false,
    };
    // responde às perguntas do PAM (normalmente só a senha)
    for _ in 0..4 {
        match json_field(&resp, "type") {
            Some("success") => break,
            Some("auth_message") => {
                let reply = match json_field(&resp, "auth_message_type") {
                    Some("secret") | Some("visible") => format!("{{\"type\":\"post_auth_message_response\",\"response\":{}}}", json_str(password)),
                    _ => "{\"type\":\"post_auth_message_response\"}".to_string(),
                };
                resp = match greetd_call(&mut sock, &reply) {
                    Ok(r) => r,
                    Err(_) => return false,
                };
            }
            _ => {
                let _ = greetd_call(&mut sock, "{\"type\":\"cancel_session\"}");
                return false;
            }
        }
    }
    if json_field(&resp, "type") != Some("success") {
        let _ = greetd_call(&mut sock, "{\"type\":\"cancel_session\"}");
        return false;
    }
    matches!(greetd_call(&mut sock, "{\"type\":\"start_session\",\"cmd\":[\"obelisk-session\"],\"env\":[]}").as_deref().map(|r| json_field(r, "type")), Ok(Some("success")))
}

// ---------------------------------------------------------------- estado e UI

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LockState {
    Typing,
    Checking,
    Wrong,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LockHit {
    Sleep,
    Restart,
    Shutdown,
    Submit,
    PrevUser,
    NextUser,
}

pub struct Lock {
    pub active: bool,
    pub password: String,
    pub state: LockState,
    pub shake: Option<Instant>,
    pub layer: Layer,
    user: String,
    name: String,
    hits: Vec<(LockHit, (f32, f32, f32, f32))>,
    key: Option<String>,
    tx: Option<channel::Sender<bool>>,
    /// tela de login: usuários (login, nome) e o escolhido; `None` = bloqueio comum
    pub greeter: Option<(Vec<(String, String)>, usize)>,
}

impl Lock {
    pub fn new() -> Self {
        let user = current_user();
        let name = display_name(&user);
        Lock { active: false, password: String::new(), state: LockState::Typing, shake: None, layer: Layer::default(), user, name, hits: vec![], key: None, tx: None, greeter: None }
    }

    /// Liga o canal de resposta da thread de autenticação ao loop.
    pub fn attach(&mut self, handle: &LoopHandle<'static, Obelisk>) {
        let (tx, rx) = channel::channel::<bool>();
        let _ = handle.insert_source(rx, |ev, _, state| {
            if let channel::Event::Msg(ok) = ev {
                state.on_auth_result(ok);
            }
        });
        self.tx = Some(tx);
    }

    pub fn submit(&mut self) {
        if self.state == LockState::Checking || self.password.is_empty() {
            return;
        }
        self.state = LockState::Checking;
        let (user, pw) = (self.user.clone(), std::mem::take(&mut self.password));
        let tx = self.tx.clone();
        let greeter = self.greeter.is_some();
        std::thread::spawn(move || {
            let ok = if greeter { greetd_login(&user, &pw) } else { authenticate(&user, &pw) };
            if let Some(tx) = tx {
                let _ = tx.send(ok);
            }
        });
    }

    /// Tela de login: começa no usuário de `last` (o último que entrou) ou no primeiro.
    pub fn start_greeter(&mut self, last: Option<String>) {
        let users = login_users();
        let i = last.and_then(|l| users.iter().position(|u| u.0 == l)).unwrap_or(0);
        if let Some((login, name)) = users.get(i) {
            self.user = login.clone();
            self.name = name.clone();
        }
        self.greeter = Some((users, i));
        self.active = true;
        self.key = None;
    }

    /// Troca o usuário na tela de login (←/→ ou as setas ao lado do nome).
    pub fn step_user(&mut self, d: i32) {
        let Some((users, i)) = &mut self.greeter else { return };
        if users.len() < 2 {
            return;
        }
        *i = (*i as i32 + d).rem_euclid(users.len() as i32) as usize;
        self.user = users[*i].0.clone();
        self.name = users[*i].1.clone();
        self.password.clear();
        self.state = LockState::Typing;
        self.key = None;
    }

    pub fn hit(&self, x: f64, y: f64) -> Option<LockHit> {
        let (x, y) = (x as f32, y as f32);
        self.hits.iter().find(|(_, r)| x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3).map(|(h, _)| *h)
    }

    pub fn animating(&self) -> bool {
        self.shake.is_some_and(|t| t.elapsed().as_millis() < 300)
    }

    /// Tremida da senha errada: −12, +12, −8, +6, 0 (290 ms).
    fn shake_x(&self) -> f32 {
        let Some(t) = self.shake else { return 0.0 };
        let ms = t.elapsed().as_millis() as f32;
        let keys = [(0.0, 0.0), (50.0, -12.0), (120.0, 12.0), (180.0, -8.0), (240.0, 6.0), (290.0, 0.0)];
        for w in keys.windows(2) {
            let ((t0, v0), (t1, v1)) = (w[0], w[1]);
            if ms <= t1 {
                return v0 + (v1 - v0) * ((ms - t0) / (t1 - t0)).clamp(0.0, 1.0);
            }
        }
        0.0
    }

    pub fn update(&mut self, sw: i32, sh: i32, scale: i32, backdrop: Option<&Pixmap>, power_enabled: bool) {
        use chrono::{Datelike, Timelike};
        const DIAS: [&str; 7] = ["Segunda-feira", "Terça-feira", "Quarta-feira", "Quinta-feira", "Sexta-feira", "Sábado", "Domingo"];
        const MESES: [&str; 12] = ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"];
        let now = chrono::Local::now();
        let shake = self.shake_x();
        let key = format!("{}|{:?}|{:.1}|{}|{}|{}|{}|{}", self.password.chars().count(), self.state, shake, now.format("%H:%M"), sw, sh, scale, self.user);
        if self.key.as_deref() == Some(key.as_str()) && self.layer.is_set() {
            return;
        }
        self.key = Some(key);
        let (w, h) = (sw as f32, sh as f32);
        let mut c = Canvas::new(sw, sh, scale as f32).unwrap();
        match backdrop {
            Some(bd) => c.fill_rrect_backdrop(0.0, 0.0, w, h, 0.0, bd, (0.0, 0.0), (w, h)),
            None => c.fill_rrect(0.0, 0.0, w, h, 0.0, Rgba(11, 15, 31, 255)),
        }
        c.fill_rrect(0.0, 0.0, w, h, 0.0, Rgba(0, 0, 0, 31)); // brilho −12 %
        let white = |o: f32| Rgba(255, 255, 255, (255.0 * o) as u8);

        // data e hora
        let date = format!("{}, {} de {}", DIAS[now.weekday().num_days_from_monday() as usize], now.day(), MESES[now.month0() as usize]);
        let dw = c.measure(&date, 22.0, Weight::Semibold);
        let top = h * 0.10;
        c.text(&date, (w - dw) / 2.0, top, 22.0, Weight::Semibold, white(0.85));
        let time = format!("{}:{:02}", now.hour(), now.minute());
        let tw = c.measure(&time, 112.0, Weight::Bold);
        c.text(&time, (w - tw) / 2.0, top + 30.0, 112.0, Weight::Bold, white(0.82));

        // avatar, nome e senha (de baixo para cima, margem 10 %)
        self.hits.clear();
        let field_y = h - h * 0.10 - 32.0;
        let name_y = field_y - 12.0 - 20.0;
        let av = 64.0;
        let av_y = name_y - 12.0 - av;
        c.fill_circle(w / 2.0, av_y + av / 2.0, av / 2.0, Rgba(0x0b, 0x0f, 0x1f, 255));
        text_glyph::obelisk_logo(&mut c, w / 2.0, av_y + av / 2.0, av * 0.56, Rgba(0xee, 0xf1, 0xff, 255));
        let nw = c.measure(&self.name, 15.0, Weight::Semibold);
        c.text(&self.name, (w - nw) / 2.0, name_y, 15.0, Weight::Semibold, white(1.0));
        // login com mais de um usuário: setas para trocar
        if self.greeter.as_ref().is_some_and(|g| g.0.len() > 1) {
            let cy = av_y + av / 2.0;
            for (dx, sym, hit) in [(-av, "chevron_left", LockHit::PrevUser), (av, "chevron_right", LockHit::NextUser)] {
                let cx = w / 2.0 + dx;
                c.fill_circle(cx, cy, 16.0, white(0.14));
                c.symbol(sym, cx, cy, 22.0, white(0.9));
                self.hits.push((hit, (cx - 18.0, cy - 18.0, 36.0, 36.0)));
            }
        }

        let (fw, fh) = (220.0, 32.0);
        let fx = (w - fw) / 2.0 + shake;
        c.fill_rrect(fx, field_y, fw, fh, 16.0, white(0.18));
        c.stroke_rrect(fx, field_y, fw, fh, 16.0, 1.0, white(0.20));
        let n = self.password.chars().count();
        if n == 0 || self.state != LockState::Typing {
            let (t, col) = match self.state {
                LockState::Checking => ("Verificando…", white(0.55)),
                LockState::Wrong if n == 0 => ("Senha incorreta", Rgba(0xff, 0xb4, 0xb0, 255)),
                _ => ("Digite a senha", white(0.55)),
            };
            if n == 0 {
                c.text_vcenter(t, fx + 14.0, field_y, fh, 13.0, Weight::Regular, col);
            }
        }
        if n > 0 {
            let dots: String = "●".repeat(n.min(24));
            c.text_vcenter(&dots, fx + 14.0, field_y, fh, 10.0, Weight::Regular, white(1.0));
        }
        let (bx, by) = (fx + fw - 4.0 - 24.0, field_y + 4.0);
        c.fill_circle(bx + 12.0, by + 12.0, 12.0, white(if n > 0 { 0.30 } else { 0.12 }));
        c.symbol("arrow_forward", bx + 12.0, by + 12.0, 16.0, white(1.0));
        self.hits.push((LockHit::Submit, (bx, by, 24.0, 24.0)));

        // energia (canto inferior direito)
        let items = [("Repouso", "bedtime", LockHit::Sleep), ("Reiniciar", "restart_alt", LockHit::Restart), ("Desligar", "power_settings_new", LockHit::Shutdown)];
        let mut px = w - 24.0 - (items.len() as f32 * 40.0 + (items.len() as f32 - 1.0) * 18.0);
        let py = h - 24.0 - 40.0 - 18.0;
        for (label, sym, hit) in items {
            let a = if power_enabled { 1.0 } else { 0.35 };
            c.fill_circle(px + 20.0, py + 20.0, 20.0, white(0.16 * a));
            c.symbol(sym, px + 20.0, py + 20.0, 20.0, white(a));
            let lw = c.measure(label, 11.0, Weight::Regular);
            c.text(label, px + 20.0 - lw / 2.0, py + 44.0, 11.0, Weight::Regular, white(0.75 * a));
            if power_enabled {
                self.hits.push((hit, (px, py, 40.0, 40.0)));
            }
            px += 58.0;
        }
        self.layer.set(&c);
    }
}
