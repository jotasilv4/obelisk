//! Emojis (⌃⌘Espaço): busca em português e grade colorida. Os emojis vêm da Noto Color Emoji
//! (imagens PNG na tabela CBDT, lidas com ttf-parser). Escolher insere no app em foco.

use std::collections::HashMap;

use resvg::tiny_skia::Pixmap;

use super::layer::Layer;
use crate::ui::canvas::Canvas;
use crate::ui::text::Weight;
use crate::ui::theme::Theme;

/// (emoji, palavras-chave)
const EMOJIS: &[(&str, &str)] = &[
    ("😀", "sorriso feliz rindo"), ("😂", "chorando de rir lagrimas"), ("🤣", "rolando de rir"), ("😊", "sorriso timido corado"),
    ("😍", "apaixonado coracao olhos"), ("😘", "beijo"), ("😎", "oculos legal"), ("🤔", "pensando duvida"),
    ("😅", "suor alivio"), ("😉", "piscando"), ("😭", "chorando triste"), ("😢", "triste lagrima"),
    ("😡", "bravo raiva"), ("😱", "grito medo"), ("😴", "dormindo sono"), ("🤯", "cabeca explodindo"),
    ("🥳", "festa comemorar"), ("🙄", "revirando olhos"), ("😬", "careta"), ("🤩", "deslumbrado estrelas"),
    ("😇", "anjo"), ("🤗", "abraco"), ("🤫", "silencio segredo"), ("🫡", "continencia saudacao"),
    ("👍", "joinha positivo ok"), ("👎", "negativo"), ("👏", "palmas aplausos"), ("🙏", "por favor obrigado reza"),
    ("👋", "tchau oi aceno"), ("💪", "forca musculo"), ("🤝", "aperto de mao acordo"), ("👀", "olhos olhando"),
    ("✌", "paz vitoria"), ("🤞", "dedos cruzados sorte"), ("👌", "perfeito ok"), ("🫶", "maos coracao"),
    ("❤", "coracao amor vermelho"), ("💙", "coracao azul"), ("💚", "coracao verde"), ("💜", "coracao roxo"),
    ("🖤", "coracao preto"), ("💔", "coracao partido"), ("✨", "brilho estrelas"), ("🔥", "fogo quente"),
    ("⭐", "estrela"), ("🌟", "estrela brilhante"), ("💯", "cem perfeito"), ("✅", "certo feito check"),
    ("❌", "errado x nao"), ("⚠", "aviso atencao"), ("❓", "pergunta"), ("❗", "exclamacao"),
    ("🎮", "jogo videogame controle"), ("🕹", "joystick arcade"), ("🎯", "alvo mira"), ("🏆", "trofeu vitoria campeao"),
    ("🥇", "medalha ouro primeiro"), ("⚽", "futebol bola"), ("🏀", "basquete"), ("🎲", "dado jogo"),
    ("🎉", "festa confete"), ("🎂", "bolo aniversario"), ("🎁", "presente"), ("🎵", "musica nota"),
    ("🎧", "fone musica"), ("📸", "camera foto"), ("💻", "notebook computador"), ("🖥", "monitor computador"),
    ("⌨", "teclado"), ("🖱", "mouse"), ("📱", "celular"), ("🔋", "bateria"),
    ("💡", "ideia lampada"), ("📌", "fixar alfinete"), ("📎", "clipe anexo"), ("📝", "nota escrever"),
    ("📅", "calendario data"), ("⏰", "despertador alarme"), ("⌛", "ampulheta tempo"), ("🔒", "cadeado bloqueado"),
    ("🔑", "chave"), ("⚙", "engrenagem configuracao"), ("🛠", "ferramentas"), ("🐛", "bug inseto"),
    ("🚀", "foguete lancamento rapido"), ("✈", "aviao viagem"), ("🚗", "carro"), ("🏠", "casa"),
    ("🌍", "mundo terra"), ("☀", "sol"), ("🌙", "lua noite"), ("🌧", "chuva"),
    ("❄", "neve frio"), ("🌈", "arco iris"), ("🌊", "onda mar"), ("🌱", "planta broto"),
    ("🐶", "cachorro"), ("🐱", "gato"), ("🦊", "raposa"), ("🐼", "panda"),
    ("🐧", "pinguim linux"), ("🦀", "caranguejo rust"), ("🐍", "cobra python"), ("🦄", "unicornio"),
    ("🍕", "pizza"), ("🍔", "hamburguer"), ("🍟", "batata frita"), ("🍣", "sushi"),
    ("☕", "cafe"), ("🍺", "cerveja"), ("🍷", "vinho"), ("🍫", "chocolate"),
    ("🍎", "maca"), ("🍌", "banana"), ("🍉", "melancia"), ("🥑", "abacate"),
    ("💰", "dinheiro"), ("💸", "dinheiro voando gasto"), ("📈", "grafico subindo"), ("📉", "grafico caindo"),
    ("🙈", "macaco nao vejo"), ("💀", "caveira morto"), ("👻", "fantasma"), ("🤖", "robo"),
    ("👽", "alien"), ("💩", "coco"), ("🤡", "palhaco"), ("😈", "diabinho"),
];

const COLS: usize = 9;
const CELL: f32 = 44.0;
const W: f32 = COLS as f32 * CELL + 24.0;
const SHADOW: f32 = 40.0;

pub struct EmojiPicker {
    pub open: bool,
    pub query: String,
    pub hover: Option<usize>,
    pub layer: Layer,
    pub pos: (f32, f32),
    pub size: (f32, f32),
    face_data: Option<Vec<u8>>,
    cache: HashMap<String, Option<Pixmap>>,
    visible: Vec<usize>,
    key: Option<String>,
}

impl Default for EmojiPicker {
    fn default() -> Self {
        EmojiPicker {
            open: false,
            query: String::new(),
            hover: None,
            layer: Layer::default(),
            pos: (0.0, 0.0),
            size: (0.0, 0.0),
            face_data: std::fs::read("/usr/share/fonts/noto/NotoColorEmoji.ttf").ok(),
            cache: HashMap::new(),
            visible: Vec::new(),
            key: None,
        }
    }
}

impl EmojiPicker {
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.query.clear();
        self.hover = None;
        self.key = None;
    }

    fn glyph(&mut self, e: &str) -> Option<Pixmap> {
        if let Some(p) = self.cache.get(e) {
            return p.clone();
        }
        let pm = (|| {
            let face = ttf_parser::Face::parse(self.face_data.as_ref()?, 0).ok()?;
            let ch = e.chars().next()?;
            let gid = face.glyph_index(ch)?;
            let img = face.glyph_raster_image(gid, 128)?;
            Pixmap::decode_png(img.data).ok()
        })();
        self.cache.insert(e.to_string(), pm.clone());
        pm
    }

    fn filter(&mut self) {
        let q = self.query.trim().to_lowercase();
        self.visible = EMOJIS
            .iter()
            .enumerate()
            .filter(|(_, (_, k))| q.is_empty() || q.split_whitespace().all(|w| k.contains(w)))
            .map(|(i, _)| i)
            .collect();
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        self.open && (x as f32) >= self.pos.0 && (x as f32) < self.pos.0 + self.size.0 && (y as f32) >= self.pos.1 && (y as f32) < self.pos.1 + self.size.1
    }

    /// Emoji sob o mouse.
    pub fn at(&self, x: f64, y: f64) -> Option<usize> {
        let (gx, gy) = (self.pos.0 + 12.0, self.pos.1 + 56.0);
        let (cx, cy) = (((x as f32 - gx) / CELL).floor(), ((y as f32 - gy) / CELL).floor());
        if cx < 0.0 || cy < 0.0 || cx >= COLS as f32 {
            return None;
        }
        let i = cy as usize * COLS + cx as usize;
        (i < self.visible.len()).then_some(i)
    }

    pub fn emoji(&self, i: usize) -> Option<&'static str> {
        self.visible.get(i).map(|&k| EMOJIS[k].0)
    }

    pub fn update(&mut self, theme: &Theme, sw: i32, sh: i32, scale: i32, backdrop: Option<&Pixmap>) {
        self.filter();
        let rows = self.visible.len().div_ceil(COLS).clamp(1, 7);
        let h = 56.0 + rows as f32 * CELL + 14.0;
        self.size = (W, h);
        self.pos = (((sw as f32 - W) / 2.0).round(), (sh as f32 * 0.25).round());
        let key = format!("{}|{:?}|{}|{sw}|{scale}", self.query, self.hover, theme.appearance as u8);
        if self.key.as_deref() == Some(key.as_str()) && self.layer.is_set() {
            return;
        }
        self.key = Some(key);
        let mut c = Canvas::new((W + 2.0 * SHADOW) as i32, (h + 2.0 * SHADOW) as i32, scale as f32).unwrap();
        let (ox, oy) = (SHADOW, SHADOW);
        c.shadow_rrect(ox, oy + 10.0, W, h, 16.0, 36.0, 120);
        if let Some(bd) = backdrop {
            c.fill_rrect_backdrop(ox, oy, W, h, 16.0, bd, (self.pos.0 - SHADOW, self.pos.1 - SHADOW), (sw as f32, sh as f32));
        }
        c.fill_rrect(ox, oy, W, h, 16.0, theme.menu_bg);
        c.stroke_rrect(ox, oy, W, h, 16.0, 1.0, theme.menu_border);
        c.fill_rrect(ox + 12.0, oy + 12.0, W - 24.0, 32.0, 9.0, theme.fg(0.08));
        c.symbol("search", ox + 12.0 + 16.0, oy + 28.0, 16.0, theme.secondary);
        if self.query.is_empty() {
            c.text_vcenter("Buscar emoji", ox + 40.0, oy + 12.0, 32.0, 13.0, Weight::Regular, theme.tertiary);
        } else {
            let tw = c.text_vcenter(&self.query, ox + 40.0, oy + 12.0, 32.0, 13.0, Weight::Regular, theme.label);
            c.fill_rrect(ox + 41.0 + tw, oy + 19.0, 1.5, 18.0, 0.0, theme.accent);
        }
        let list: Vec<usize> = self.visible.iter().copied().take(COLS * 7).collect();
        for (n, k) in list.into_iter().enumerate() {
            let (col, row) = (n % COLS, n / COLS);
            let (x, y) = (ox + 12.0 + col as f32 * CELL, oy + 56.0 + row as f32 * CELL);
            if self.hover == Some(n) {
                c.fill_rrect(x + 2.0, y + 2.0, CELL - 4.0, CELL - 4.0, 9.0, theme.fg(0.14));
            }
            if let Some(pm) = self.glyph(EMOJIS[k].0) {
                c.blit(&pm, x + 7.0, y + 7.0, CELL - 14.0, CELL - 14.0, 1.0);
            }
        }
        if self.visible.is_empty() {
            c.text_vcenter("Nenhum emoji", ox + 16.0, oy + 56.0, CELL, 13.0, Weight::Regular, theme.secondary);
        }
        self.layer.set(&c);
    }
}
