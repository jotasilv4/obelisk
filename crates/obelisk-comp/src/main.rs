//! Obelisk — compositor Wayland e shell com visual inspirado no macOS, em Rust.

#![allow(irrefutable_let_patterns)]

mod apps;
mod assets;
mod backend;
mod calc;
mod capture;
mod screencopy;
mod gamemode;
mod grabs;
pub mod handlers;
mod input;
mod lock;
mod recorder;
mod render;
mod rounded;
mod shell;
mod notifications;
mod polkit;
mod services;
mod settings;
mod settings_ctl;
mod state;
mod tiling;
mod tray;
mod xwayland;
mod ui;

use smithay::reexports::{calloop::EventLoop, wayland_server::Display};

pub use state::Obelisk;
use state::BackendKind;
use ui::theme::Theme;

fn usage() {
    println!(
        "obelisk-compositor [opções]\n\
         \n  --backend winit|drm   winit = janela aninhada (padrão se já houver sessão gráfica)\n\
         \x20 -c, --command CMD     executa CMD ao iniciar (ex.: foot)\n\
         \x20 --scale N             escala inteira da interface (1 ou 2)\n\
         \x20 --dark                aparência escura\n\
         \x20 --demo-dock           mostra todos os ícones do dock, mesmo sem o app instalado\n\
         \x20 --greeter             tela de login do greetd (usa GREETD_SOCK)\n\
         \nVariáveis: OBELISK_MOD=alt|super, OBELISK_ASSETS=<pasta branding>, RUST_LOG=debug"
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(filter) = tracing_subscriber::EnvFilter::try_from_default_env() {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    } else {
        tracing_subscriber::fmt().with_env_filter("info").init();
    }

    let mut args = std::env::args().skip(1);
    let mut backend_arg: Option<String> = None;
    let mut commands: Vec<String> = Vec::new();
    let mut scale = std::env::var("OBELISK_SCALE").ok().and_then(|s| s.parse().ok()).unwrap_or(1);
    let mut dark = !std::env::var("OBELISK_APPEARANCE").is_ok_and(|v| v == "light");
    let mut demo = std::env::var("OBELISK_DEMO_DOCK").is_ok();
    let mut debug_ui: Option<String> = None;
    let mut dark_flag_given = false;
    let mut debug_capture: Option<String> = None;
    // tela de login do greetd: só a tela de bloqueio com a lista de usuários
    let mut greeter = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--backend" => backend_arg = args.next(),
            "-c" | "--command" => commands.extend(args.next()),
            "--scale" => scale = args.next().and_then(|s| s.parse().ok()).unwrap_or(1),
            "--dark" => {
                dark = true;
                dark_flag_given = true;
            }
            "--light" => {
                dark = false;
                dark_flag_given = true;
            }
            "--demo-dock" => demo = true,
            "--debug-ui" => debug_ui = args.next(),
            "--debug-capture" => debug_capture = args.next(),
            "--greeter" => greeter = true,
            "-h" | "--help" => {
                usage();
                return Ok(());
            }
            other => {
                eprintln!("opção desconhecida: {other}");
                usage();
                std::process::exit(2);
            }
        }
    }

    let kind = match backend_arg.as_deref() {
        Some("winit") => BackendKind::Nested,
        Some("drm") | Some("tty") => BackendKind::Drm,
        Some(x) => {
            eprintln!("backend desconhecido: {x}");
            std::process::exit(2);
        }
        None if std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some() => BackendKind::Nested,
        None => BackendKind::Drm,
    };

    let theme = if dark { Theme::dark() } else { Theme::light() };
    let mut event_loop: EventLoop<'static, Obelisk> = EventLoop::try_new()?;
    let display: Display<Obelisk> = Display::new()?;
    let mut state = Obelisk::new(&mut event_loop, display, kind, theme, scale, demo);
    // Ajustes salvos (as flags --dark/--light da linha de comando têm prioridade)
    if !dark_flag_given {
        state.apply_settings();
    } else {
        let t = state.shell.theme;
        state.apply_settings();
        state.shell.theme = t;
        state.auto_appearance = false;
    }

    match kind {
        BackendKind::Nested => backend::winit::init(&mut event_loop, &mut state)?,
        BackendKind::Drm => backend::drm::init(&mut event_loop, &mut state)?,
    }

    // clientes lançados daqui em diante falam com este compositor
    std::env::set_var("WAYLAND_DISPLAY", &state.socket_name);
    std::env::set_var("XDG_SESSION_TYPE", "wayland");
    std::env::set_var("XDG_CURRENT_DESKTOP", "Obelisk");
    tracing::info!("Obelisk pronto em WAYLAND_DISPLAY={}", state.socket_name.to_string_lossy());
    // sessão de verdade: portais (compartilhar tela, escolher arquivo) e serviços do systemd do
    // usuário precisam saber deste display. Nunca no modo aninhado (bagunçaria a sessão do host).
    if std::env::var_os("OBELISK_SESSION").is_some() {
        crate::apps::spawn("dbus-update-activation-environment --systemd WAYLAND_DISPLAY XDG_CURRENT_DESKTOP XDG_SESSION_TYPE");
    }

    // testes visuais: --debug-capture "FRAMES:arquivo.png" captura e encerra
    // testes visuais: --debug-capture "MS:arquivo.png" captura depois de MS milissegundos e encerra
    if let Some(spec) = debug_capture {
        if let Some((ms, path)) = spec.split_once(':') {
            let path: std::path::PathBuf = path.into();
            let ms: u64 = ms.parse().unwrap_or(4000);
            state.debug_capture_path = Some(path.clone());
            let _ = event_loop.handle().insert_source(
                smithay::reexports::calloop::timer::Timer::from_duration(std::time::Duration::from_millis(ms)),
                move |_, _, state| {
                    state.debug_capture = Some((0, path.clone(), true));
                    smithay::reexports::calloop::timer::TimeoutAction::Drop
                },
            );
        }
    }
    let pending_ui = debug_ui;

    state.lock.attach(&event_loop.handle());
    if greeter {
        let last = std::fs::read_to_string("/var/cache/obelisk-greeter/last-user").ok().map(|s| s.trim().to_string());
        state.lock.start_greeter(last);
        state.greeter = true;
        if let Some(spec) = pending_ui {
            state.debug_ui(&spec);
        }
        event_loop.run(None, &mut state, |state| {
            let _ = state.display_handle.flush_clients();
            if state.extra_exit {
                state.loop_signal.stop();
            }
        })?;
        return Ok(());
    }
    // sistema live (pendrive): abre o instalador; senão, na primeira sessão, o assistente
    if shell::installer::is_live() {
        state.shell.installer.show();
    } else if std::env::var_os("OBELISK_SESSION").is_some() && !shell::welcome::done_file().exists() {
        state.shell.welcome.open = true;
    }
    // notificações (org.freedesktop.Notifications); não substitui outro servidor já ativo
    state.notif_conn = notifications::start(&event_loop.handle());
    state.tray = Some(tray::start(&event_loop.handle()));
    state.settings_init(&event_loop.handle());
    state.polkit_conn = polkit::start(&event_loop.handle());

    // XWayland: Steam, jogos via Proton/Wine e outros apps X11 (OBELISK_XWAYLAND=0 desliga)
    if std::env::var("OBELISK_XWAYLAND").map(|v| v != "0").unwrap_or(true) {
        state.start_xwayland();
    }

    for c in commands {
        apps::spawn(&c);
    }
    // a UI de teste abre depois que os clientes tiveram tempo de mapear janelas
    if let Some(d) = pending_ui {
        let _ = event_loop.handle().insert_source(
            smithay::reexports::calloop::timer::Timer::from_duration(std::time::Duration::from_millis(1500)),
            move |_, _, state| {
                state.debug_ui(&d);
                smithay::reexports::calloop::timer::TimeoutAction::Drop
            },
        );
    }

    event_loop.run(None, &mut state, |state| {
        let _ = state.display_handle.flush_clients();
        if state.extra_exit {
            state.loop_signal.stop();
        }
    })?;
    Ok(())
}
