//! Menus of the menu bar, clicked through the accessibility tree.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use prism_core::Composition;
use prism_engine::EngineStatus;
use prism_ui::{AudioStatus, ControlPanel, Document, FrameInfo, MidiStatus};

#[test]
fn settings_menus_stay_open_on_options() {
    let status = EngineStatus::default();
    let mut panel = ControlPanel::new(Document::new(Composition::default()), Vec::new());
    let mut themed = false;
    let mut harness = Harness::new_ui(|ui| {
        if !themed {
            prism_ui::apply_theme(ui.ctx());
            themed = true;
            return;
        }
        let frame = FrameInfo {
            status: &status,
            preview: None,
            generator_previews: None,
            slot_preview: None,
            screens: &[],
            output_screens: &[],
            audio: AudioStatus::default(),
            midi: MidiStatus::default(),
        };
        panel.ui(ui, &frame);
    });
    harness.run_steps(2);

    harness.get_by_label("Contrôle").click();
    harness.run_steps(2);
    harness.get_by_label("Art-Net").click();
    harness.run_steps(2);
    assert!(
        harness.query_by_label("Correspondances…").is_some(),
        "the menu is still open"
    );

    harness.get_by_label("Correspondances…").click();
    harness.run_steps(2);
    assert!(
        harness.query_by_label("Art-Net").is_none(),
        "an action closes the menu"
    );
}
