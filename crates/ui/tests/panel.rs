//! The control panel drawn without window, frame by frame.

#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use egui::{Event, Key, Modifiers, RawInput, Rect, pos2, vec2};
use prism_core::{Clip, Composition, Effect, Slot};
use prism_engine::{ClipState, Command, EngineStatus, LayerStatus};
use prism_ui::{Action, ControlPanel, Document, FrameInfo, Screen, Shortcut};

/// A context with Prism's theme, as the application makes it.
fn context() -> egui::Context {
    let ctx = egui::Context::default();
    prism_ui::apply_theme(&ctx);
    ctx
}

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
        generator_previews: None,
        screens: &screens,
        output_screens: &[1],
        audio: Default::default(),
        midi: Default::default(),
    };
    let mut actions = Vec::new();
    let mut output = ctx.run_ui(input(events), |ui| actions.extend(panel.ui(ui, &info)));
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
        worst_interval_ms: 17.0,
        hitches: 0,
        worst_frame_time_ms: 5.0,
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
        ..EngineStatus::default()
    }
}

#[test]
fn draws_every_part_without_panicking() {
    let (document, slot) = sample_document();
    let mut panel = ControlPanel::new(document, Vec::new());
    let ctx = context();
    let status = playing_status(slot);
    for _ in 0..3 {
        frame(&ctx, &mut panel, &status, Vec::new());
    }
    assert!(panel.title().starts_with("Prism – Sans titre"));
}

#[test]
fn draws_effect_chains_without_changing_them() {
    let (mut document, slot) = sample_document();
    let composition = document.composition_mut();
    let mut blur = Effect::new("blur");
    blur.set_param("radius", 12.0);
    composition.clip_mut(slot).unwrap().effects = vec![blur, Effect::new("mirror")];
    let mut bypassed = Effect::new("color");
    bypassed.bypass = true;
    composition.layer_mut(slot.layer).unwrap().effects = vec![bypassed];
    composition.effects = vec![Effect::new("from a later version")];
    let saved = composition.clone();
    let document = Document::new(saved.clone());

    let mut panel = ControlPanel::new(document, Vec::new());
    let ctx = context();
    let status = playing_status(slot);
    for _ in 0..3 {
        frame(&ctx, &mut panel, &status, Vec::new());
    }
    // Showing settings does not write their defaults into the project.
    assert_eq!(panel.document().composition(), &saved);
    assert!(!panel.document().is_dirty());
}

#[test]
fn digit_keys_trigger_columns_and_space_stops() {
    let (document, slot) = sample_document();
    let mut panel = ControlPanel::new(document, Vec::new());
    let ctx = context();
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
    let ctx = context();
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
    let ctx = context();
    frame(&ctx, &mut panel, &EngineStatus::default(), Vec::new());
}

#[test]
fn missing_media_dialog_is_drawn() {
    let (document, slot) = sample_document();
    let missing = document.missing_media();
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].slot, slot);
    let mut panel = ControlPanel::new(document, missing);
    let ctx = context();
    for _ in 0..2 {
        frame(&ctx, &mut panel, &EngineStatus::default(), Vec::new());
    }
}

fn released(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

fn key_with(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

#[test]
fn command_q_quits_or_asks_first() {
    let ctx = context();
    let status = EngineStatus::default();
    let mut panel = ControlPanel::new(Document::default(), Vec::new());
    frame(&ctx, &mut panel, &status, Vec::new());
    let actions = frame(
        &ctx,
        &mut panel,
        &status,
        vec![key_with(Key::Q, Modifiers::COMMAND)],
    );
    assert!(matches!(actions.as_slice(), [Action::Quit]));

    // With unsaved changes, the confirmation comes first.
    panel.drop_files(&[PathBuf::from("/media/a.mov")]);
    let actions = frame(
        &ctx,
        &mut panel,
        &status,
        vec![key_with(Key::Q, Modifiers::COMMAND)],
    );
    assert!(actions.is_empty());
    panel.shortcut(Shortcut::Quit);
    assert!(panel.take_actions().is_empty());
}

#[test]
fn crossfader_moves_on_its_own_when_asked() {
    use prism_ui::Preferences;

    let preferences = Preferences {
        crossfade_seconds: 0.5,
        ..Preferences::default()
    };
    let mut panel =
        ControlPanel::new(Document::default(), Vec::new()).with_preferences(preferences, None);
    let ctx = context();
    let status = EngineStatus::default();
    frame(&ctx, &mut panel, &status, Vec::new());
    assert!(!panel.is_animating());

    // Click "B ▶": find it by its label.
    let clicked = click_button(&ctx, &mut panel, &status, "B ▶");
    assert!(clicked);
    assert!(panel.is_animating());
    for _ in 0..120 {
        frame(&ctx, &mut panel, &status, Vec::new());
        if !panel.is_animating() {
            break;
        }
    }
    assert!(!panel.is_animating());
    assert_eq!(
        panel.document().composition().crossfader.position.get(),
        1.0
    );
}

/// Clicks the first widget labelled `text`. Returns whether one was found.
fn click_button(
    ctx: &egui::Context,
    panel: &mut ControlPanel,
    status: &EngineStatus,
    text: &str,
) -> bool {
    // Draw a frame and find the text shape of the widget.
    let mut target = None;
    let screens: [Screen; 0] = [];
    let info = FrameInfo {
        status,
        preview: None,
        generator_previews: None,
        screens: &screens,
        output_screens: &[],
        audio: Default::default(),
        midi: Default::default(),
    };
    let mut output = ctx.run_ui(input(Vec::new()), |ui| {
        panel.ui(ui, &info);
    });
    output.textures_delta.clear();
    for shape in &output.shapes {
        if let egui::epaint::Shape::Text(text_shape) = &shape.shape
            && text_shape.galley.text() == text
        {
            target = Some(text_shape.visual_bounding_rect().center());
        }
    }
    let Some(position) = target else {
        return false;
    };
    let press = |pressed| Event::PointerButton {
        pos: position,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    frame(ctx, panel, status, vec![Event::PointerMoved(position)]);
    frame(ctx, panel, status, vec![press(true)]);
    frame(ctx, panel, status, vec![press(false)]);
    true
}

#[test]
fn tap_tempo_and_resync_reach_the_engine() {
    let (document, _) = sample_document();
    let mut panel = ControlPanel::new(document, Vec::new());
    let ctx = context();
    let status = EngineStatus::default();
    frame(&ctx, &mut panel, &status, Vec::new());

    // Each tap presses and releases T: a key pressed again without being
    // released counts as an automatic repeat.
    let tap = || vec![key(Key::T), released(Key::T)];

    // A first tap puts a beat on itself at the current tempo.
    let actions = frame(&ctx, &mut panel, &status, tap());
    assert!(
        matches!(actions.as_slice(), [Action::Engine(Command::Tap { bpm, .. })] if *bpm == 120.0),
        "{actions:?}"
    );
    // A second tap, a few milliseconds later: far too fast, the tempo stops
    // at the maximum, in the document and in the command.
    let actions = frame(&ctx, &mut panel, &status, tap());
    assert!(
        matches!(actions.as_slice(), [Action::Engine(Command::Tap { bpm, .. })] if *bpm == prism_core::MAX_BPM),
        "{actions:?}"
    );
    assert_eq!(
        panel.document().composition().tempo.get(),
        prism_core::MAX_BPM
    );
    assert!(panel.document().is_dirty());

    let actions = frame(&ctx, &mut panel, &status, vec![key(Key::R)]);
    assert!(
        matches!(actions.as_slice(), [Action::Engine(Command::Resync { .. })]),
        "{actions:?}"
    );
}

#[test]
fn the_midi_clock_drives_the_tempo_when_followed() {
    let (document, _) = sample_document();
    let reading = |bpm, starts| prism_control::ClockReading {
        bpm,
        starts,
        timecode: None,
    };
    let mut panel = ControlPanel::new(document, Vec::new());
    // Not followed: nothing changes.
    assert!(panel.follow_clock(reading(Some(128.04), 1)).is_empty());
    assert_eq!(panel.document().composition().tempo.get(), 120.0);

    let preferences = prism_ui::Preferences {
        midi_clock: true,
        ..prism_ui::Preferences::default()
    };
    let mut panel =
        ControlPanel::new(sample_document().0, Vec::new()).with_preferences(preferences, None);
    let actions = panel.follow_clock(reading(Some(128.04), 0));
    assert!(actions.is_empty());
    assert_eq!(panel.document().composition().tempo.get(), 128.0);
    // A start begins a bar.
    let actions = panel.follow_clock(reading(Some(128.0), 1));
    assert!(matches!(
        actions.as_slice(),
        [Action::Engine(Command::Resync { .. })]
    ));
    // The tempo is not an edit to undo.
    assert!(!panel.document().can_undo());
}

/// The criterion of phase 5, second half: a clip starts at the right
/// timecode.
#[test]
fn a_cue_starts_its_column_when_the_timecode_passes_it() {
    use prism_clock::{Timecode, TimecodeRate};
    let (mut document, slot) = sample_document();
    document.composition_mut().stop_all();
    let at = |text| Timecode::parse(text, TimecodeRate::Fps25).unwrap();
    document.composition_mut().timeline = prism_core::Timeline {
        enabled: true,
        source: prism_core::TimecodeSource::Midi,
        cues: vec![prism_core::Cue {
            at: at("01:00:05:00").seconds(),
            target: prism_core::CueTarget::Column {
                column: slot.column,
            },
        }],
    };
    let mut panel = ControlPanel::new(document, Vec::new());
    let playing = |panel: &ControlPanel| {
        panel
            .document()
            .composition()
            .active_clip(slot.layer)
            .is_some()
    };
    panel.follow_timecode(Some(at("01:00:04:20")));
    panel.follow_timecode(Some(at("01:00:04:24")));
    assert!(!playing(&panel), "one frame early");
    panel.follow_timecode(Some(at("01:00:05:00")));
    assert!(playing(&panel), "on time");
    // A jump back over the cue does not launch it again.
    panel.shortcut(Shortcut::StopAll);
    panel.follow_timecode(Some(at("01:00:04:00")));
    panel.follow_timecode(None);
    panel.follow_timecode(Some(at("01:00:06:00")));
    assert!(!playing(&panel));
}
