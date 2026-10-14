//! Names shown in the interface for model values.

use prism_core::{BlendMode, CrossfaderCurve, CrossfaderSide, Direction, LoopMode};

pub(crate) fn blend(mode: BlendMode) -> &'static str {
    match mode {
        BlendMode::Normal => "Normal",
        BlendMode::Add => "Addition",
        BlendMode::Multiply => "Produit",
        BlendMode::Screen => "Écran",
        BlendMode::Lighten => "Éclaircir",
        BlendMode::Darken => "Assombrir",
        BlendMode::Difference => "Différence",
        BlendMode::Subtract => "Soustraction",
        BlendMode::Overlay => "Incrustation",
        BlendMode::Exclusion => "Exclusion",
    }
}

pub(crate) fn curve(curve: CrossfaderCurve) -> &'static str {
    match curve {
        CrossfaderCurve::FullCenter => "Plein au centre",
        CrossfaderCurve::Linear => "Linéaire",
        CrossfaderCurve::EqualPower => "Puissance constante",
    }
}

pub(crate) fn side(side: Option<CrossfaderSide>) -> &'static str {
    match side {
        None => "–",
        Some(CrossfaderSide::A) => "A",
        Some(CrossfaderSide::B) => "B",
    }
}

pub(crate) fn direction(direction: Direction) -> &'static str {
    match direction {
        Direction::Forward => "Avant",
        Direction::Backward => "Arrière",
        Direction::Random => "Aléatoire",
        Direction::Paused => "Pause",
    }
}

pub(crate) fn loop_mode(mode: LoopMode) -> &'static str {
    match mode {
        LoopMode::Loop => "Boucle",
        LoopMode::PingPong => "Aller-retour",
        LoopMode::Once => "Une fois",
    }
}

/// Seconds as `m:ss.cc`.
pub(crate) fn time(seconds: f64) -> String {
    let seconds = seconds.max(0.0);
    let minutes = (seconds / 60.0).floor();
    format!("{}:{:05.2}", minutes as u64, seconds - minutes * 60.0)
}

#[cfg(test)]
mod tests {
    #[test]
    fn times_are_minutes_and_seconds() {
        assert_eq!(super::time(0.0), "0:00.00");
        assert_eq!(super::time(65.5), "1:05.50");
        assert_eq!(super::time(-3.0), "0:00.00");
    }
}
