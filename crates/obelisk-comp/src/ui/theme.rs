//! Tokens de design do Obelisk (espelham `branding/tokens.json` / `docs/02-design-system.md`).
//!
//! Cores em RGBA 0..255 (não pré-multiplicado). Os números são inspirados nos
//! *padrões* de interface do macOS (cantos contínuos, vidro translúcido, luzes de
//! trânsito à esquerda), sem copiar nenhum ativo da Apple.

#[derive(Clone, Copy, Debug)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

impl Rgba {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Rgba(r, g, b, 255)
    }
    pub fn with_alpha(self, a: u8) -> Self {
        Rgba(self.0, self.1, self.2, a)
    }
    pub fn to_skia(self) -> resvg::tiny_skia::Color {
        resvg::tiny_skia::Color::from_rgba8(self.0, self.1, self.2, self.3)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appearance {
    Light,
    Dark,
}

/// Medidas em pixels lógicos.
pub mod metrics {
    pub const MENUBAR_H: i32 = 32;
    pub const TITLEBAR_H: i32 = 32;
    pub const WINDOW_RADIUS: f32 = 12.0;
    pub const MENU_RADIUS: f32 = 12.0;
    pub const TRAFFIC_D: f32 = 12.0;
    pub const TRAFFIC_GAP: f32 = 8.0;
    pub const SHADOW_PAD: i32 = 36;
    pub const LAUNCHER_W: i32 = 680;
}

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub appearance: Appearance,
    // rótulos (HIG): branco/preto com opacidade 85 / 55 / 25(40) / 10 %
    pub label: Rgba,
    pub secondary: Rgba,
    pub tertiary: Rgba,
    pub quaternary: Rgba,
    pub separator: Rgba,
    // superfícies
    pub window_bg: Rgba,
    pub raised_bg: Rgba,
    pub sidebar_bg: Rgba,
    pub dock_bg: Rgba,
    pub dock_border: Rgba,
    pub menu_bg: Rgba,
    pub menu_border: Rgba,
    pub cc_module: Rgba,
    pub surface: Rgba,
    pub surface_hover: Rgba,
    // cores de sistema (HIG, modo escuro jun/2025)
    pub blue: Rgba,
    pub red: Rgba,
    pub green: Rgba,
    pub orange: Rgba,
    pub yellow: Rgba,
    pub purple: Rgba,
    pub pink: Rgba,
    pub gray: Rgba,
    pub accent: Rgba,
    pub text_on_accent: Rgba,
    // compatibilidade com os componentes já existentes
    pub menubar_bg: Rgba,
    pub menubar_text: Rgba,
    pub titlebar_bg: Rgba,
    pub titlebar_bg_inactive: Rgba,
    pub titlebar_text: Rgba,
    pub titlebar_text_inactive: Rgba,
    pub panel_bg: Rgba,
    pub panel_text: Rgba,
    pub panel_text_secondary: Rgba,
    pub selection: Rgba,
    pub selection_text: Rgba,
    pub tl_close: Rgba,
    pub tl_min: Rgba,
    pub tl_max: Rgba,
    pub tl_inactive: Rgba,
    pub dot: Rgba,
    pub tooltip_bg: Rgba,
    pub tooltip_text: Rgba,
}

fn fg(dark: bool, opacity: f32) -> Rgba {
    let c = if dark { 255 } else { 0 };
    Rgba(c, c, c, (opacity * 255.0).round() as u8)
}

fn rgbaf(r: f32, g: f32, b: f32, a: f32) -> Rgba {
    Rgba((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8, (a * 255.0) as u8)
}

impl Theme {
    fn build(dark: bool) -> Self {
        let blue = Rgba::rgb(0x00, 0x91, 0xff);
        let window_bg = if dark { Rgba::rgb(0x1e, 0x1e, 0x20) } else { Rgba::rgb(0xf5, 0xf5, 0xf7) };
        let raised_bg = if dark { Rgba::rgb(0x2a, 0x2a, 0x2d) } else { Rgba::rgb(0xff, 0xff, 0xff) };
        let menu_bg = if dark { rgbaf(0.13, 0.13, 0.14, 0.72) } else { rgbaf(0.98, 0.98, 0.99, 0.9) };
        let label = fg(dark, 0.85);
        let secondary = fg(dark, 0.55);
        Theme {
            appearance: if dark { Appearance::Dark } else { Appearance::Light },
            label,
            secondary,
            tertiary: fg(dark, if dark { 0.25 } else { 0.4 }),
            quaternary: fg(dark, 0.10),
            separator: fg(dark, 0.10),
            window_bg,
            raised_bg,
            sidebar_bg: if dark { rgbaf(0.13, 0.13, 0.15, 0.85) } else { rgbaf(0.94, 0.94, 0.96, 0.9) },
            dock_bg: if dark { rgbaf(0.11, 0.11, 0.13, 0.42) } else { rgbaf(1.0, 1.0, 1.0, 0.65) },
            dock_border: fg(dark, 0.14),
            menu_bg,
            menu_border: fg(dark, 0.14),
            cc_module: fg(dark, 0.08),
            surface: fg(dark, 0.08),
            surface_hover: fg(dark, 0.14),
            blue,
            red: Rgba::rgb(0xff, 0x42, 0x45),
            green: Rgba::rgb(0x30, 0xd1, 0x58),
            orange: Rgba::rgb(0xff, 0x92, 0x30),
            yellow: Rgba::rgb(0xff, 0xd6, 0x00),
            purple: Rgba::rgb(0xdb, 0x34, 0xf2),
            pink: Rgba::rgb(0xff, 0x37, 0x5f),
            gray: Rgba::rgb(0x8e, 0x8e, 0x93),
            accent: blue,
            text_on_accent: Rgba::rgb(255, 255, 255),
            menubar_bg: Rgba(0, 0, 0, 0),
            menubar_text: Rgba::rgb(255, 255, 255),
            titlebar_bg: window_bg,
            titlebar_bg_inactive: window_bg,
            titlebar_text: label,
            titlebar_text_inactive: fg(dark, 0.25),
            panel_bg: menu_bg,
            panel_text: label,
            panel_text_secondary: secondary,
            selection: blue,
            selection_text: Rgba::rgb(255, 255, 255),
            tl_close: Rgba::rgb(0xff, 0x5f, 0x57),
            tl_min: Rgba::rgb(0xfe, 0xbc, 0x2e),
            tl_max: Rgba::rgb(0x28, 0xc8, 0x40),
            tl_inactive: fg(dark, if dark { 0.18 } else { 0.15 }),
            dot: fg(dark, 0.55),
            tooltip_bg: menu_bg,
            tooltip_text: label,
        }
    }

    pub fn light() -> Self {
        Self::build(false)
    }

    pub fn dark() -> Self {
        Self::build(true)
    }

    /// Cores de destaque (Ajustes > Aparência), como as do macOS.
    pub const ACCENTS: [(&'static str, &'static str, (u8, u8, u8)); 8] = [
        ("blue", "Azul", (0x00, 0x91, 0xff)),
        ("purple", "Roxo", (0xa5, 0x5e, 0xea)),
        ("pink", "Rosa", (0xff, 0x37, 0x8f)),
        ("red", "Vermelho", (0xff, 0x42, 0x45)),
        ("orange", "Laranja", (0xff, 0x92, 0x30)),
        ("yellow", "Amarelo", (0xff, 0xc6, 0x00)),
        ("green", "Verde", (0x30, 0xd1, 0x58)),
        ("graphite", "Grafite", (0x8e, 0x8e, 0x93)),
    ];

    pub fn with_accent(mut self, name: &str) -> Self {
        if let Some((_, _, (r, g, b))) = Self::ACCENTS.iter().find(|a| a.0 == name) {
            let c = Rgba::rgb(*r, *g, *b);
            self.accent = c;
            self.selection = c;
            // texto sobre o destaque: branco enquanto houver contraste (amarelo/laranja/verde → escuro)
            let lum = 0.2126 * *r as f32 + 0.7152 * *g as f32 + 0.0722 * *b as f32;
            self.text_on_accent = if lum > 150.0 { Rgba(0, 0, 0, 217) } else { Rgba::rgb(255, 255, 255) };
            self.selection_text = self.text_on_accent;
        }
        self
    }

    /// Branco (escuro) ou preto (claro) com opacidade `o` — o `foreground()` do kyro.
    pub fn fg(&self, o: f32) -> Rgba {
        fg(self.appearance == Appearance::Dark, o)
    }

    pub fn menu_bg_opaque(&self) -> Rgba {
        self.menu_bg.with_alpha(242)
    }

    /// Texto da barra de menus (Tahoe): preto em wallpaper claro, branco em escuro.
    pub fn bar_text(top_luminance: f32) -> (Rgba, Rgba, bool) {
        if top_luminance > 0.62 {
            (Rgba(0, 0, 0, 217), Rgba(0, 0, 0, 89), true)
        } else {
            (Rgba(255, 255, 255, 255), Rgba(255, 255, 255, 115), false)
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name {
            "light" | "claro" => Self::light(),
            _ => Self::dark(),
        }
    }
}
