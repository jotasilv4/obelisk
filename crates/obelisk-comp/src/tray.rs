//! Bandeja de status (StatusNotifierItem + dbusmenu), como os ícones de apps na barra do macOS.
//! O Obelisk é o `org.kde.StatusNotifierWatcher`; cada app registra seu item, o ícone vai para a
//! barra e o menu do app (dbusmenu) abre no estilo NSMenu. Chamadas D-Bus rodam em threads.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use resvg::tiny_skia::{Pixmap, PremultipliedColorU8};
use smithay::reexports::calloop::{channel, LoopHandle};
use zbus::zvariant::{OwnedValue, Value};

use crate::state::Obelisk;

#[derive(Clone)]
pub struct TrayItem {
    pub bus: String,
    pub path: String,
    pub title: String,
    pub icon: Option<Pixmap>,
    pub icon_name: String,
    pub menu: Option<String>,
    pub item_is_menu: bool,
}

#[derive(Clone, Debug)]
pub struct MenuEntry {
    pub id: i32,
    pub label: String,
    pub enabled: bool,
    pub separator: bool,
    pub checked: Option<bool>,
}

pub enum TrayEvent {
    Registered(String, String),
    Updated(TrayItem),
    Removed(String),
    Menu(String, Vec<MenuEntry>),
}

struct Watcher {
    tx: channel::Sender<TrayEvent>,
    items: Arc<Mutex<Vec<String>>>,
}

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    fn register_status_notifier_item(&self, service: &str, #[zbus(header)] hdr: zbus::message::Header<'_>) {
        let sender = hdr.sender().map(|s| s.to_string()).unwrap_or_default();
        // o serviço pode ser um nome no barramento ou um caminho de objeto
        let (bus, path) = if service.starts_with('/') { (sender, service.to_string()) } else { (service.to_string(), "/StatusNotifierItem".to_string()) };
        self.items.lock().unwrap().push(format!("{bus}{path}"));
        let _ = self.tx.send(TrayEvent::Registered(bus, path));
    }

    fn register_status_notifier_host(&self, _service: &str) {}

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        self.items.lock().unwrap().clone()
    }

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }
}

pub struct Tray {
    pub conn: Option<zbus::blocking::Connection>,
    tx: Option<channel::Sender<TrayEvent>>,
}

/// Registra o watcher. Se outro painel já é o watcher, o Obelisk segue sem bandeja.
pub fn start(handle: &LoopHandle<'static, Obelisk>) -> Tray {
    let (tx, rx) = channel::channel::<TrayEvent>();
    let _ = handle.insert_source(rx, |ev, _, state| {
        if let channel::Event::Msg(ev) = ev {
            state.on_tray_event(ev);
        }
    });
    let watcher = Watcher { tx: tx.clone(), items: Arc::new(Mutex::new(Vec::new())) };
    let conn = zbus::blocking::connection::Builder::session()
        .and_then(|b| b.serve_at("/StatusNotifierWatcher", watcher))
        .and_then(|b| b.build())
        .ok()
        .and_then(|c| {
            use zbus::fdo::{RequestNameFlags, RequestNameReply};
            match c.request_name_with_flags("org.kde.StatusNotifierWatcher", RequestNameFlags::DoNotQueue.into()) {
                Ok(RequestNameReply::PrimaryOwner) | Ok(RequestNameReply::AlreadyOwner) => Some(c),
                _ => {
                    tracing::warn!("bandeja: outro painel já é o StatusNotifierWatcher");
                    None
                }
            }
        });
    if conn.is_some() {
        tracing::info!("bandeja de status ativa");
    }
    Tray { conn, tx: Some(tx) }
}

fn pixmap_from_argb(list: Vec<(i32, i32, Vec<u8>)>) -> Option<Pixmap> {
    let (w, h, data) = list.into_iter().filter(|(w, h, d)| *w > 0 && *h > 0 && d.len() >= (*w * *h * 4) as usize).max_by_key(|(w, _, _)| *w)?;
    let mut pm = Pixmap::new(w as u32, h as u32)?;
    for (px, s) in pm.pixels_mut().iter_mut().zip(data.chunks_exact(4)) {
        // ARGB32 em ordem de rede (big-endian)
        let (a, r, g, b) = (s[0] as u16, s[1] as u16, s[2] as u16, s[3] as u16);
        if let Some(c) = PremultipliedColorU8::from_rgba((r * a / 255) as u8, (g * a / 255) as u8, (b * a / 255) as u8, a as u8) {
            *px = c;
        }
    }
    Some(pm)
}

fn fetch(conn: &zbus::blocking::Connection, bus: &str, path: &str) -> Option<TrayItem> {
    let p = zbus::blocking::Proxy::new(conn, bus.to_string(), path.to_string(), "org.kde.StatusNotifierItem").ok()?;
    let icon_name: String = p.get_property("IconName").unwrap_or_default();
    let theme_path: String = p.get_property("IconThemePath").unwrap_or_default();
    let title: String = p.get_property("Title").or_else(|_| p.get_property("Id")).unwrap_or_default();
    let menu: Option<String> = p.get_property::<zbus::zvariant::OwnedObjectPath>("Menu").ok().map(|m| m.to_string()).filter(|m| m != "/");
    let item_is_menu: bool = p.get_property("ItemIsMenu").unwrap_or(false);
    let mut icon = None;
    if !icon_name.is_empty() {
        let candidates = [format!("{theme_path}/{icon_name}.png"), format!("{theme_path}/{icon_name}.svg")];
        for c in candidates.iter().filter(|_| !theme_path.is_empty()) {
            let path = std::path::Path::new(c);
            if path.is_file() {
                icon = if c.ends_with(".svg") { crate::ui::icons::render_svg(path, 64) } else { crate::ui::icons::load_png(path) };
                break;
            }
        }
    }
    if icon.is_none() && icon_name.is_empty() {
        icon = p.get_property::<Vec<(i32, i32, Vec<u8>)>>("IconPixmap").ok().and_then(pixmap_from_argb);
    }
    Some(TrayItem { bus: bus.into(), path: path.into(), title, icon, icon_name, menu, item_is_menu })
}

fn val_str(v: &Value) -> Option<String> {
    match v {
        Value::Str(s) => Some(s.to_string()),
        Value::Value(b) => val_str(b),
        _ => None,
    }
}
fn val_bool(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::Value(b) => val_bool(b),
        _ => None,
    }
}
fn val_i32(v: &Value) -> Option<i32> {
    match v {
        Value::I32(i) => Some(*i),
        Value::Value(b) => val_i32(b),
        _ => None,
    }
}

/// Lê o primeiro nível do menu dbusmenu (submenus aparecem como itens com "›").
fn fetch_menu(conn: &zbus::blocking::Connection, bus: &str, path: &str) -> Vec<MenuEntry> {
    let Ok(p) = zbus::blocking::Proxy::new(conn, bus.to_string(), path.to_string(), "com.canonical.dbusmenu") else { return vec![] };
    let _: Result<bool, _> = p.call("AboutToShow", &(0i32,));
    type Layout = (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);
    let reply: Result<(u32, Layout), _> = p.call("GetLayout", &(0i32, 1i32, Vec::<String>::new()));
    let Ok((_, (_, _, children))) = reply else { return vec![] };
    let mut out = Vec::new();
    for child in children {
        let v: &Value = &child;
        let v = if let Value::Value(inner) = v { inner.as_ref() } else { v };
        let Value::Structure(st) = v else { continue };
        let f = st.fields();
        let Some(id) = f.first().and_then(val_i32) else { continue };
        let Some(Value::Dict(dict)) = f.get(1) else { continue };
        let Ok(dict) = dict.try_clone() else { continue };
        let Ok(props) = HashMap::<String, OwnedValue>::try_from(dict) else { continue };
        let get = |k: &str| props.get(k).map(|v| Value::from(v.clone()));
        let visible = get("visible").as_ref().and_then(val_bool).unwrap_or(true);
        if !visible {
            continue;
        }
        let separator = get("type").as_ref().and_then(val_str).is_some_and(|t| t == "separator");
        let mut label = get("label").as_ref().and_then(val_str).unwrap_or_default().replace("__", "\u{1}").replace('_', "").replace('\u{1}', "_");
        if get("children-display").as_ref().and_then(val_str).is_some_and(|t| t == "submenu") {
            label.push_str("  ›");
        }
        let enabled = get("enabled").as_ref().and_then(val_bool).unwrap_or(true);
        let checked = get("toggle-type").as_ref().and_then(val_str).filter(|t| !t.is_empty()).map(|_| get("toggle-state").as_ref().and_then(val_i32).unwrap_or(0) == 1);
        out.push(MenuEntry { id, label, enabled, separator, checked });
    }
    out
}

impl Tray {
    pub fn refresh(&self, bus: String, path: String) {
        let (Some(conn), Some(tx)) = (self.conn.clone(), self.tx.clone()) else { return };
        std::thread::spawn(move || match fetch(&conn, &bus, &path) {
            Some(item) => {
                let _ = tx.send(TrayEvent::Updated(item));
            }
            None => {
                let _ = tx.send(TrayEvent::Removed(format!("{bus}{path}")));
            }
        });
    }

    /// Clique: Activate (esquerdo) ou SecondaryActivate (meio), nas coordenadas do clique.
    pub fn activate(&self, item: &TrayItem, method: &'static str, x: i32, y: i32) {
        let Some(conn) = self.conn.clone() else { return };
        let (bus, path) = (item.bus.clone(), item.path.clone());
        std::thread::spawn(move || {
            if let Ok(p) = zbus::blocking::Proxy::new(&conn, bus, path, "org.kde.StatusNotifierItem") {
                let _: Result<(), _> = p.call(method, &(x, y));
            }
        });
    }

    pub fn open_menu(&self, item: &TrayItem) {
        let (Some(conn), Some(tx), Some(menu)) = (self.conn.clone(), self.tx.clone(), item.menu.clone()) else { return };
        let (bus, key) = (item.bus.clone(), format!("{}{}", item.bus, item.path));
        std::thread::spawn(move || {
            let entries = fetch_menu(&conn, &bus, &menu);
            let _ = tx.send(TrayEvent::Menu(key, entries));
        });
    }

    pub fn menu_click(&self, item: &TrayItem, id: i32) {
        let (Some(conn), Some(menu)) = (self.conn.clone(), item.menu.clone()) else { return };
        let bus = item.bus.clone();
        std::thread::spawn(move || {
            if let Ok(p) = zbus::blocking::Proxy::new(&conn, bus, menu, "com.canonical.dbusmenu") {
                let ts = 0u32;
                let _: Result<(), _> = p.call("Event", &(id, "clicked", Value::I32(0), ts));
            }
        });
    }
}
