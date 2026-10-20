//! The lighting tab of the stage window: fixtures (LED strips, lights)
//! placed over the composition, whose colours are sent as DMX.

use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use prism_core::{ColorOrder, DmxProtocol, FixtureId, MAX_FIXTURE_PIXELS, Point};

use crate::panel::ControlPanel;
use crate::slider::Slider;
use crate::stage::StageFrame;
use crate::theme::{self, Emphasis};

const SELECTED: Color32 = theme::DARK.brand;
const OTHER: Color32 = theme::DARK.success_text;
/// Control points of a curved fixture.
const TANGENT: Color32 = theme::DARK.warning;
/// Straight pieces a curved fixture is drawn and picked with.
const PATH_SEGMENTS: usize = 48;

/// Distance from `point` to the line of `fixture`, on the screen.
fn path_distance(point: Pos2, image: Rect, fixture: &prism_core::Fixture) -> f32 {
    fixture
        .path(PATH_SEGMENTS)
        .windows(2)
        .map(|pair| segment_distance(point, to_screen(image, pair[0]), to_screen(image, pair[1])))
        .fold(f32::INFINITY, f32::min)
}

fn to_screen(rect: Rect, point: Point) -> Pos2 {
    rect.min + Vec2::new(point.x * rect.width(), point.y * rect.height())
}

fn to_fraction(rect: Rect, position: Pos2) -> Point {
    Point::new(
        ((position.x - rect.min.x) / rect.width().max(1.0)).clamp(0.0, 1.0),
        ((position.y - rect.min.y) / rect.height().max(1.0)).clamp(0.0, 1.0),
    )
}

/// Distance from `point` to the segment from `a` to `b`.
pub(crate) fn segment_distance(point: Pos2, a: Pos2, b: Pos2) -> f32 {
    let edge = b - a;
    let t = ((point - a).dot(edge) / edge.length_sq().max(1e-6)).clamp(0.0, 1.0);
    (point - (a + edge * t)).length()
}

impl ControlPanel {
    /// Shows the lighting tab of the stage window, or its screens.
    pub fn show_lighting(&mut self, on: bool) {
        self.stage.lighting = on;
    }

    /// The lighting tab: DMX settings, fixtures, and the composition with
    /// the fixtures over it.
    pub(crate) fn lighting_ui(&mut self, ui: &mut egui::Ui, frame: &StageFrame<'_>) {
        let style = ui.style().clone();
        egui::Panel::top("lighting settings")
            .frame(theme::bar_frame(&style))
            .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let lighting = &mut self.document.composition_mut().lighting;
                ui.checkbox(&mut lighting.enabled, RichText::new("Envoyer le DMX").semibold());
                egui::ComboBox::from_id_salt("dmx protocol")
                    .selected_text(match lighting.protocol {
                        DmxProtocol::ArtNet => "Art-Net",
                        DmxProtocol::Sacn => "sACN",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut lighting.protocol, DmxProtocol::ArtNet, "Art-Net");
                        ui.selectable_value(&mut lighting.protocol, DmxProtocol::Sacn, "sACN");
                    });
                ui.label("vers");
                ui.add(
                    egui::TextEdit::singleline(&mut lighting.target)
                        .hint_text("tout le réseau")
                        .desired_width(150.0),
                )
                .on_hover_text("Adresse IP du contrôleur de LED ou du nœud DMX (« IP » ou « IP:port ») ; vide : diffusion (Art-Net) ou multicast (sACN)");
                ui.label("cadence");
                ui.add(Slider::new(&mut lighting.rate, 1.0..=60.0).suffix(" /s"));
                match &frame.dmx.0 {
                    Some(error) => {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    }
                    None if lighting.enabled => {
                        ui.label(RichText::new(format!("{} univers envoyés", frame.dmx.1)).weak());
                    }
                    None => {}
                }
            });
        });
        egui::Panel::left("lighting fixtures")
            .frame(theme::side_frame(&style))
            .resizable(true)
            .default_size(260.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.fixtures_ui(ui));
            });
        egui::CentralPanel::default()
            .frame(theme::canvas_frame(&style))
            .show(ui, |ui| self.fixtures_view(ui, frame));
    }

    fn fixtures_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Luminaires").semibold());
            if ui
                .button("+ Bande")
                .on_hover_text("Une bande de LED RVB de 60 pixels, après les canaux déjà pris")
                .clicked()
            {
                let lighting = &mut self.document.composition_mut().lighting;
                let count = lighting.fixtures().len();
                self.stage.fixture = Some(lighting.add_strip(format!("Bande {}", count + 1), 60));
            }
        });
        let lighting = &mut self.document.composition_mut().lighting;
        let ids: Vec<(FixtureId, String, bool)> = lighting
            .fixtures()
            .iter()
            .map(|fixture| (fixture.id(), fixture.name.clone(), fixture.enabled))
            .collect();
        let mut removed = None;
        for (id, name, mut enabled) in ids {
            ui.horizontal(|ui| {
                if ui.checkbox(&mut enabled, "").changed()
                    && let Some(fixture) = lighting.fixture_mut(id)
                {
                    fixture.enabled = enabled;
                }
                if ui
                    .selectable_label(self.stage.fixture == Some(id), name)
                    .clicked()
                {
                    self.stage.fixture = Some(id);
                }
                if ui.small_button("×").on_hover_text("Supprimer").clicked() {
                    removed = Some(id);
                }
            });
        }
        if let Some(id) = removed {
            lighting.remove(id);
        }
        let Some(fixture) = self.stage.fixture.and_then(|id| lighting.fixture_mut(id)) else {
            return;
        };
        ui.separator();
        ui.add(egui::TextEdit::singleline(&mut fixture.name));
        egui::Grid::new("fixture").num_columns(2).show(ui, |ui| {
            ui.label("Pixels");
            ui.add(egui::DragValue::new(&mut fixture.pixels).range(1..=MAX_FIXTURE_PIXELS));
            ui.end_row();
            ui.label("Couleurs");
            egui::ComboBox::from_id_salt("fixture order")
                .selected_text(match fixture.order {
                    ColorOrder::Rgb => "RVB",
                    ColorOrder::Grb => "VRB (GRB)",
                    ColorOrder::Rgbw => "RVB + blanc",
                    ColorOrder::Dimmer => "Gradateur",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut fixture.order, ColorOrder::Rgb, "RVB");
                    ui.selectable_value(&mut fixture.order, ColorOrder::Grb, "VRB (GRB)");
                    ui.selectable_value(&mut fixture.order, ColorOrder::Rgbw, "RVB + blanc");
                    ui.selectable_value(&mut fixture.order, ColorOrder::Dimmer, "Gradateur");
                });
            ui.end_row();
            ui.label("Univers");
            ui.add(egui::DragValue::new(&mut fixture.universe).range(0..=32767));
            ui.end_row();
            ui.label("Canal");
            ui.add(egui::DragValue::new(&mut fixture.channel).range(1..=512))
                .on_hover_text("Canal du premier pixel ; les pixels qui ne tiennent plus passent à l'univers suivant");
            ui.end_row();
            ui.label("Luminosité");
            ui.add(Slider::new(&mut fixture.brightness, 0.0..=1.0).quarters());
            ui.end_row();
            ui.label("Forme");
            let mut curved = fixture.bend.is_some();
            if ui
                .checkbox(&mut curved, "Courbe (Bézier)")
                .on_hover_text("La ligne devient une courbe : deux poignées jaunes la plient, les pixels restent également espacés le long de la courbe")
                .changed()
            {
                if curved {
                    fixture.bend_straight();
                } else {
                    fixture.bend = None;
                }
            }
            ui.end_row();
            ui.label("Gamma");
            ui.add(Slider::new(&mut fixture.gamma, 1.0..=3.0).snaps([2.2]))
                .on_hover_text("1 : valeurs de l'image telles quelles ; 2,2 environ pour des LED dont le contrôleur ne corrige pas (tons moyens trop clairs sinon)");
            ui.end_row();
        });
        let channels = usize::from(fixture.pixels) * fixture.order.channels();
        ui.label(
            RichText::new(format!(
                "{channels} canaux, à partir de l'univers {} canal {}",
                fixture.universe, fixture.channel
            ))
            .weak(),
        );
    }

    fn fixtures_view(&mut self, ui: &mut egui::Ui, frame: &StageFrame<'_>) {
        let space = ui.available_rect_before_wrap();
        let resolution = self.document.composition().resolution();
        let aspect = resolution.width as f32 / resolution.height.max(1) as f32;
        let size = if space.width() / space.height().max(1.0) > aspect {
            Vec2::new(space.height() * aspect, space.height())
        } else {
            Vec2::new(space.width(), space.width() / aspect)
        };
        let image = Rect::from_center_size(space.center(), size);
        ui.painter().rect_filled(image, 0.0, Color32::BLACK);
        if let Some((texture, _)) = frame.composition {
            ui.painter().image(
                texture,
                image,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        ui.allocate_rect(space, Sense::hover());

        let lighting = &self.document.composition().lighting;
        for fixture in lighting.fixtures() {
            let color = if Some(fixture.id()) == self.stage.fixture {
                SELECTED
            } else {
                OTHER.gamma_multiply(if fixture.enabled { 1.0 } else { 0.4 })
            };
            let path = fixture.path(PATH_SEGMENTS);
            ui.painter().add(egui::Shape::line(
                path.into_iter()
                    .map(|point| to_screen(image, point))
                    .collect(),
                Stroke::new(1.5, color),
            ));
            // Pixels as dots, at most a few hundred drawn.
            let step = (usize::from(fixture.pixels) / 300).max(1);
            for point in fixture.pixel_points().into_iter().step_by(step) {
                ui.painter()
                    .circle_filled(to_screen(image, point), 2.0, color);
            }
        }

        // A click near a line picks its fixture.
        if ui.input(|input| input.pointer.primary_pressed())
            && let Some(position) = ui.input(|input| input.pointer.interact_pos())
            && image.contains(position)
        {
            let picked = lighting
                .fixtures()
                .iter()
                .rev()
                .find(|fixture| path_distance(position, image, fixture) < 8.0);
            if let Some(fixture) = picked
                && self.stage.fixture != Some(fixture.id())
            {
                self.stage.fixture = Some(fixture.id());
                return;
            }
        }

        let Some(id) = self.stage.fixture else {
            return;
        };
        let Some(fixture) = self.document.composition_mut().lighting.fixture_mut(id) else {
            return;
        };
        let base = ui.id().with(("fixture", id.0));
        // The line: drag to move the whole fixture.
        let path: Vec<Pos2> = fixture
            .path(PATH_SEGMENTS)
            .into_iter()
            .map(|point| to_screen(image, point))
            .collect();
        let line = Rect::from_points(&path).expand(6.0);
        let moving = ui.interact(line, base.with("move"), Sense::drag());
        if moving.dragged()
            && moving.interact_pointer_pos().is_some_and(|position| {
                path_distance(position - moving.drag_delta(), image, fixture) < 8.0
            })
        {
            let delta = moving.drag_delta();
            let controls = fixture.bend.iter_mut().flatten();
            for point in [&mut fixture.from, &mut fixture.to]
                .into_iter()
                .chain(controls)
            {
                point.x = (point.x + delta.x / image.width()).clamp(0.0, 1.0);
                point.y = (point.y + delta.y / image.height()).clamp(0.0, 1.0);
            }
        }
        // Curved: the control points, each tied to its end; drag to bend.
        if let Some(bend) = &mut fixture.bend {
            for (index, (control, end)) in
                bend.iter_mut().zip([fixture.from, fixture.to]).enumerate()
            {
                let center = to_screen(image, *control);
                ui.painter()
                    .line_segment([to_screen(image, end), center], Stroke::new(1.0, TANGENT));
                let rect = Rect::from_center_size(center, Vec2::splat(16.0));
                let response = ui
                    .interact(rect, base.with(("control", index)), Sense::drag())
                    .on_hover_text("Poignée : glisser pour courber la bande");
                let side = if response.hovered() || response.dragged() {
                    11.0
                } else {
                    8.0
                };
                ui.painter().rect(
                    Rect::from_center_size(center, Vec2::splat(side)),
                    1.0,
                    TANGENT.gamma_multiply(0.6),
                    Stroke::new(1.5, TANGENT),
                    egui::StrokeKind::Middle,
                );
                if response.dragged()
                    && let Some(position) = response.interact_pointer_pos()
                {
                    *control = to_fraction(image, position);
                }
            }
        }
        // The ends: drag to place them; a curve's control point follows.
        let mut moved = [None, None];
        for (index, point) in [&mut fixture.from, &mut fixture.to].into_iter().enumerate() {
            let center = to_screen(image, *point);
            let rect = Rect::from_center_size(center, Vec2::splat(16.0));
            let response = ui.interact(rect, base.with(("end", index)), Sense::drag());
            ui.painter().circle(
                center,
                if response.hovered() || response.dragged() {
                    8.0
                } else {
                    6.0
                },
                SELECTED.gamma_multiply(0.6),
                Stroke::new(1.5, SELECTED),
            );
            if response.dragged()
                && let Some(position) = response.interact_pointer_pos()
            {
                let at = to_fraction(image, position);
                moved[index] = Some(Point::new(at.x - point.x, at.y - point.y));
                *point = at;
            }
        }
        if let Some(bend) = &mut fixture.bend {
            for (control, delta) in bend.iter_mut().zip(moved) {
                if let Some(delta) = delta {
                    control.x = (control.x + delta.x).clamp(0.0, 1.0);
                    control.y = (control.y + delta.y).clamp(0.0, 1.0);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances_to_a_segment() {
        let (a, b) = (Pos2::new(0.0, 0.0), Pos2::new(10.0, 0.0));
        assert_eq!(segment_distance(Pos2::new(5.0, 3.0), a, b), 3.0);
        assert_eq!(segment_distance(Pos2::new(13.0, 4.0), a, b), 5.0);
    }
}
