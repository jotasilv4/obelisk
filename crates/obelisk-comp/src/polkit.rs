//! Agente de autenticação do polkit (como o do kyro): quando um app pede privilégio (pkexec,
//! fuso horário, atualizações…), o pedido aparece no notch e a senha é validada pelo helper do
//! polkit. Polkit ≥ 127 usa o socket `/run/polkit/agent-helper.socket`; versões antigas, o binário
//! `polkit-agent-helper-1`.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc;
use std::time::Duration;

use smithay::reexports::calloop::{channel, LoopHandle};
use zbus::zvariant::{OwnedValue, Value};

use crate::state::Obelisk;

const AGENT_PATH: &str = "/org/obelisk/PolicyKit1/AuthenticationAgent";

/// Pedido em andamento: mensagem para mostrar e onde devolver a senha (`None` = cancelado).
pub struct AuthRequest {
    pub message: String,
    pub action: String,
    pub reply: mpsc::Sender<Option<String>>,
}

pub enum PolkitEvent {
    Begin(AuthRequest),
    Done,
}

struct Agent {
    tx: channel::Sender<PolkitEvent>,
    user: String,
}

/// Conversa com o helper: manda usuário/cookie, responde às perguntas com a senha.
fn run_helper(user: &str, cookie: &str, password: &str) -> bool {
    fn converse<R: BufRead, W: Write>(mut r: R, mut w: W, password: &str) -> bool {
        let mut line = String::new();
        loop {
            line.clear();
            if r.read_line(&mut line).unwrap_or(0) == 0 {
                return false;
            }
            let l = line.trim_end();
            if l.starts_with("PAM_PROMPT_ECHO_OFF") || l.starts_with("PAM_PROMPT_ECHO_ON") {
                if writeln!(w, "{password}").and_then(|_| w.flush()).is_err() {
                    return false;
                }
            } else if l == "SUCCESS" {
                return true;
            } else if l == "FAILURE" {
                return false;
            }
        }
    }
    // polkit ≥ 127: socket ativado pelo systemd
    if let Ok(sock) = std::os::unix::net::UnixStream::connect("/run/polkit/agent-helper.socket") {
        let _ = sock.set_read_timeout(Some(Duration::from_secs(30)));
        let mut w = match sock.try_clone() {
            Ok(w) => w,
            Err(_) => return false,
        };
        if writeln!(w, "{user}").and_then(|_| writeln!(w, "{cookie}")).and_then(|_| w.flush()).is_err() {
            return false;
        }
        return converse(BufReader::new(sock), w, password);
    }
    // versões antigas: binário setuid
    for helper in ["/usr/lib/polkit-1/polkit-agent-helper-1", "/usr/libexec/polkit-agent-helper-1"] {
        if let Ok(mut child) = std::process::Command::new(helper)
            .arg(user)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            let mut stdin = child.stdin.take().unwrap();
            let _ = writeln!(stdin, "{cookie}");
            let ok = converse(BufReader::new(child.stdout.take().unwrap()), stdin, password);
            let _ = child.wait();
            return ok;
        }
    }
    false
}

#[zbus::interface(name = "org.freedesktop.PolicyKit1.AuthenticationAgent")]
impl Agent {
    /// Bloqueia até o usuário responder (roda na thread do zbus deste barramento).
    #[allow(clippy::too_many_arguments)]
    fn begin_authentication(
        &self,
        action_id: String,
        message: String,
        _icon_name: String,
        _details: HashMap<String, String>,
        cookie: String,
        _identities: Vec<(String, HashMap<String, OwnedValue>)>,
    ) -> zbus::fdo::Result<()> {
        for _attempt in 0..3 {
            let (reply, rx) = mpsc::channel();
            let _ = self.tx.send(PolkitEvent::Begin(AuthRequest { message: message.clone(), action: action_id.clone(), reply }));
            match rx.recv_timeout(Duration::from_secs(120)) {
                Ok(Some(pw)) => {
                    if run_helper(&self.user, &cookie, &pw) {
                        let _ = self.tx.send(PolkitEvent::Done);
                        return Ok(());
                    }
                }
                _ => {
                    let _ = self.tx.send(PolkitEvent::Done);
                    return Err(zbus::fdo::Error::Failed("cancelado".into()));
                }
            }
        }
        let _ = self.tx.send(PolkitEvent::Done);
        Err(zbus::fdo::Error::Failed("senha incorreta".into()))
    }

    fn cancel_authentication(&self, _cookie: String) {
        let _ = self.tx.send(PolkitEvent::Done);
    }
}

/// Registra o agente para a sessão atual. Se já houver outro agente (ex.: rodando aninhado
/// dentro de outro desktop), o Obelisk não o substitui.
pub fn start(handle: &LoopHandle<'static, Obelisk>) -> Option<zbus::blocking::Connection> {
    let session = std::env::var("XDG_SESSION_ID").ok()?;
    let (tx, rx) = channel::channel::<PolkitEvent>();
    handle
        .insert_source(rx, |ev, _, state| {
            if let channel::Event::Msg(ev) = ev {
                state.on_polkit_event(ev);
            }
        })
        .ok()?;
    let agent = Agent { tx, user: crate::lock::current_user() };
    let conn = zbus::blocking::connection::Builder::system().and_then(|b| b.serve_at(AGENT_PATH, agent)).and_then(|b| b.build()).ok()?;
    let mut details: HashMap<&str, Value> = HashMap::new();
    details.insert("session-id", Value::from(session.as_str()));
    let subject = ("unix-session", details);
    let locale = std::env::var("LANG").unwrap_or_else(|_| "pt_BR.UTF-8".into());
    let res = conn.call_method(
        Some("org.freedesktop.PolicyKit1"),
        "/org/freedesktop/PolicyKit1/Authority",
        Some("org.freedesktop.PolicyKit1.Authority"),
        "RegisterAuthenticationAgent",
        &(subject, locale.as_str(), AGENT_PATH),
    );
    match res {
        Ok(_) => {
            tracing::info!("agente de autenticação (polkit) registrado");
            Some(conn)
        }
        Err(e) => {
            tracing::warn!("agente polkit não registrado (outro agente ativo?): {e}");
            None
        }
    }
}
