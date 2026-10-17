//! What a message does to the composition, through its mappings.

use std::collections::HashMap;

use prism_core::{Composition, ControlSource, ControlTarget, Effect, EffectOwner, Mapping, Unit};

use crate::ControlMessage;

/// What applying a message asks of the application, besides the edits
/// already made to the composition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The composition changed.
    Changed,
    /// A tap of the tap tempo.
    Tap,
    /// Start a bar.
    Resync,
}

/// Applies controller messages to a composition. Remembers which buttons
/// are down, so a control change acting as a button acts once per press.
#[derive(Debug, Default)]
pub struct Controls {
    down: HashMap<ControlSource, bool>,
}

impl Controls {
    /// Applies `message` through the mappings of `composition`.
    pub fn apply(
        &mut self,
        composition: &mut Composition,
        message: &ControlMessage,
    ) -> Vec<Outcome> {
        let was_down = self
            .down
            .insert(message.source.clone(), message.pressed)
            .unwrap_or(false);
        let pressed_now = message.pressed && !was_down;
        let targets: Vec<ControlTarget> = composition
            .mappings
            .iter()
            .filter(|mapping| mapping.source == message.source)
            .map(|mapping| mapping.target.clone())
            .collect();
        let mut outcomes = Vec::new();
        for target in targets {
            if !target.is_continuous() && !pressed_now {
                continue;
            }
            if let Some(outcome) = apply_target(composition, &target, message.value)
                && !outcomes.contains(&outcome)
            {
                outcomes.push(outcome);
            }
        }
        outcomes
    }
}

fn effect_mut(
    composition: &mut Composition,
    owner: EffectOwner,
    index: usize,
) -> Option<&mut Effect> {
    match owner {
        EffectOwner::Composition => composition.effects.get_mut(index),
        EffectOwner::Layer { layer } => composition.layer_mut(layer)?.effects.get_mut(index),
        EffectOwner::Clip { slot } => composition.clip_mut(slot)?.effects.get_mut(index),
        EffectOwner::Generator { slot } => composition.clip_mut(slot)?.source.generator_mut(),
    }
}

fn apply_target(
    composition: &mut Composition,
    target: &ControlTarget,
    value: f32,
) -> Option<Outcome> {
    let changed = Some(Outcome::Changed);
    match target {
        ControlTarget::Clip { slot } => composition.trigger_clip(*slot).ok().and(changed),
        ControlTarget::Column { column } => composition.trigger_column(*column).ok().and(changed),
        ControlTarget::LayerOpacity { layer } => {
            composition.layer_mut(*layer)?.opacity = Unit::new(value);
            changed
        }
        ControlTarget::LayerBypass { layer } => {
            let layer = composition.layer_mut(*layer)?;
            layer.bypass = !layer.bypass;
            changed
        }
        ControlTarget::LayerSolo { layer } => {
            let layer = composition.layer_mut(*layer)?;
            layer.solo = !layer.solo;
            changed
        }
        ControlTarget::LayerStop { layer } => composition.stop_layer(*layer).ok().and(changed),
        ControlTarget::Crossfader => {
            composition.crossfader.position = Unit::new(value);
            changed
        }
        ControlTarget::Dashboard { slot, slider } => {
            *composition.clip_mut(*slot)?.dashboard.get_mut(*slider)? = Unit::new(value);
            changed
        }
        ControlTarget::EffectParam {
            owner,
            index,
            param,
        } => {
            let effect = effect_mut(composition, *owner, *index)?;
            let info = prism_fx::info_for(effect)?;
            let range = info.params.iter().find(|info| info.id == param)?;
            effect.set_param(
                param.clone(),
                range.clamp(range.min + (range.max - range.min) * value),
            );
            changed
        }
        ControlTarget::EffectBypass { owner, index } => {
            let effect = effect_mut(composition, *owner, *index)?;
            effect.bypass = !effect.bypass;
            changed
        }
        ControlTarget::Tap => Some(Outcome::Tap),
        ControlTarget::Resync => Some(Outcome::Resync),
        ControlTarget::StopAll => {
            composition.stop_all();
            changed
        }
    }
}

/// Adds a mapping from `source` to `target`, replacing what `source` drove
/// before: learning a control again moves it.
pub fn learn(composition: &mut Composition, source: ControlSource, target: ControlTarget) {
    composition
        .mappings
        .retain(|mapping| mapping.source != source);
    composition.mappings.push(Mapping { source, target });
}

#[cfg(test)]
mod tests {
    use super::*;
    use prism_core::{Clip, Slot};

    fn note(note: u8, pressed: bool) -> ControlMessage {
        ControlMessage {
            source: ControlSource::MidiNote { channel: 0, note },
            value: if pressed { 1.0 } else { 0.0 },
            pressed,
        }
    }

    fn control(number: u8, value: f32) -> ControlMessage {
        ControlMessage {
            source: ControlSource::MidiControl { channel: 0, number },
            value,
            pressed: value >= 0.5,
        }
    }

    #[test]
    fn buttons_act_once_per_press_faders_follow() {
        let mut comp = Composition::default();
        let layer = comp.layers()[0].id();
        let slot = Slot::new(layer, comp.columns()[0].id());
        comp.set_clip(slot, Some(Clip::from_file("/a.mov")))
            .unwrap();
        learn(
            &mut comp,
            note(36, true).source,
            ControlTarget::Clip { slot },
        );
        learn(
            &mut comp,
            control(1, 0.0).source,
            ControlTarget::LayerOpacity { layer },
        );
        learn(
            &mut comp,
            control(2, 0.0).source,
            ControlTarget::LayerBypass { layer },
        );
        learn(&mut comp, note(40, true).source, ControlTarget::Tap);
        let mut controls = Controls::default();

        assert_eq!(
            controls.apply(&mut comp, &note(36, true)),
            [Outcome::Changed]
        );
        assert_eq!(
            comp.active_clip(layer).map(|(column, _)| column),
            Some(slot.column)
        );
        assert!(controls.apply(&mut comp, &note(36, false)).is_empty());

        controls.apply(&mut comp, &control(1, 0.25));
        assert_eq!(comp.layer(layer).unwrap().opacity.get(), 0.25);

        // A control change used as a button: one switch per press.
        controls.apply(&mut comp, &control(2, 1.0));
        controls.apply(&mut comp, &control(2, 1.0));
        assert!(comp.layer(layer).unwrap().bypass);
        controls.apply(&mut comp, &control(2, 0.0));
        controls.apply(&mut comp, &control(2, 1.0));
        assert!(!comp.layer(layer).unwrap().bypass);

        assert_eq!(controls.apply(&mut comp, &note(40, true)), [Outcome::Tap]);
        // Not mapped.
        assert!(controls.apply(&mut comp, &note(99, true)).is_empty());
    }

    #[test]
    fn effect_settings_follow_across_their_range() {
        let mut comp = Composition::default();
        comp.effects.push(Effect::new("blur"));
        let target = ControlTarget::EffectParam {
            owner: EffectOwner::Composition,
            index: 0,
            param: "radius".into(),
        };
        learn(&mut comp, control(10, 0.0).source, target.clone());
        // Learning the same control again replaces its mapping.
        learn(&mut comp, control(10, 0.0).source, target);
        assert_eq!(comp.mappings.len(), 1);
        let mut controls = Controls::default();
        controls.apply(&mut comp, &control(10, 0.5));
        assert_eq!(comp.effects[0].param("radius"), Some(32.0));
        // A mapping whose effect is gone does nothing.
        comp.effects.clear();
        assert!(controls.apply(&mut comp, &control(10, 0.7)).is_empty());
    }
}
