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

/// What a mapping drives, as written in the interface.
pub(crate) fn target(
    composition: &prism_core::Composition,
    target: &prism_core::ControlTarget,
) -> String {
    use prism_core::{ControlTarget, EffectOwner};
    let layer_name = |id| {
        composition.layer(id).map_or_else(
            || "couche retirée".to_owned(),
            |layer| format!("« {} »", layer.name),
        )
    };
    let clip_name = |slot: prism_core::Slot| {
        composition.clip(slot).map_or_else(
            || "case vide".to_owned(),
            |clip| format!("« {} »", clip.name),
        )
    };
    let owner_name = |owner: &EffectOwner| match owner {
        EffectOwner::Composition => "de la composition".to_owned(),
        EffectOwner::Layer { layer } => format!("de la couche {}", layer_name(*layer)),
        EffectOwner::Clip { slot } => format!("du clip {}", clip_name(*slot)),
        EffectOwner::Generator { slot } => format!("du générateur {}", clip_name(*slot)),
    };
    match target {
        ControlTarget::Clip { slot } => format!("Lancer le clip {}", clip_name(*slot)),
        ControlTarget::Column { column } => {
            let name = composition
                .columns()
                .iter()
                .find(|entry| entry.id() == *column)
                .map_or_else(
                    || "retirée".to_owned(),
                    |entry| format!("« {} »", entry.name),
                );
            format!("Lancer la colonne {name}")
        }
        ControlTarget::LayerOpacity { layer } => format!("Opacité de {}", layer_name(*layer)),
        ControlTarget::LayerBypass { layer } => format!("Bypass de {}", layer_name(*layer)),
        ControlTarget::LayerSolo { layer } => format!("Solo de {}", layer_name(*layer)),
        ControlTarget::LayerStop { layer } => format!("Arrêter {}", layer_name(*layer)),
        ControlTarget::Crossfader => "Crossfader".to_owned(),
        ControlTarget::Dashboard { slot, slider } => {
            format!("Curseur {} du clip {}", slider + 1, clip_name(*slot))
        }
        ControlTarget::EffectParam {
            owner,
            index,
            param,
        } => {
            format!("Effet {} {} : {param}", index + 1, owner_name(owner))
        }
        ControlTarget::EffectBypass { owner, index } => {
            format!("Activer l'effet {} {}", index + 1, owner_name(owner))
        }
        ControlTarget::Tap => "Tap tempo".to_owned(),
        ControlTarget::Resync => "Resync".to_owned(),
        ControlTarget::StopAll => "Tout arrêter".to_owned(),
    }
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
