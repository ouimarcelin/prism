//! The control panel drawn without window, frame by frame.

#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use egui::{Event, Key, Modifiers, RawInput, Rect, pos2, vec2};
use prism_core::{Clip, Composition, Slot};
use prism_engine::{ClipState, EngineStatus, LayerStatus};
use prism_ui::{Action, ControlPanel, Document, FrameInfo, Screen, Shortcut};

fn input(events: Vec<Event>) -> RawInput {
    RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1400.0, 900.0))),
        events,
        ..RawInput::default()
    }
}

fn key(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

/// Runs one frame and returns the actions.
fn frame(
    ctx: &egui::Context,
    panel: &mut ControlPanel,
    status: &EngineStatus,
    events: Vec<Event>,
) -> Vec<Action> {
    let screens = [
        Screen {
            name: "Intégré".into(),
            width: 2880,
            height: 1800,
            primary: true,
        },
        Screen {
            name: "Projecteur".into(),
            width: 1920,
            height: 1080,
            primary: false,
        },
    ];
    let info = FrameInfo {
        status,
        preview: None,
        screens: &screens,
        output_screen: Some(1),
    };
    let mut actions = Vec::new();
    let mut output = ctx.run_ui(input(events), |ui| actions = panel.ui(ui, &info));
    // No renderer here: textures are not uploaded.
    output.textures_delta.clear();
    actions
}

fn sample_document() -> (Document, Slot) {
    let mut composition = Composition::default();
    let slot = Slot::new(composition.layers()[0].id(), composition.columns()[0].id());
    composition
        .set_clip(slot, Some(Clip::from_file("/media/missing.mov")))
        .unwrap();
    composition.trigger_clip(slot).unwrap();
    (Document::new(composition), slot)
}

fn playing_status(slot: Slot) -> EngineStatus {
    EngineStatus {
        frames: 120,
        fps: 60.0,
        frame_time_ms: 3.5,
        late_ticks: 0,
        layers: vec![LayerStatus {
            layer: Some(slot.layer),
            slot: Some(slot),
            state: ClipState::Playing,
            position: 1.5,
            duration: 4.0,
            frame: Some(90),
            frame_count: 240,
            ..LayerStatus::default()
        }],
    }
}

#[test]
fn draws_every_part_without_panicking() {
    let (document, slot) = sample_document();
    let mut panel = ControlPanel::new(document, Vec::new());
    let ctx = egui::Context::default();
    let status = playing_status(slot);
    for _ in 0..3 {
        frame(&ctx, &mut panel, &status, Vec::new());
    }
    assert!(panel.title().starts_with("Prism – Sans titre"));
}

#[test]
fn digit_keys_trigger_columns_and_space_stops() {
    let (document, slot) = sample_document();
    let mut panel = ControlPanel::new(document, Vec::new());
    let ctx = egui::Context::default();
    let status = EngineStatus::default();
    frame(&ctx, &mut panel, &status, Vec::new());

    frame(&ctx, &mut panel, &status, vec![key(Key::Space)]);
    assert!(
        panel
            .document()
            .composition()
            .active_clip(slot.layer)
            .is_none()
    );

    let revision = panel.document().revision();
    frame(&ctx, &mut panel, &status, vec![key(Key::Num1)]);
    assert!(
        panel
            .document()
            .composition()
            .active_clip(slot.layer)
            .is_some()
    );
    assert!(panel.document().revision() > revision);
    // Playback is not an unsaved change.
    assert!(!panel.document().is_dirty());
}

#[test]
fn output_shortcuts_become_actions() {
    let (document, _) = sample_document();
    let mut panel = ControlPanel::new(document, Vec::new());
    let ctx = egui::Context::default();
    let status = EngineStatus::default();
    // Like the application, start with a frame without input: keys sent in
    // the very first frame of a context are not seen.
    frame(&ctx, &mut panel, &status, Vec::new());
    let actions = frame(&ctx, &mut panel, &status, vec![key(Key::F)]);
    assert!(matches!(actions.as_slice(), [Action::ToggleOutput]));
    let actions = frame(&ctx, &mut panel, &status, vec![key(Key::Escape)]);
    assert!(matches!(actions.as_slice(), [Action::CloseOutput]));

    // The same shortcuts typed in the output window.
    panel.shortcut(Shortcut::ToggleOutput);
    assert!(matches!(
        panel.take_actions().as_slice(),
        [Action::ToggleOutput]
    ));
}

#[test]
fn dropped_videos_go_to_the_selected_slot() {
    let mut panel = ControlPanel::new(Document::default(), Vec::new());
    panel.drop_files(&[PathBuf::from("/media/a.mov"), PathBuf::from("/media/b.mov")]);
    let composition = panel.document().composition();
    let top = composition.layers().last().unwrap().id();
    let names: Vec<_> = composition.columns()[..2]
        .iter()
        .map(|column| {
            composition
                .clip(Slot::new(top, column.id()))
                .unwrap()
                .name
                .clone()
        })
        .collect();
    assert_eq!(names, ["a", "b"]);
    assert!(panel.document().is_dirty());
    assert!(panel.title().ends_with("(modifié)"));
}

#[test]
fn quitting_with_unsaved_changes_asks_first() {
    let mut panel = ControlPanel::new(Document::default(), Vec::new());
    assert!(panel.request_quit());
    panel.drop_files(&[PathBuf::from("/media/a.mov")]);
    assert!(!panel.request_quit());
    // The confirmation is drawn without panicking.
    let ctx = egui::Context::default();
    frame(&ctx, &mut panel, &EngineStatus::default(), Vec::new());
}

#[test]
fn missing_media_dialog_is_drawn() {
    let (document, slot) = sample_document();
    let missing = document.missing_media();
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].slot, slot);
    let mut panel = ControlPanel::new(document, missing);
    let ctx = egui::Context::default();
    for _ in 0..2 {
        frame(&ctx, &mut panel, &EngineStatus::default(), Vec::new());
    }
}
