//! Save and load of project files on disk.

#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::Path;

use prism_core::{
    BlendMode, Clip, Composition, CrossfaderSide, Direction, FORMAT_VERSION, LoopMode, Project,
    ProjectError, Slot, Unit,
};

fn slot(comp: &Composition, layer: usize, column: usize) -> Slot {
    Slot::new(comp.layers()[layer].id(), comp.columns()[column].id())
}

/// A project with media inside and outside `dir`, and non-default settings.
fn sample_project(dir: &Path) -> Project {
    let mut comp = Composition::default();
    let inside = slot(&comp, 0, 0);
    let outside = slot(&comp, 2, 3);

    let mut clip = Clip::from_file(dir.join("media").join("tunnel.mov"));
    clip.playback.direction = Direction::Backward;
    clip.playback.loop_mode = LoopMode::PingPong;
    clip.playback.speed = 1.5;
    clip.playback.cue_points[2] = Some(4.25);
    comp.set_clip(inside, Some(clip)).unwrap();
    comp.set_clip(outside, Some(Clip::from_file("/Volumes/Show/strobe.mov")))
        .unwrap();

    let layer = comp.layer_mut(inside.layer).unwrap();
    layer.opacity = Unit::new(0.75);
    layer.blend_mode = BlendMode::Screen;
    layer.crossfader_side = Some(CrossfaderSide::A);
    layer.transition_ms = 250;
    comp.crossfader.position = Unit::new(0.2);

    Project::new(comp)
}

#[test]
fn save_then_load_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("show.prism");
    let project = sample_project(dir.path());

    project.save(&file).unwrap();
    let loaded = Project::load(&file).unwrap();

    assert_eq!(loaded, project);
}

#[test]
fn media_inside_project_folder_is_stored_relative() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("show.prism");
    sample_project(dir.path()).save(&file).unwrap();

    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(json["format_version"], FORMAT_VERSION);
    let paths: Vec<_> = json["composition"]["layers"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|layer| layer["clips"].as_array().unwrap())
        .filter(|clip| !clip.is_null())
        .map(|clip| clip["source"]["path"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(paths, ["media/tunnel.mov", "/Volumes/Show/strobe.mov"]);
}

#[test]
fn moved_project_folder_still_finds_its_media() {
    let dir = tempfile::tempdir().unwrap();
    let original = dir.path().join("original");
    let moved = dir.path().join("moved");
    fs::create_dir_all(original.join("media")).unwrap();
    fs::write(original.join("media").join("tunnel.mov"), b"").unwrap();

    let mut project = sample_project(&original);
    let outside = slot(&project.composition, 2, 3);
    project.composition.set_clip(outside, None).unwrap();
    project.save(original.join("show.prism")).unwrap();

    fs::rename(&original, &moved).unwrap();
    let loaded = Project::load(moved.join("show.prism")).unwrap();

    assert!(loaded.missing_media().is_empty());
    let inside = slot(&loaded.composition, 0, 0);
    assert_eq!(
        loaded.composition.clip(inside).unwrap().source.path(),
        Some(moved.join("media").join("tunnel.mov").as_path())
    );
}

#[test]
fn missing_media_is_reported_with_its_slot() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("media")).unwrap();
    fs::write(dir.path().join("media").join("tunnel.mov"), b"").unwrap();
    let project = sample_project(dir.path());

    let missing = project.missing_media();

    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].slot, slot(&project.composition, 2, 3));
    assert_eq!(missing[0].path, Path::new("/Volumes/Show/strobe.mov"));
}

#[test]
fn playing_state_is_not_saved() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("show.prism");
    let mut project = sample_project(dir.path());
    let first = project.composition.columns()[0].id();
    project.composition.trigger_column(first).unwrap();

    project.save(&file).unwrap();
    let loaded = Project::load(&file).unwrap();

    assert!(
        loaded
            .composition
            .layers()
            .iter()
            .all(|layer| layer.active_column().is_none())
    );
}

#[test]
fn newer_format_version_is_rejected() {
    let json = format!(
        r#"{{"format_version": {}, "composition": {{}}}}"#,
        FORMAT_VERSION + 1
    );
    match Project::from_json(&json, None) {
        Err(ProjectError::UnsupportedVersion { found, supported }) => {
            assert_eq!(found, u64::from(FORMAT_VERSION + 1));
            assert_eq!(supported, FORMAT_VERSION);
        }
        other => panic!("expected UnsupportedVersion, got {other:?}"),
    }
}

#[test]
fn missing_format_version_is_rejected() {
    assert!(matches!(
        Project::from_json(r#"{"composition": {}}"#, None),
        Err(ProjectError::MissingVersion)
    ));
}

#[test]
fn failed_save_keeps_the_previous_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("show.prism");
    let project = sample_project(dir.path());
    project.save(&file).unwrap();
    let before = fs::read_to_string(&file).unwrap();

    // Saving into a folder that does not exist fails before touching anything.
    let error = project.save(dir.path().join("absent").join("show.prism"));
    assert!(matches!(error, Err(ProjectError::Io { .. })));

    assert_eq!(fs::read_to_string(&file).unwrap(), before);
    let leftovers: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .filter(|name| name.to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "temporary files left: {leftovers:?}");
}

#[test]
fn effect_files_inside_project_folder_are_stored_relative() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("show.prism");
    let mut project = sample_project(dir.path());
    let comp = &mut project.composition;
    let mut shader = prism_core::Effect::new("isf");
    shader.file = Some(dir.path().join("shaders").join("waves.fs"));
    comp.effects.push(shader.clone());
    let mut generator = Clip::from_generator("Shader", "isf");
    *generator.source.generator_mut().unwrap() = shader;
    comp.set_clip(slot(comp, 1, 1), Some(generator)).unwrap();
    let mut outside = prism_core::Effect::new("isf");
    outside.file = Some("/Shaders/glow.fs".into());
    comp.clip_mut(slot(comp, 0, 0))
        .unwrap()
        .effects
        .push(outside);
    project.save(&file).unwrap();

    let json = fs::read_to_string(&file).unwrap();
    assert_eq!(
        json.matches(r#""file": "shaders/waves.fs""#).count(),
        2,
        "{json}"
    );
    assert!(json.contains(r#""file": "/Shaders/glow.fs""#), "{json}");
    assert_eq!(Project::load(&file).unwrap(), project);
}
