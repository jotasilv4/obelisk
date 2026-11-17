pub mod about_app;
pub mod activity;
pub mod capture_ui;
pub mod control_center;
pub mod deco;
pub mod desktop;
pub mod dock;
pub mod emoji;
pub mod installer;
pub mod force_quit;
pub mod launcher;
pub mod launchpad;
pub mod layer;
pub mod menu;
pub mod menubar;
pub mod mission;
pub mod notch;
pub mod notif_center;
pub mod settings_app;
pub mod settings_model;
pub mod settings_pages;
pub mod status_menu;
pub mod store_pages;
pub mod switcher;
pub mod wallpaper;
pub mod welcome;
pub mod widgets;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use smithay::reexports::wayland_server::backend::ObjectId;

use crate::apps::App;
use crate::services::SysStatus;
use crate::ui::icons::IconStore;
use crate::ui::theme::Theme;

/// Grupos de apps padrão do Dock: o primeiro instalado de cada grupo entra fixado.
const DEFAULT_PINS: [&[&str]; 9] = [
    &["org.gnome.Nautilus", "thunar", "org.xfce.Thunar", "org.kde.dolphin", "pcmanfm", "nemo"],
    &["firefox", "chromium", "google-chrome", "org.gnome.Epiphany", "brave-browser"],
    &["foot", "org.gnome.Ptyxis", "kitty", "Alacritty", "org.wezfurlong.wezterm", "org.gnome.Console"],
    &["org.gnome.TextEditor", "org.xfce.mousepad", "mousepad", "org.kde.kate", "code"],
    &["org.gnome.Loupe", "org.gnome.eog", "org.kde.gwenview"],
    &["mpv", "org.gnome.Totem", "vlc"],
    &["org.gnome.Calculator", "gnome-calculator", "org.kde.kcalc", "qalculate-gtk"],
    &["obelisk-store", "org.gnome.Software"],
    &["obelisk-settings", "org.gnome.Settings", "systemsettings"],
];

pub struct Shell {
    pub theme: Theme,
    pub menubar: menubar::MenuBar,
    pub dock: dock::Dock,
    pub launcher: launcher::Launcher,
    pub launchpad: launchpad::Launchpad,
    pub switcher: switcher::Switcher,
    pub cc: control_center::ControlCenter,
    pub mission: mission::MissionControl,
    pub notch: notch::Notch,
    pub nc: notif_center::NotifCenter,
    pub capture: capture_ui::CaptureUi,
    pub activity: activity::Activity,
    pub force_quit: force_quit::ForceQuit,
    pub desktop: desktop::Desktop,
    pub about_app: about_app::AboutApp,
    pub welcome: welcome::Welcome,
    pub installer: installer::Installer,
    pub status_menu: status_menu::StatusMenu,
    pub emoji: emoji::EmojiPicker,
    pub settings: settings_app::SettingsApp,
    pub widgets: widgets::Widgets,
    pub menu: Option<menu::Menu>,
    pub wallpaper: wallpaper::Wallpaper,
    pub decos: HashMap<ObjectId, deco::Deco>,
    pub icons: IconStore,
    pub apps: Vec<App>,
    pub apps_loaded: Instant,
    pub cursor: layer::Layer,
    pub cursors: crate::ui::cursor::CursorStore,
    pub cursor_layers: HashMap<(String, i32), (layer::Layer, (i32, i32))>,
    pub clock: String,
    pub clock_at: Instant,
    pub status: SysStatus,
    pub trash_full: bool,
    /// ids (.desktop) fixados no Dock, na ordem
    pub pinned: Vec<String>,
    /// Janela cuja barra de título está sob o mouse e o botão destacado.
    pub hover_title: Option<(ObjectId, Option<usize>)>,
}

fn pins_file() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("obelisk/dock"))
}

impl Shell {
    pub fn new(theme: Theme, demo_dock: bool) -> Self {
        let apps = crate::apps::load();
        let pinned = Self::load_pins(&apps, demo_dock);
        Shell {
            theme,
            menubar: Default::default(),
            dock: dock::Dock::new(),
            launcher: Default::default(),
            launchpad: Default::default(),
            switcher: Default::default(),
            cc: Default::default(),
            mission: Default::default(),
            notch: Default::default(),
            nc: Default::default(),
            capture: Default::default(),
            activity: Default::default(),
            force_quit: Default::default(),
            desktop: Default::default(),
            about_app: Default::default(),
            welcome: Default::default(),
            installer: Default::default(),
            status_menu: Default::default(),
            emoji: Default::default(),
            settings: Default::default(),
            widgets: Default::default(),
            menu: None,
            wallpaper: wallpaper::Wallpaper::new(),
            decos: HashMap::new(),
            icons: IconStore::default(),
            apps,
            apps_loaded: Instant::now(),
            cursor: Default::default(),
            cursors: crate::ui::cursor::CursorStore::new(),
            cursor_layers: HashMap::new(),
            clock: menubar::clock_string(),
            clock_at: Instant::now(),
            status: SysStatus::default(),
            trash_full: false,
            pinned,
            hover_title: None,
        }
    }

    fn load_pins(apps: &[App], demo: bool) -> Vec<String> {
        if let Some(text) = pins_file().and_then(|p| std::fs::read_to_string(p).ok()) {
            let v: Vec<String> = text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect();
            if !v.is_empty() {
                return v;
            }
        }
        DEFAULT_PINS
            .iter()
            .filter_map(|group| {
                group
                    .iter()
                    .find(|id| apps.iter().any(|a| a.id == **id))
                    .or(if demo { group.first() } else { None })
                    .map(|s| s.to_string())
            })
            .collect()
    }

    pub fn save_pins(&self) {
        if let Some(p) = pins_file() {
            let _ = std::fs::create_dir_all(p.parent().unwrap());
            let _ = std::fs::write(p, self.pinned.join("\n") + "\n");
        }
    }

    pub fn app_by_id(&self, id: &str) -> Option<&App> {
        self.apps.iter().find(|a| a.id == id)
    }

    /// Algum painel/sobreposição está aberto (fecha com clique fora ou Esc).
    pub fn any_popup(&self) -> bool {
        self.launcher.open || self.launchpad.open || self.mission.open || self.cc.open || self.nc.open || self.menu.is_some() || self.switcher.open
    }

    pub fn close_popups(&mut self) {
        self.launcher.close();
        self.launchpad.close();
        self.nc.open = false;
        self.activity.open = false;
        self.force_quit.open = false;
        self.about_app.open = false;
        self.status_menu.open = None;
        self.status_menu.dragging = false;
        self.emoji.open = false;
        self.settings.open = false;
        if self.mission.open {
            self.mission.toggle();
        }
        if self.cc.open {
            self.cc.toggle();
        }
        self.menu = None;
        self.dock.menu_open = false;
    }
}

pub fn trash_full() -> bool {
    std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join(".local/share/Trash/files"))
        .and_then(|p| std::fs::read_dir(p).ok())
        .is_some_and(|mut d| d.next().is_some())
}
