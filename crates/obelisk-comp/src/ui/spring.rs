//! Mola com a física do SwiftUI (como o `Spring.qml` do kyro):
//! rigidez k = (2π/response)², amortecimento c = 4π·ζ/response, massa 1, passos de 1/240 s.
//! Preserva a velocidade quando o alvo muda no meio do caminho.

#[derive(Clone, Copy, Debug)]
pub struct Spring {
    pub value: f32,
    pub velocity: f32,
    pub target: f32,
    pub response: f32,
    pub damping: f32,
}

impl Spring {
    pub fn new(value: f32, response: f32, damping: f32) -> Self {
        Spring { value, velocity: 0.0, target: value, response, damping }
    }

    pub fn set_target(&mut self, t: f32) {
        self.target = t;
    }

    pub fn step(&mut self, dt: f32) {
        let k = (2.0 * std::f32::consts::PI / self.response).powi(2);
        let c = 4.0 * std::f32::consts::PI * self.damping / self.response;
        let mut left = dt.min(0.1);
        const H: f32 = 1.0 / 240.0;
        while left > 0.0 {
            let h = left.min(H);
            let a = -k * (self.value - self.target) - c * self.velocity;
            self.velocity += a * h;
            self.value += self.velocity * h;
            left -= h;
        }
        if self.settled() {
            self.value = self.target;
            self.velocity = 0.0;
        }
    }

    pub fn settled(&self) -> bool {
        (self.value - self.target).abs() < 0.001 && self.velocity.abs() < 0.01
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn converge() {
        let mut s = Spring::new(0.0, 0.42, 0.8);
        s.set_target(1.0);
        for _ in 0..120 {
            s.step(1.0 / 60.0);
        }
        assert!((s.value - 1.0).abs() < 0.01);
        // ζ < 1 passa do alvo (quique leve)
        let mut s = Spring::new(0.0, 0.42, 0.8);
        s.set_target(1.0);
        let mut max: f32 = 0.0;
        for _ in 0..60 {
            s.step(1.0 / 60.0);
            max = max.max(s.value);
        }
        assert!(max > 1.0);
    }
}
