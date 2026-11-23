//! The projector and the 3D objects of the selected screen, in the stage
//! editor.

use egui::RichText;
use prism_core::{MeshSource, Object3d};

use crate::panel::ControlPanel;
use crate::theme::Emphasis;

/// Three numbers side by side, dragged.
fn vector(ui: &mut egui::Ui, value: &mut [f32; 3], speed: f64, suffix: &str) {
    ui.horizontal(|ui| {
        for (component, axis) in value.iter_mut().zip(["x ", "y ", "z "]) {
            ui.add(
                egui::DragValue::new(component)
                    .speed(speed)
                    .prefix(axis)
                    .suffix(suffix),
            );
        }
    });
}

/// Name of a shape.
fn mesh_label(mesh: &MeshSource) -> String {
    match mesh {
        MeshSource::Cube => "Cube".to_owned(),
        MeshSource::Plane => "Plan".to_owned(),
        MeshSource::File { path } => path.file_name().map_or_else(
            || "Fichier OBJ".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        ),
    }
}

impl ControlPanel {
    /// The projector of the selected screen, then its objects.
    pub(crate) fn stage_objects(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.stage.screen else {
            return;
        };
        let Some(screen) = self.document.composition_mut().stage.screen_mut(id) else {
            return;
        };
        ui.label(RichText::new("Objets 3D").semibold())
            .on_hover_text("Des volumes vus depuis le projecteur de cet écran, couverts par la composition selon leurs coordonnées de texture, dessinés par-dessus les découpes");
        egui::CollapsingHeader::new("Projecteur")
            .id_salt(("projector", id))
            .show(ui, |ui| {
                let projector = &mut screen.projector;
                egui::Grid::new("projector")
                    .num_columns(2)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        ui.label("Position");
                        vector(ui, &mut projector.position, 0.01, " m");
                        ui.end_row();
                        ui.label("Visée");
                        vector(ui, &mut projector.target, 0.01, " m");
                        ui.end_row();
                        ui.label("Champ")
                            .on_hover_text("Angle vertical de l'image projetée");
                        ui.add(
                            egui::DragValue::new(&mut projector.fov)
                                .range(1.0..=170.0)
                                .speed(0.1)
                                .suffix("°"),
                        );
                        ui.end_row();
                        ui.label("Décentrement")
                            .on_hover_text("Décalage d'objectif, en fractions de l'image");
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::DragValue::new(&mut projector.shift[0])
                                    .range(-2.0..=2.0)
                                    .speed(0.005)
                                    .prefix("x "),
                            );
                            ui.add(
                                egui::DragValue::new(&mut projector.shift[1])
                                    .range(-2.0..=2.0)
                                    .speed(0.005)
                                    .prefix("y "),
                            );
                        });
                        ui.end_row();
                    });
            });
        let mut remove = None;
        for (index, object) in screen.objects.iter_mut().enumerate() {
            ui.push_id(("object", index), |ui| {
                egui::CollapsingHeader::new(format!("{} · {}", object.name, mesh_label(&object.mesh)))
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::Grid::new("object").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                            ui.label("Nom");
                            ui.horizontal(|ui| {
                                ui.add(egui::TextEdit::singleline(&mut object.name).desired_width(120.0));
                                ui.checkbox(&mut object.enabled, "affiché");
                                if ui.small_button("×").on_hover_text("Retirer l'objet").clicked() {
                                    remove = Some(index);
                                }
                            });
                            ui.end_row();
                            ui.label("Position");
                            vector(ui, &mut object.position, 0.01, " m");
                            ui.end_row();
                            ui.label("Rotation");
                            vector(ui, &mut object.rotation, 0.5, "°");
                            ui.end_row();
                            ui.label("Échelle");
                            ui.add(egui::DragValue::new(&mut object.scale).range(0.001..=1000.0).speed(0.01));
                            ui.end_row();
                            ui.label("Zone").on_hover_text("Partie de la composition posée sur l'objet : gauche, haut, largeur, hauteur");
                            ui.horizontal(|ui| {
                                for (value, name) in object.area.iter_mut().zip(["g ", "h ", "l ", "h "]) {
                                    ui.add(egui::DragValue::new(value).range(0.0..=1.0).speed(0.005).prefix(name));
                                }
                            });
                            ui.end_row();
                            ui.label("Opacité");
                            ui.add(crate::slider::Slider::new(&mut object.opacity, 0.0..=1.0).quarters());
                            ui.end_row();
                        });
                    });
            });
        }
        if let Some(index) = remove {
            screen.objects.remove(index);
        }
        ui.horizontal_wrapped(|ui| {
            let count = screen.objects.len() + 1;
            if ui.button("+ Cube").clicked() {
                screen.objects.push(Object3d::new(format!("Objet {count}"), MeshSource::Cube));
            }
            if ui.button("+ Plan").clicked() {
                screen.objects.push(Object3d::new(format!("Objet {count}"), MeshSource::Plane));
            }
            if ui
                .button("+ Fichier OBJ…")
                .on_hover_text("Un modèle 3D au format Wavefront OBJ ; ses coordonnées de texture placent la composition")
                .clicked()
                && let Some(path) = rfd::FileDialog::new().add_filter("Modèle 3D", &["obj"]).pick_file()
            {
                screen.objects.push(Object3d::new(format!("Objet {count}"), MeshSource::File { path }));
            }
        });
    }
}
