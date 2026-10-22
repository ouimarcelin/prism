//! Layers of the clip grid: dragged by their grip, renamed by a double
//! click.

use egui::{Pos2, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use prism_core::{Composition, LayerId};
use prism_engine::EngineStatus;
use prism_ui::{AudioStatus, ControlPanel, Document, FrameInfo, MidiStatus};

/// Drags from `from` to `to` over a few frames, as a hand would.
fn drag(harness: &mut Harness<'_>, from: Pos2, to: Pos2) {
    harness.hover_at(from);
    harness.run_steps(1);
    harness.drag_at(from);
    harness.run_steps(1);
    for step in 1..=6 {
        harness.hover_at(from + (to - from) * (step as f32 / 6.0));
        harness.run_steps(1);
    }
    harness.drop_at(to);
    harness.run_steps(2);
}

/// Grip of a layer or group: just left of its name.
fn grip(harness: &Harness<'_>, name: &str) -> Pos2 {
    harness.get_by_label(name).rect().left_center() - Vec2::new(10.0, 0.0)
}

/// Middle of the row of a layer, by its name, in the clip column.
fn row_point(harness: &Harness<'_>, name: &str, height: f32) -> Pos2 {
    let rect = harness.get_by_label(name).rect();
    // The name is on the first of three lines of a 64-point row.
    Pos2::new(rect.left() + 200.0, rect.top() - 4.0 + 64.0 * height)
}

/// The control panel of `panel`, themed, in a window of 1440 × 900.
fn harness<'a>(panel: &'a mut ControlPanel, status: &'a EngineStatus) -> Harness<'a> {
    harness_with(panel, status, false)
}

/// Like [`harness`]; with `twice`, every frame is laid out two times, as
/// egui does when a widget asks for it.
fn harness_with<'a>(
    panel: &'a mut ControlPanel,
    status: &'a EngineStatus,
    twice: bool,
) -> Harness<'a> {
    let mut themed = false;
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1440.0, 900.0))
        .with_pixels_per_point(2.0)
        // Frames close enough for a double click.
        .with_step_dt(0.05)
        .build_ui(move |ui| {
            if !themed {
                prism_ui::apply_theme(ui.ctx());
                themed = true;
                return;
            }
            let frame = FrameInfo {
                status,
                preview: Some((
                    egui::TextureId::Managed(0),
                    prism_core::Resolution {
                        width: 1920,
                        height: 1080,
                    },
                )),
                generator_previews: None,
                screens: &[],
                output_screens: &[],
                audio: AudioStatus::default(),
                midi: MidiStatus::default(),
            };
            panel.ui(ui, &frame);
            if twice && ui.ctx().current_pass_index() == 0 {
                ui.ctx().request_discard("test: two passes");
            }
        });
    harness.run_steps(2);
    harness
}

fn layer_ids(panel: &ControlPanel) -> Vec<LayerId> {
    let composition = panel.document().composition();
    composition
        .layers()
        .iter()
        .map(|layer| layer.id())
        .collect()
}

#[test]
fn layers_reorder_and_group_by_drag_and_drop() {
    let status = EngineStatus::default();
    let mut panel = ControlPanel::new(Document::new(Composition::default()), Vec::new());
    let ids = layer_ids(&panel);
    let mut harness = harness(&mut panel, &status);

    // Couche 1 dropped on the top edge of Couche 3: between 3 and 4.
    let from = grip(&harness, "Couche 1");
    let to = row_point(&harness, "Couche 3", 0.05);
    drag(&mut harness, from, to);
    // Couche 2 dropped on the middle of Couche 4: a group of the two.
    let from = grip(&harness, "Couche 2");
    let to = row_point(&harness, "Couche 4", 0.5);
    drag(&mut harness, from, to);
    drop(harness);

    let composition = panel.document().composition();
    let order: Vec<LayerId> = composition.layers().iter().map(|l| l.id()).collect();
    assert_eq!(order, [ids[2], ids[0], ids[3], ids[1]]);
    let group = composition.layer(ids[1]).unwrap().group().expect("grouped");
    assert_eq!(composition.group_layers(group), [ids[3], ids[1]]);
}

#[test]
fn a_double_click_renames_a_layer() {
    let status = EngineStatus::default();
    let mut panel = ControlPanel::new(Document::new(Composition::default()), Vec::new());
    let ids = layer_ids(&panel);
    let mut harness = harness(&mut panel, &status);

    // The whole name is selected: typing replaces it.
    let rename = |harness: &mut Harness<'_>, name: &str, text: &str, key: egui::Key| {
        let at = harness.get_by_label(name).rect().center();
        harness.hover_at(at);
        harness.run_steps(1);
        for pressed in [true, false, true, false] {
            harness.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        // The clicks, then the field and its focus.
        harness.run_steps(4);
        harness.event(egui::Event::Text(text.into()));
        harness.run_steps(1);
        harness.key_press(key);
        harness.run_steps(2);
    };
    rename(&mut harness, "Couche 2", "Lumières", egui::Key::Enter);
    // Escape keeps the old name.
    rename(&mut harness, "Couche 3", "Oublié", egui::Key::Escape);
    drop(harness);

    let composition = panel.document().composition();
    assert_eq!(composition.layer(ids[1]).unwrap().name, "Lumières");
    assert_eq!(composition.layer(ids[2]).unwrap().name, "Couche 3");
}

#[test]
fn the_inspector_edge_follows_the_pointer() {
    for twice in [false, true] {
        let status = EngineStatus::default();
        let mut panel = ControlPanel::new(Document::new(Composition::default()), Vec::new());
        let mut harness = harness_with(&mut panel, &status, twice);
        let before = harness.get_by_label("Programme").rect().left();
        // The inspector takes 42 % of the 1440-point window at first.
        let edge = Pos2::new(1440.0 * (1.0 - 0.42), 500.0);
        // A quick hand: already 20 points away on the first frame of the
        // drag.
        harness.hover_at(edge);
        harness.run_steps(1);
        harness.drag_at(edge);
        harness.run_steps(1);
        for x in [20.0, 35.0, 50.0] {
            harness.hover_at(edge - Vec2::new(x, 0.0));
            harness.run_steps(1);
        }
        harness.drop_at(edge - Vec2::new(50.0, 0.0));
        harness.run_steps(2);
        let after = harness.get_by_label("Programme").rect().left();
        assert!(
            (before - after - 50.0).abs() < 2.0,
            "two passes: {twice}, {before} → {after}"
        );
    }
}
