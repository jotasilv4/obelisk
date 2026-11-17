//! Ícones da Material Symbols Rounded por nome (ex.: `sym("wifi")`).

use super::symbols_table::SYMBOLS;

pub fn sym(name: &str) -> String {
    SYMBOLS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, c)| c.to_string())
        .unwrap_or_else(|| {
            tracing::warn!("símbolo desconhecido: {name}");
            "?".into()
        })
}
