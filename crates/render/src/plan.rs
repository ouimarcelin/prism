//! Which layers to draw for the current state of a composition.

use prism_core::{BlendMode, Composition, Slot};

/// A layer to draw: the slot of its playing clip and how to mix it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayerPlan {
    /// Layer and column of the playing clip.
    pub slot: Slot,
    /// Opacity with crossfader, bypass and solo applied.
    pub opacity: f32,
    /// Blend mode of the layer.
    pub blend_mode: BlendMode,
}

/// Layers to draw, bottom first: those playing a clip and not fully
/// transparent once bypass, solo and the crossfader are applied.
pub fn plan_layers(composition: &Composition) -> Vec<LayerPlan> {
    composition
        .layers()
        .iter()
        .filter_map(|layer| {
            let (column, _) = composition.active_clip(layer.id())?;
            let opacity = composition.effective_opacity(layer.id())?;
            (opacity > 0.0).then_some(LayerPlan {
                slot: Slot::new(layer.id(), column),
                opacity,
                blend_mode: layer.blend_mode,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use prism_core::{Clip, CrossfaderCurve, CrossfaderSide, Unit};

    use super::*;

    /// Three layers, each with a clip in the first column.
    fn playing_composition() -> Composition {
        let mut comp = Composition::default();
        let column = comp.columns()[0].id();
        let layers: Vec<_> = comp.layers().iter().map(|layer| layer.id()).collect();
        for &layer in &layers[..3] {
            comp.set_clip(Slot::new(layer, column), Some(Clip::from_file("/x.mov")))
                .unwrap();
        }
        comp.trigger_column(column).unwrap();
        comp
    }

    #[test]
    fn only_playing_layers_are_drawn_bottom_first() {
        let comp = playing_composition();
        let plan = plan_layers(&comp);
        let layers: Vec<_> = plan.iter().map(|p| p.slot.layer).collect();
        let expected: Vec<_> = comp.layers()[..3].iter().map(|l| l.id()).collect();
        assert_eq!(layers, expected);
    }

    #[test]
    fn invisible_layers_are_skipped() {
        let mut comp = playing_composition();
        let ids: Vec<_> = comp.layers().iter().map(|layer| layer.id()).collect();
        comp.layer_mut(ids[0]).unwrap().bypass = true;
        comp.layer_mut(ids[1]).unwrap().opacity = Unit::ZERO;
        comp.crossfader.curve = CrossfaderCurve::Linear;
        comp.crossfader.position = Unit::ONE;
        comp.layer_mut(ids[2]).unwrap().crossfader_side = Some(CrossfaderSide::A);
        assert!(plan_layers(&comp).is_empty());
    }

    #[test]
    fn plan_carries_opacity_and_blend_mode() {
        let mut comp = playing_composition();
        let top = comp.layers()[2].id();
        let layer = comp.layer_mut(top).unwrap();
        layer.opacity = Unit::new(0.5);
        layer.blend_mode = BlendMode::Screen;
        let plan = plan_layers(&comp);
        assert_eq!(plan[2].opacity, 0.5);
        assert_eq!(plan[2].blend_mode, BlendMode::Screen);
    }
}
