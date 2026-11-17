//! Modelo dos Ajustes: cada página vira grupos de linhas. As páginas com dados do sistema são
//! montadas numa thread (podem rodar nmcli, pactl…), então nada trava a tela.

use std::collections::HashMap;

/// O que um clique pede. Tudo é `Send` para atravessar a thread.
#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    /// ajuste do próprio Obelisk (chave, valor)
    Set(String, String),
    /// comando comum (depois recarrega a página)
    Run(String),
    /// comando como root via pkexec (passa pelo agente polkit no notch)
    Root(String),
    /// comando com campos: `{id}` é trocado pelo texto do campo (já com aspas)
    Submit { template: String, root: bool },
    /// abre/fecha detalhes de um item da lista
    Expand(String),
    /// coloca o cursor num campo de texto
    Focus(String),
    Page(String),
    /// começou a arrastar a tela (nome) no arranjo de Telas
    DisplayGrab(String),
    /// tarefa longa com progresso (instalar app, atualizar o sistema): título e comando
    Job(String, String),
    Nothing,
}

#[derive(Clone, Debug)]
pub enum Row {
    /// (ícone, cor, rótulo, subtítulo, ligado, ação ao alternar)
    Switch { icon: String, color: (u8, u8, u8), label: String, sub: String, on: bool, act: Act },
    /// valor 0..1; `template` recebe `{v}` (0..100)
    Slider { icon: String, color: (u8, u8, u8), label: String, value: f32, template: String, root: bool },
    Choice { icon: String, color: (u8, u8, u8), label: String, options: Vec<(String, Act)>, selected: usize },
    /// item de lista (rede, dispositivo, saída de áudio…)
    Item { icon: String, color: (u8, u8, u8), title: String, sub: String, trailing: String, checked: bool, act: Act },
    Field { id: String, label: String, placeholder: String, secret: bool },
    Button { label: String, button: String, act: Act, primary: bool },
    Info { label: String, value: String },
    /// app da loja: ícone (arquivo), nome, descrição, botão e ação
    App { icon: String, title: String, sub: String, button: String, act: Act },
    Note(String),
}

#[derive(Clone, Debug, Default)]
pub struct Group {
    pub title: String,
    pub rows: Vec<Row>,
}

pub fn group(title: &str, rows: Vec<Row>) -> Group {
    Group { title: title.into(), rows }
}

pub fn sw(icon: &str, color: (u8, u8, u8), label: &str, sub: &str, on: bool, act: Act) -> Row {
    Row::Switch { icon: icon.into(), color, label: label.into(), sub: sub.into(), on, act }
}

pub fn item(icon: &str, color: (u8, u8, u8), title: &str, sub: &str, trailing: &str, checked: bool, act: Act) -> Row {
    Row::Item { icon: icon.into(), color, title: title.into(), sub: sub.into(), trailing: trailing.into(), checked, act }
}

pub fn info(label: &str, value: &str) -> Row {
    Row::Info { label: label.into(), value: value.into() }
}

pub fn button(label: &str, btn: &str, act: Act) -> Row {
    Row::Button { label: label.into(), button: btn.into(), act, primary: false }
}

pub fn note(t: &str) -> Row {
    Row::Note(t.into())
}

pub fn field(id: &str, label: &str, placeholder: &str, secret: bool) -> Row {
    Row::Field { id: id.into(), label: label.into(), placeholder: placeholder.into(), secret }
}

pub fn choice(icon: &str, color: (u8, u8, u8), label: &str, options: Vec<(&str, Act)>, selected: usize) -> Row {
    Row::Choice { icon: icon.into(), color, label: label.into(), options: options.into_iter().map(|(a, b)| (a.to_string(), b)).collect(), selected }
}

/// Aspas de shell para valores digitados pelo usuário.
pub fn q(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Executa um comando e devolve a saída (limite de 3 s).
pub fn sh(cmd: &str) -> String {
    sh_t(cmd, 3000)
}

/// Como `sh`, com limite de `ms` milissegundos (listas grandes da loja).
pub fn sh_t(cmd: &str, ms: u128) -> String {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let Ok(mut child) = Command::new("sh").arg("-c").arg(cmd).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn() else {
        return String::new();
    };
    // lê em paralelo: saídas grandes não travam o comando com o cano cheio
    let mut pipe = child.stdout.take();
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        if let Some(o) = pipe.as_mut() {
            let _ = o.read_to_string(&mut out);
        }
        out
    });
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return reader.join().unwrap_or_default(),
            Ok(None) if start.elapsed().as_millis() < ms => std::thread::sleep(std::time::Duration::from_millis(15)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return String::new();
            }
        }
    }
}

pub fn has(bin: &str) -> bool {
    crate::apps::first_in_path(&[bin]).is_some()
}

/// Estado do Obelisk que as páginas precisam (cópia enviada à thread).
#[derive(Clone, Debug, Default)]
pub struct Ctx {
    pub settings: HashMap<String, String>,
    pub expanded: Option<String>,
    pub nested: bool,
    pub night: bool,
    pub outputs: Vec<(String, String)>,
    pub scale: i32,
    pub fscale: f64,
    /// telas do backend nativo: (nome, modos, modo atual, escala)
    pub modes: Vec<(String, Vec<String>, String, f64)>,
}

impl Ctx {
    pub fn get(&self, k: &str) -> &str {
        self.settings.get(k).map(|s| s.as_str()).unwrap_or("")
    }
    pub fn on(&self, k: &str) -> bool {
        self.get(k) == "true"
    }
}
