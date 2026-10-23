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

    let previews = generator_previews();
    let mut previews_texture: Option<egui::TextureHandle> = None;
    let mut capture = harness(Vec2::new(1440.0, 900.0), |ui| {
        if previews_texture.is_none()
            && let Some(image) = &previews
        {
            previews_texture = Some(ui.ctx().load_texture(
                "generator previews",
                image.clone(),
                Default::default(),
            ));
        }
        let frame = FrameInfo {
            status: &status,
            preview: None,
            generator_previews: previews_texture.as_ref().map(egui::TextureHandle::id),
            slot_preview: None,
            screens: &screens,
            output_screens: &[],
            audio: AudioStatus::default(),
            midi: MidiStatus::default(),
        };
        panel.ui(ui, &frame);
    });
    save(&mut capture, &folder, "controle");
    // A layer held over another: they would make a group.
    let from = capture.get_by_label("Couche 3").rect().left_center() - Vec2::new(10.0, 0.0);
    let to = capture.get_by_label("Couche 4").rect().left_top() + Vec2::new(260.0, 28.0);
    capture.hover_at(from);
    capture.run_steps(1);
    capture.drag_at(from);
    for step in 1..=6 {
        capture.hover_at(from + (to - from) * (step as f32 / 6.0));
        capture.run_steps(1);
    }
    save(&mut capture, &folder, "glisser");
    capture.key_press(egui::Key::Escape);
    capture.event(egui::Event::PointerButton {
        pos: from,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    capture.run_steps(2);
    capture.get_by_label("Dégradé").click();
    capture.run_steps(4);
    save(&mut capture, &folder, "clip");
    capture.get_by_label("Distorsion").click();
    capture.run_steps(4);
    save(&mut capture, &folder, "generateur");
    // The generator browser, its previews drawn by the engine.
    capture.get_by_label("Distorsion…").click();
    capture.run_steps(6);
    capture.get_by_label("Fluides et matières (9)").click();
    capture.run_steps(4);
    let family = capture.get_by_label("Fluides et matières (9)").rect();
    capture.hover_at(family.left_top() + Vec2::new(380.0, 200.0));
    capture.run_steps(4);
    save(&mut capture, &folder, "selecteur");
    capture.key_press(egui::Key::Escape);
    capture.run_steps(2);
    if let Some(close) = capture.query_by_label("Close window") {
        close.click();
    }
    capture.run_steps(4);
    capture.get_by_label("Dégradé").click();
    capture.run_steps(4);
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
    // The middle point of the Bézier warp, picked: its handles show.
    let point = egui::pos2(1016.0, 456.0);
    capture.hover_at(point);
    capture.run_steps(1);
    capture.drag_at(point);
    capture.run_steps(1);
    capture.drop_at(point);
    capture.run_steps(2);
    save(&mut capture, &folder, "bezier-clair");
    capture.get_by_label("Lumière (DMX)").click();
    capture.run_steps(4);
    save(&mut capture, &folder, "lumiere-clair");
    capture.get_by_label("Arche").click();
    capture.run_steps(4);
    save(&mut capture, &folder, "lumiere-courbe");
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
        (3, 3, "Distorsion", "domain_warp"),
    ];
    for (layer, column, name, kind) in clips {
        if let (Some(&layer), Some(&column)) = (layers.get(layer), columns.get(column)) {
            let mut clip = Clip::from_generator(name, kind);
            clip.effects.push(Effect::new("kaleidoscope"));
            let mut blur = Effect::new("blur");
            blur.collapsed = true;
            clip.effects.push(blur);
            let _ = composition.set_clip(Slot::new(layer, column), Some(clip));
        }
    }
    if let (Some(&layer), Some(&column)) = (layers.first(), columns.get(1)) {
        let _ = composition.trigger_clip(Slot::new(layer, column));
    }
    if let Some(&layer) = layers.first()
        && let Ok(group) = composition.group_with_above(layer)
        && let Some(group) = composition.group_mut(group)
    {
        group.name = "Fond".into();
        group.effects.push(Effect::new("color"));
    }
    if let Ok(screen) = composition.stage.add_screen("Façade", 1920, 1080)
        && let Some(slice) = composition
            .stage
            .screen_mut(screen)
            .and_then(|screen| screen.slices.first_mut())
    {
        // A Bézier warp, one handle pulled.
        let mut warp = prism_core::Warp::from_quad(&slice.output, 2, 2);
        warp.curve = prism_core::WarpCurve::Bezier;
        warp.ensure_handles();
        warp.handles_mut()[4].across = prism_core::Point::new(0.25, 0.12);
        slice.warp = Some(warp);
    }
    let _ = composition.stage.add_screen("Écran LED", 1280, 720);
    // A curved LED strip.
    let strip = composition.lighting.add_strip("Arche", 60);
    if let Some(strip) = composition.lighting.fixture_mut(strip) {
        strip.from = prism_core::Point::new(0.15, 0.8);
        strip.to = prism_core::Point::new(0.85, 0.8);
        strip.bend = Some([
            prism_core::Point::new(0.2, 0.1),
            prism_core::Point::new(0.8, 0.1),
        ]);
    }
    composition
}

/// Moving previews of the generators as the engine draws them, after a
/// second of play, read back as an image; `None` without a GPU.
fn generator_previews() -> Option<egui::ColorImage> {
    let gpu = std::sync::Arc::new(prism_render::Gpu::headless().ok()?);
    let mut state = prism_engine::EngineState::new(gpu.clone(), Composition::default());
    state.apply(prism_engine::Command::ShowGeneratorPreviews(true));
    let mut now = std::time::Instant::now();
    for _ in 0..60 {
        state.tick(now);
        let _ = gpu
            .device
            .poll(prism_render::wgpu::PollType::wait_indefinitely());
        now += std::time::Duration::from_secs_f64(1.0 / 60.0);
    }
    let image = state.generator_previews()?;
    let pixels = prism_render::read_texture(&gpu.device, &gpu.queue, image.texture()).ok()?;
    let size = image.resolution();
    let byte = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    Some(egui::ColorImage::new(
        [size.width as usize, size.height as usize],
        pixels
            .iter()
            .map(|[r, g, b, a]| {
                egui::Color32::from_rgba_premultiplied(byte(*r), byte(*g), byte(*b), byte(*a))
            })
            .collect(),
    ))
}
