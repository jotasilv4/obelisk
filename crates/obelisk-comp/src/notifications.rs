//! Servidor de notificações (org.freedesktop.Notifications, spec 1.2) no D-Bus da sessão.
//! As notificações aparecem no notch (estilo Dynamic Island) e ficam na Central de Notificações.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

use smithay::reexports::calloop::{channel, LoopHandle};
use zbus::zvariant::OwnedValue;

use crate::state::Obelisk;

#[derive(Clone, Debug)]
pub struct Notif {
    pub id: u32,
    pub app: String,
    pub summary: String,
    pub body: String,
    pub icon: String,
    /// (chave, rótulo)
    pub actions: Vec<(String, String)>,
    pub critical: bool,
    pub timeout_ms: i32,
    pub at: Instant,
}

pub enum NotifEvent {
    New(Notif),
    Close(u32),
}

struct Server {
    tx: channel::Sender<NotifEvent>,
    next: Arc<AtomicU32>,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Server {
    fn get_capabilities(&self) -> Vec<String> {
        vec!["body".into(), "actions".into(), "persistence".into()]
    }

    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> u32 {
        let id = if replaces_id != 0 { replaces_id } else { self.next.fetch_add(1, Ordering::Relaxed) };
        let critical = hints.get("urgency").and_then(|v| u8::try_from(v).ok()).is_some_and(|u| u >= 2);
        let pairs = actions.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0].clone(), c[1].clone())).collect();
        let _ = self.tx.send(NotifEvent::New(Notif {
            id,
            app: app_name,
            summary,
            body: strip_markup(&body),
            icon: app_icon,
            actions: pairs,
            critical,
            timeout_ms: expire_timeout,
            at: Instant::now(),
        }));
        id
    }

    fn close_notification(&self, id: u32) {
        let _ = self.tx.send(NotifEvent::Close(id));
    }

    fn get_server_information(&self) -> (String, String, String, String) {
        ("Obelisk".into(), "Obelisk".into(), env!("CARGO_PKG_VERSION").into(), "1.2".into())
    }
}

/// Corpo pode trazer marcação simples (<b>, <i>, <a>): mostramos só o texto.
fn strip_markup(s: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for c in s.chars() {
        match c {
            '<' => tag = true,
            '>' => tag = false,
            c if !tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"")
}

/// Registra o serviço. Se outro servidor já ocupa o nome, o Obelisk segue sem notificações.
pub fn start(handle: &LoopHandle<'static, Obelisk>) -> Option<zbus::blocking::Connection> {
    let (tx, rx) = channel::channel::<NotifEvent>();
    handle
        .insert_source(rx, |ev, _, state| {
            if let channel::Event::Msg(ev) = ev {
                state.on_notif_event(ev);
            }
        })
        .ok()?;
    let server = Server { tx, next: Arc::new(AtomicU32::new(1)) };
    let conn = zbus::blocking::connection::Builder::session()
        .and_then(|b| b.serve_at("/org/freedesktop/Notifications", server))
        .and_then(|b| b.build());
    let conn = match conn {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("notificações: sem D-Bus da sessão ({e})");
            return None;
        }
    };
    // não toma o nome de outro servidor que já esteja rodando
    use zbus::fdo::RequestNameFlags;
    match conn.request_name_with_flags("org.freedesktop.Notifications", RequestNameFlags::DoNotQueue.into()) {
        Ok(zbus::fdo::RequestNameReply::PrimaryOwner) | Ok(zbus::fdo::RequestNameReply::AlreadyOwner) => {
            tracing::info!("servidor de notificações ativo");
            Some(conn)
        }
        other => {
            tracing::warn!("outro servidor de notificações já está ativo ({other:?}); o Obelisk não vai substituí-lo");
            None
        }
    }
}

pub fn emit_closed(conn: &zbus::blocking::Connection, id: u32, reason: u32) {
    let _ = conn.emit_signal(None::<&str>, "/org/freedesktop/Notifications", "org.freedesktop.Notifications", "NotificationClosed", &(id, reason));
}

pub fn emit_action(conn: &zbus::blocking::Connection, id: u32, key: &str) {
    let _ = conn.emit_signal(None::<&str>, "/org/freedesktop/Notifications", "org.freedesktop.Notifications", "ActionInvoked", &(id, key));
}
