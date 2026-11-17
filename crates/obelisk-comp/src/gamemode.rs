//! Modo Jogo (como o do macOS Sonoma e o do kyro).
//!
//! Liga sozinho quando há uma janela de jogo (Steam `steam_app_*`, gamescope, cs2 ou classes
//! listadas em `~/.config/obelisk/games`) e desliga quando a última fecha. Ligado:
//! - sem vidro (blur), magnificação do Dock nem visualizador de música;
//! - leituras do sistema (nmcli, wpctl…) pausadas;
//! - perfil de energia em desempenho (`powerprofilesctl`), devolvido ao sair;
//! - jogo em tela cheia é composto sozinho (permite *direct scanout* no backend nativo).

use std::path::PathBuf;

#[derive(Default)]
pub struct GameMode {
    pub active: bool,
    /// Escolha manual (botão do notch/Central): `Some(true)` força, `Some(false)` suspende até
    /// fechar todos os jogos.
    pub manual: Option<bool>,
    extra_classes: Vec<String>,
    previous_profile: Option<String>,
    loaded: bool,
    /// PIDs registrados no Feral GameMode (prioridade de CPU/GPU para o jogo).
    registered: std::collections::HashSet<i32>,
    bus: Option<zbus::blocking::Connection>,
}

const BUILTIN: [&str; 4] = ["gamescope", "cs2", "steam_app_", "steam_proton"];

fn games_file() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("obelisk/games"))
}

impl GameMode {
    pub fn is_game(&mut self, app_id: &str) -> bool {
        if !self.loaded {
            self.loaded = true;
            if let Some(t) = games_file().and_then(|p| std::fs::read_to_string(p).ok()) {
                self.extra_classes = t.lines().map(|l| l.trim().to_lowercase()).filter(|l| !l.is_empty()).collect();
            }
        }
        let a = app_id.to_lowercase();
        !a.is_empty() && (BUILTIN.iter().any(|b| a.starts_with(b)) || self.extra_classes.iter().any(|c| *c == a))
    }

    /// Recalcula o estado; devolve `Some(novo)` quando muda.
    pub fn evaluate(&mut self, game_running: bool) -> Option<bool> {
        if !game_running && self.manual == Some(false) {
            self.manual = None; // a suspensão manual vale até fechar todos os jogos
        }
        let want = self.manual.unwrap_or(game_running);
        if want == self.active {
            return None;
        }
        self.active = want;
        if want {
            self.enter();
        } else {
            self.leave();
        }
        Some(want)
    }

    pub fn toggle(&mut self, game_running: bool) {
        self.manual = Some(!self.active);
        if !game_running && self.active {
            self.manual = None;
        }
        let _ = self.evaluate(game_running);
    }

    fn enter(&mut self) {
        tracing::info!("Modo Jogo ligado");
        if crate::apps::first_in_path(&["powerprofilesctl"]).is_some() {
            self.previous_profile = std::process::Command::new("powerprofilesctl")
                .arg("get")
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .filter(|s| !s.is_empty());
            crate::apps::spawn("powerprofilesctl set performance");
        }
    }

    fn leave(&mut self) {
        tracing::info!("Modo Jogo desligado");
        if let Some(p) = self.previous_profile.take() {
            crate::apps::spawn(&format!("powerprofilesctl set {p}"));
        }
    }
}

impl GameMode {
    /// Mantém o Feral GameMode (gamemoded) informado dos processos de jogo, se ele existir.
    pub fn sync_pids(&mut self, pids: &[i32]) {
        let want: std::collections::HashSet<i32> = if self.active { pids.iter().copied().collect() } else { Default::default() };
        if want == self.registered {
            return;
        }
        if self.bus.is_none() {
            self.bus = zbus::blocking::Connection::session().ok();
        }
        let Some(bus) = &self.bus else { return };
        let call = |method: &str, pid: i32| {
            bus.call_method(
                Some("com.feralinteractive.GameMode"),
                "/com/feralinteractive/GameMode",
                Some("com.feralinteractive.GameMode"),
                method,
                &(pid,),
            )
            .map(|_| ())
        };
        for pid in want.difference(&self.registered) {
            match call("RegisterGame", *pid) {
                Ok(()) => tracing::info!("GameMode: jogo {pid} registrado"),
                Err(e) => tracing::debug!("GameMode indisponível: {e}"),
            }
        }
        for pid in self.registered.difference(&want) {
            let _ = call("UnregisterGame", *pid);
        }
        self.registered = want;
    }
}
