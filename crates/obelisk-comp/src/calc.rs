//! Calculadora da Busca (docs/03-especificacao-kyro.md §6): `+ - * / ^ %`, parênteses,
//! `pi`, `e`, `sqrt()`, `abs()`, `round()`, notação `1e3`, vírgula ou ponto decimal, `×` `÷`.

pub fn eval(input: &str) -> Option<f64> {
    if input.len() > 160 {
        return None;
    }
    let lower = input.to_lowercase().replace(',', ".").replace('×', "*").replace('÷', "/");
    let mut s: Vec<char> = lower.chars().filter(|c| !c.is_whitespace()).collect();
    // `x` entre número/`)` e número/`(` vira multiplicação
    for i in 1..s.len().saturating_sub(1) {
        if s[i] == 'x' && (s[i - 1].is_ascii_digit() || s[i - 1] == ')') && (s[i + 1].is_ascii_digit() || s[i + 1] == '(') {
            s[i] = '*';
        }
    }
    // precisa ter algum operador/função: número sozinho não vira conta
    let text: String = s.iter().collect();
    let has_op = s.iter().any(|c| "+-*/^%(".contains(*c)) || text.contains("pi");
    if !has_op || !s.iter().any(|c| c.is_ascii_digit() || *c == 'p' || *c == 'e') {
        return None;
    }
    let mut p = Parser { s, i: 0, depth: 0 };
    let v = p.expr(0)?;
    (p.i == p.s.len() && v.is_finite()).then_some(v)
}

/// 12 algarismos significativos, sem zeros à direita, vírgula decimal.
pub fn format(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let digits = 12 - 1 - v.abs().log10().floor() as i32;
    let s = if digits > 0 { format!("{:.*}", digits.min(15) as usize, v) } else { format!("{:.0}", v) };
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    s.replace('.', ",")
}

struct Parser {
    s: Vec<char>,
    i: usize,
    depth: u32,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    fn eat(&mut self, word: &str) -> bool {
        let w: Vec<char> = word.chars().collect();
        if self.s.get(self.i..self.i + w.len()) == Some(&w[..]) {
            self.i += w.len();
            true
        } else {
            false
        }
    }

    /// Precedência: + - (1) < * / (2) < unário (3) < ^ (4, à direita).
    fn expr(&mut self, min: u8) -> Option<f64> {
        self.depth += 1;
        if self.depth > 40 {
            return None;
        }
        let mut lhs = self.unary()?;
        loop {
            let (op, prec) = match self.peek() {
                Some('+') => ('+', 1),
                Some('-') => ('-', 1),
                Some('*') => ('*', 2),
                Some('/') => ('/', 2),
                Some('^') => ('^', 4),
                _ => break,
            };
            if prec < min {
                break;
            }
            self.i += 1;
            let next_min = if op == '^' { prec } else { prec + 1 };
            let rhs = self.expr(next_min)?;
            lhs = match op {
                '+' => lhs + rhs,
                '-' => lhs - rhs,
                '*' => lhs * rhs,
                '/' => lhs / rhs,
                _ => lhs.powf(rhs),
            };
        }
        self.depth -= 1;
        Some(lhs)
    }

    fn unary(&mut self) -> Option<f64> {
        match self.peek()? {
            '-' => {
                self.i += 1;
                Some(-self.expr(3)?)
            }
            '+' => {
                self.i += 1;
                self.expr(3)
            }
            _ => self.postfix(),
        }
    }

    fn postfix(&mut self) -> Option<f64> {
        let mut v = self.atom()?;
        while self.peek() == Some('%') {
            self.i += 1;
            v /= 100.0;
        }
        Some(v)
    }

    fn atom(&mut self) -> Option<f64> {
        for (name, f) in [("sqrt", f64::sqrt as fn(f64) -> f64), ("abs", f64::abs), ("round", f64::round)] {
            if self.eat(name) {
                if self.peek()? != '(' {
                    return None;
                }
                return Some(f(self.atom()?));
            }
        }
        if self.eat("pi") {
            return Some(std::f64::consts::PI);
        }
        if self.peek()? == '(' {
            self.i += 1;
            let v = self.expr(0)?;
            if self.peek()? != ')' {
                return None;
            }
            self.i += 1;
            return Some(v);
        }
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '.') {
            self.i += 1;
        }
        if start == self.i {
            // constante e (sem número antes)
            if self.peek() == Some('e') {
                self.i += 1;
                return Some(std::f64::consts::E);
            }
            return None;
        }
        // expoente: 1e3, 2.5e-4
        if self.peek() == Some('e') {
            let save = self.i;
            self.i += 1;
            if matches!(self.peek(), Some('+') | Some('-')) {
                self.i += 1;
            }
            if self.peek().is_some_and(|c| c.is_ascii_digit()) {
                while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    self.i += 1;
                }
            } else {
                self.i = save;
            }
        }
        self.s[start..self.i].iter().collect::<String>().parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn basics() {
        assert_eq!(eval("2+2"), Some(4.0));
        assert_eq!(eval("2+3*4"), Some(14.0));
        assert_eq!(eval("(2+3)*4"), Some(20.0));
        assert_eq!(eval("2^3^2"), Some(512.0));
        assert_eq!(eval("-2^2"), Some(-4.0));
        assert_eq!(eval("10/4"), Some(2.5));
        assert_eq!(eval("1,5*2"), Some(3.0));
        assert_eq!(eval("200*10%"), Some(20.0));
        assert_eq!(eval("200*(1+10%)"), Some(220.00000000000003));
        assert_eq!(eval("sqrt(16)+abs(-2)"), Some(6.0));
        assert_eq!(eval("2x3"), Some(6.0));
        assert_eq!(eval("1e3+1"), Some(1001.0));
        assert!(eval("2*pi").is_some_and(|v| (v - 6.283185307).abs() < 1e-6));
        assert_eq!(eval("firefox"), None);
        assert_eq!(eval("42"), None);
        assert_eq!(format(2.5), "2,5");
        assert_eq!(format(14.0), "14");
        assert_eq!(format(220.00000000000003), "220");
        assert_eq!(format(1.0 / 3.0), "0,333333333333");
    }
}
