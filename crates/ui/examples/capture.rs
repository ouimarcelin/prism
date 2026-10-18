//! Renders the control and stage windows off screen, with a demo project,
//! and saves them as PNG images: a way to review the look of the interface
//! without running the application.
//!
//! `cargo run -p prism-ui --example capture -- [folder]` (default:
//! `target/captures`).

use std::path::PathBuf;

use egui::Vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use prism_core::{Clip, Composition, Effect, Slot};
use prism_engine::EngineStatus;
use prism_ui::{AudioStatus, ControlPanel, Document, FrameInfo, MidiStatus, Screen, StageFrame};

fn main() {
    let folder = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("target/captures"), PathBuf::from);
    std::fs::create_dir_all(&folder).expect("dossier des captures");

    let screens = [
        Screen {
            name: "Écran intégré".into(),
            width: 3024,
            height: 1964,
            primary: true,
        },
        Screen {
            name: "Projecteur".into(),
            width: 1920,
            height: 1080,
            primary: false,
        },
    ];
    let status = EngineStatus::default();
    let mut panel = ControlPanel::new(Document::new(demo()), Vec::new());

    let mut capture = harness(Vec2::new(1440.0, 900.0), |ui| {
        let frame = FrameInfo {
            status: &status,
            preview: None,
            screens: &screens,
            output_screens: &[],
            audio: AudioStatus::default(),
            midi: MidiStatus::default(),
        };
        panel.ui(ui, &frame);
    });
    save(&mut capture, &folder, "controle");
    capture.get_by_label("Dégradé").click();
    capture.run_steps(4);
    save(&mut capture, &folder, "clip");
    capture.get_by_label("Fichier").click();
    capture.run_steps(4);
    save(&mut capture, &folder, "menu");
    capture.get_by_label("Mode clair").click();
    capture.run_steps(4);
    save(&mut capture, &folder, "menu-clair");
    capture.key_press(egui::Key::Escape);
    capture.run_steps(4);
    save(&mut capture, &folder, "controle-clair");
    drop(capture);

    let mut capture = harness(Vec2::new(1280.0, 800.0), |ui| {
        let frame = StageFrame {
            composition: None,
            screens: &[],
            simulation: None,
            dmx: (None, 0),
            displays: &screens,
        };
        panel.stage_ui(ui, &frame);
    });
    save(&mut capture, &folder, "scene-clair");
    capture.get_by_label("Lumière (DMX)").click();
    capture.run_steps(4);
    save(&mut capture, &folder, "lumiere-clair");
}

fn harness<'a>(size: Vec2, mut app: impl FnMut(&mut egui::Ui) + 'a) -> Harness<'a> {
    // The theme's fonts load at the start of the next pass: the first pass
    // only applies it.
    let mut themed = false;
    let mut harness = Harness::builder()
        .with_size(size)
        .with_pixels_per_point(2.0)
        .wgpu()
        .build_ui(move |ui| {
            if themed {
                app(ui);
            } else {
                prism_ui::apply_theme(ui.ctx());
                themed = true;
            }
        });
    harness.run_steps(4);
    harness
}

fn save(harness: &mut Harness<'_>, folder: &std::path::Path, name: &str) {
    let path = folder.join(format!("{name}.png"));
    harness
        .render()
        .expect("rendu de l'interface")
        .save(&path)
        .expect("écriture de la capture");
    println!("{}", path.display());
}

/// A small show: four layers, a few generators and effects, one clip
/// playing, two screens.
fn demo() -> Composition {
    let mut composition = Composition::default();
    let layers: Vec<_> = composition
        .layers()
        .iter()
        .map(|layer| layer.id())
        .collect();
    let columns: Vec<_> = composition
        .columns()
        .iter()
        .map(|column| column.id())
        .collect();
    let clips = [
        (0, 0, "Dégradé", "gradient"),
        (0, 1, "Anneaux", "rings"),
        (0, 2, "Lignes", "lines"),
        (1, 0, "Damier", "checker"),
        (1, 3, "Bruit", "noise"),
        (2, 1, "Aplat", "solid"),
        (2, 2, "Anneaux lents", "rings"),
    ];
    for (layer, column, name, kind) in clips {
        if let (Some(&layer), Some(&column)) = (layers.get(layer), columns.get(column)) {
            let mut clip = Clip::from_generator(name, kind);
            clip.effects.push(Effect::new("kaleidoscope"));
            let _ = composition.set_clip(Slot::new(layer, column), Some(clip));
        }
    }
    if let (Some(&layer), Some(&column)) = (layers.first(), columns.get(1)) {
        let _ = composition.trigger_clip(Slot::new(layer, column));
    }
    let _ = composition.stage.add_screen("Façade", 1920, 1080);
    let _ = composition.stage.add_screen("Écran LED", 1280, 720);
    composition
}
