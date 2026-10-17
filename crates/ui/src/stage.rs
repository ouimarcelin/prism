//! The stage editor, shown in its own window: the screens of the output,
//! and the slices of the composition each one shows.
//!
//! On the left, the composition with the area of each slice; on the right,
//! the selected screen with the corners of each slice. Dragging inside a
//! shape moves it, dragging a corner reshapes it.

use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use prism_core::{Area, Point, Quad, Resolution, ScreenId, SliceId};

use crate::panel::{Action, ControlPanel, Screen as Display};

/// Radius of a corner handle, in points.
const HANDLE: f32 = 6.0;
/// Smallest width or height of a slice's area, as a fraction.
const MIN_SIZE: f32 = 0.01;

const SELECTED: Color32 = Color32::from_rgb(240, 170, 30);
const OTHER: Color32 = Color32::from_rgb(90, 160, 230);

/// What the stage window shows besides the document.
pub struct StageFrame<'a> {
    /// Composition image registered with the renderer, and its size.
    pub composition: Option<(egui::TextureId, Resolution)>,
    /// Image of each screen of the stage, registered with the renderer.
    pub screens: &'a [(ScreenId, egui::TextureId)],
    /// Displays of the computer.
    pub displays: &'a [Display],
}

/// Selection and drag of the stage editor.
#[derive(Debug, Default)]
pub(crate) struct StageEditor {
    screen: Option<ScreenId>,
    slice: Option<SliceId>,
}

/// The largest rectangle of `aspect` (width over height) centered in
/// `space`.
fn fit(space: Rect, aspect: f32) -> Rect {
    if aspect <= 0.0 || space.width() <= 0.0 || space.height() <= 0.0 {
        return space;
    }
    let size = if space.width() / space.height() > aspect {
        Vec2::new(space.height() * aspect, space.height())
    } else {
        Vec2::new(space.width(), space.width() / aspect)
    };
    Rect::from_center_size(space.center(), size)
}

fn to_screen(rect: Rect, point: Point) -> Pos2 {
    rect.min + Vec2::new(point.x * rect.width(), point.y * rect.height())
}

fn to_fraction(rect: Rect, position: Pos2) -> Point {
    Point::new(
        (position.x - rect.min.x) / rect.width().max(1.0),
        (position.y - rect.min.y) / rect.height().max(1.0),
    )
}

/// `area` with its corner `corner` (top left, top right, bottom right,
/// bottom left) moved to `point`, the opposite corner fixed, kept inside
/// the image.
pub(crate) fn drag_corner(area: Area, corner: usize, point: Point) -> Area {
    let corners = area.corners();
    let fixed = corners[(corner + 2) % 4];
    let x = point.x.clamp(0.0, 1.0);
    let y = point.y.clamp(0.0, 1.0);
    let (left, right) = if corner == 0 || corner == 3 {
        (x.min(fixed.x - MIN_SIZE), fixed.x)
    } else {
        (fixed.x, x.max(fixed.x + MIN_SIZE))
    };
    let (top, bottom) = if corner < 2 {
        (y.min(fixed.y - MIN_SIZE), fixed.y)
    } else {
        (fixed.y, y.max(fixed.y + MIN_SIZE))
    };
    Area {
        x: left.max(0.0),
        y: top.max(0.0),
        width: (right - left.max(0.0)).min(1.0),
        height: (bottom - top.max(0.0)).min(1.0),
    }
}

/// `area` moved by `delta`, kept inside the image.
pub(crate) fn move_area(area: Area, delta: Point) -> Area {
    Area {
        x: (area.x + delta.x).clamp(0.0, 1.0 - area.width),
        y: (area.y + delta.y).clamp(0.0, 1.0 - area.height),
        ..area
    }
}

/// Whether `point` lies inside the convex or concave quad `quad`.
pub(crate) fn quad_contains(quad: &Quad, point: Point) -> bool {
    // Crossings of a ray to the right with the edges.
    let mut inside = false;
    for index in 0..4 {
        let a = quad.0[index];
        let b = quad.0[(index + 1) % 4];
        if (a.y > point.y) != (b.y > point.y) {
            let x = a.x + (point.y - a.y) / (b.y - a.y) * (b.x - a.x);
            if point.x < x {
                inside = !inside;
            }
        }
    }
    inside
}

/// Draws a corner handle and returns its drag response.
fn handle(ui: &egui::Ui, id: egui::Id, center: Pos2, color: Color32) -> egui::Response {
    let rect = Rect::from_center_size(center, Vec2::splat(HANDLE * 2.5));
    let response = ui.interact(rect, id, Sense::drag());
    let radius = if response.hovered() || response.dragged() {
        HANDLE * 1.3
    } else {
        HANDLE
    };
    ui.painter().circle(
        center,
        radius,
        color.gamma_multiply(0.6),
        Stroke::new(1.5, color),
    );
    response
}

fn image(ui: &egui::Ui, rect: Rect, texture: Option<egui::TextureId>) {
    ui.painter().rect_filled(rect, 0.0, Color32::BLACK);
    if let Some(texture) = texture {
        ui.painter().image(
            texture,
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    }
    ui.painter().rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, Color32::from_gray(90)),
        egui::StrokeKind::Outside,
    );
}

impl ControlPanel {
    /// Draws the stage editor; returns what the application must do.
    pub fn stage_ui(&mut self, ui: &mut egui::Ui, frame: &StageFrame<'_>) -> Vec<Action> {
        let before = self.document.composition().clone();
        let stage = &self.document.composition().stage;
        // A screen or slice removed elsewhere is no longer selected.
        if self
            .stage
            .screen
            .is_none_or(|id| stage.screen(id).is_none())
        {
            self.stage.screen = stage.screens().first().map(prism_core::Screen::id);
        }
        let screen = self.stage.screen.and_then(|id| stage.screen(id));
        if self
            .stage
            .slice
            .is_none_or(|id| screen.is_none_or(|screen| screen.slice(id).is_none()))
        {
            self.stage.slice =
                screen.and_then(|screen| screen.slices.last().map(|slice| slice.id()));
        }

        egui::Panel::top("stage screens").show(ui, |ui| self.stage_screens(ui, frame));
        if self.stage.screen.is_some() {
            egui::Panel::left("stage slices")
                .resizable(true)
                .default_size(260.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| self.stage_slices(ui));
                });
        }
        egui::CentralPanel::default().show(ui, |ui| self.stage_views(ui, frame));

        self.document.commit(&before);
        std::mem::take(&mut self.actions)
    }

    fn stage_screens(&mut self, ui: &mut egui::Ui, frame: &StageFrame<'_>) {
        ui.horizontal_wrapped(|ui| {
            let stage = &self.document.composition().stage;
            ui.label(RichText::new("Écran").strong());
            let current = self
                .stage
                .screen
                .and_then(|id| stage.screen(id))
                .map_or_else(|| "aucun".to_owned(), |screen| screen.name.clone());
            let ids: Vec<(ScreenId, String)> = stage
                .screens()
                .iter()
                .map(|screen| (screen.id(), screen.name.clone()))
                .collect();
            egui::ComboBox::from_id_salt("stage screen")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for (id, name) in ids {
                        ui.selectable_value(&mut self.stage.screen, Some(id), name);
                    }
                });
            let display = frame
                .displays
                .iter()
                .find(|display| !display.primary)
                .or(frame.displays.first());
            if ui
                .button("+ Écran")
                .on_hover_text("Un écran de la taille d'un écran du système, montrant toute la composition")
                .clicked()
            {
                let count = self.document.composition().stage.screens().len();
                let (width, height) = display.map_or((1920, 1080), |display| {
                    (display.width, display.height)
                });
                let stage = &mut self.document.composition_mut().stage;
                if let Ok(id) = stage.add_screen(format!("Écran {}", count + 1), width, height) {
                    self.stage.screen = Some(id);
                    self.stage.slice = None;
                }
            }
            let Some(id) = self.stage.screen else {
                return;
            };
            ui.separator();
            let stage = &mut self.document.composition_mut().stage;
            let Some(screen) = stage.screen_mut(id) else {
                return;
            };
            ui.add(egui::TextEdit::singleline(&mut screen.name).desired_width(120.0));
            let (mut width, mut height) = screen.size();
            let mut resized = ui
                .add(
                    egui::DragValue::new(&mut width)
                        .range(1..=prism_core::MAX_SCREEN_SIZE)
                        .suffix(" px"),
                )
                .changed();
            ui.label("×");
            resized |= ui
                .add(
                    egui::DragValue::new(&mut height)
                        .range(1..=prism_core::MAX_SCREEN_SIZE)
                        .suffix(" px"),
                )
                .changed();
            if resized {
                let _ = screen.set_size(width, height);
            }
            ui.label("sur");
            let shown = screen
                .display
                .clone()
                .unwrap_or_else(|| "aucun écran".to_owned());
            egui::ComboBox::from_id_salt("stage display")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut screen.display, None, "aucun écran");
                    for display in frame.displays {
                        let label = format!(
                            "{} ({} × {})",
                            display.name, display.width, display.height
                        );
                        if ui
                            .selectable_label(
                                screen.display.as_deref() == Some(display.name.as_str()),
                                label,
                            )
                            .clicked()
                        {
                            screen.display = Some(display.name.clone());
                            // The screen takes the size of its display.
                            let _ = screen.set_size(display.width, display.height);
                        }
                    }
                })
                .response
                .on_hover_text("La sortie plein écran ouverte sur cet écran du système montre cet écran de la scène");
            if ui.button("Supprimer l'écran").clicked() {
                stage.remove_screen(id);
                self.stage.screen = None;
            }
        });
    }

    fn stage_slices(&mut self, ui: &mut egui::Ui) {
        let Some(screen_id) = self.stage.screen else {
            return;
        };
        let resolution = self.document.composition().resolution();
        ui.horizontal(|ui| {
            ui.label(RichText::new("Découpes").strong());
            if ui.button("+ Découpe").clicked() {
                let stage = &mut self.document.composition_mut().stage;
                let count = stage
                    .screen(screen_id)
                    .map_or(0, |screen| screen.slices.len());
                self.stage.slice = stage.add_slice(screen_id, format!("Découpe {}", count + 1));
            }
        });
        let stage = &mut self.document.composition_mut().stage;
        let Some(screen) = stage.screen_mut(screen_id) else {
            return;
        };
        let screen_size = screen.size();
        let mut remove = None;
        let mut swap = None;
        let count = screen.slices.len();
        for (index, slice) in screen.slices.iter_mut().enumerate().rev() {
            ui.horizontal(|ui| {
                ui.checkbox(&mut slice.enabled, "")
                    .on_hover_text("Dessiner la découpe");
                let selected = self.stage.slice == Some(slice.id());
                if ui.selectable_label(selected, &slice.name).clicked() {
                    self.stage.slice = Some(slice.id());
                }
                if ui
                    .add_enabled(index + 1 < count, egui::Button::new("⬆").small())
                    .on_hover_text("Au-dessus")
                    .clicked()
                {
                    swap = Some((index, index + 1));
                }
                if ui
                    .add_enabled(index > 0, egui::Button::new("⬇").small())
                    .on_hover_text("En dessous")
                    .clicked()
                {
                    swap = Some((index, index - 1));
                }
                if ui.small_button("×").on_hover_text("Supprimer").clicked() {
                    remove = Some(slice.id());
                }
            });
        }
        if let Some((a, b)) = swap {
            screen.slices.swap(a, b);
        }
        if let Some(id) = remove {
            stage.remove_slice(id);
            return;
        }
        let Some(slice) = self
            .stage
            .slice
            .and_then(|id| stage.screen_mut(screen_id)?.slice_mut(id))
        else {
            return;
        };
        ui.separator();
        ui.add(egui::TextEdit::singleline(&mut slice.name));
        ui.label(RichText::new("Entrée (pixels de la composition)").weak());
        let (comp_width, comp_height) = (resolution.width as f32, resolution.height as f32);
        let mut area = [
            slice.input.x * comp_width,
            slice.input.y * comp_height,
            slice.input.width * comp_width,
            slice.input.height * comp_height,
        ];
        let mut changed = false;
        egui::Grid::new("slice input")
            .num_columns(4)
            .show(ui, |ui| {
                for (label, value, limit) in [
                    ("x", 0, comp_width),
                    ("y", 1, comp_height),
                    ("l", 2, comp_width),
                    ("h", 3, comp_height),
                ] {
                    ui.label(label);
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut area[value])
                                .range(0.0..=limit)
                                .max_decimals(0),
                        )
                        .changed();
                    if value == 1 {
                        ui.end_row();
                    }
                }
            });
        if changed {
            let width = (area[2] / comp_width).max(MIN_SIZE);
            let height = (area[3] / comp_height).max(MIN_SIZE);
            slice.input = Area {
                x: (area[0] / comp_width).clamp(0.0, 1.0 - width),
                y: (area[1] / comp_height).clamp(0.0, 1.0 - height),
                width: width.min(1.0),
                height: height.min(1.0),
            };
        }
        ui.label(RichText::new("Sortie (pixels de l'écran)").weak());
        let (width, height) = (screen_size.0 as f32, screen_size.1 as f32);
        egui::Grid::new("slice output")
            .num_columns(3)
            .show(ui, |ui| {
                for (corner, name) in ["haut gauche", "haut droite", "bas droite", "bas gauche"]
                    .iter()
                    .enumerate()
                {
                    let point = &mut slice.output.0[corner];
                    let (mut x, mut y) = (point.x * width, point.y * height);
                    ui.label(*name);
                    if ui
                        .add(egui::DragValue::new(&mut x).max_decimals(0))
                        .changed()
                    {
                        point.x = x / width;
                    }
                    if ui
                        .add(egui::DragValue::new(&mut y).max_decimals(0))
                        .changed()
                    {
                        point.y = y / height;
                    }
                    ui.end_row();
                }
            });
        ui.horizontal_wrapped(|ui| {
            if ui
                .button("Plein écran")
                .on_hover_text("La découpe couvre tout l'écran")
                .clicked()
            {
                slice.output = Quad::FULL;
            }
            if ui
                .button("Comme l'entrée")
                .on_hover_text("La découpe garde sa place et sa taille dans l'image")
                .clicked()
            {
                slice.output = Quad::from_area(slice.input);
            }
        });
    }

    fn stage_views(&mut self, ui: &mut egui::Ui, frame: &StageFrame<'_>) {
        let Some(screen_id) = self.stage.screen else {
            ui.centered_and_justified(|ui| {
                ui.label(
                    "Aucun écran : chaque sortie montre toute la composition.\n« + Écran » en crée un, montrant toute la composition ; ajoutez-y des découpes.",
                );
            });
            return;
        };
        let space = ui.available_rect_before_wrap();
        let gap = 16.0;
        let half = (space.width() - gap) / 2.0;
        let title = 20.0;
        let left = Rect::from_min_size(
            space.min + Vec2::new(0.0, title),
            Vec2::new(half, space.height() - title),
        );
        let right = Rect::from_min_size(
            space.min + Vec2::new(half + gap, title),
            Vec2::new(half, space.height() - title),
        );
        ui.painter().text(
            space.min,
            egui::Align2::LEFT_TOP,
            "Entrée : la composition",
            egui::FontId::proportional(13.0),
            ui.visuals().text_color(),
        );
        ui.painter().text(
            space.min + Vec2::new(half + gap, 0.0),
            egui::Align2::LEFT_TOP,
            "Sortie : l'écran",
            egui::FontId::proportional(13.0),
            ui.visuals().text_color(),
        );

        let resolution = self.document.composition().resolution();
        let input = fit(
            left,
            resolution.width as f32 / resolution.height.max(1) as f32,
        );
        image(ui, input, frame.composition.map(|(texture, _)| texture));
        let Some(screen) = self.document.composition().stage.screen(screen_id) else {
            return;
        };
        let (width, height) = screen.size();
        let output = fit(right, width as f32 / height.max(1) as f32);
        let texture = frame
            .screens
            .iter()
            .find(|(id, _)| *id == screen_id)
            .map(|(_, texture)| *texture);
        image(ui, output, texture);
        ui.allocate_rect(space, Sense::hover());

        // Outlines of every slice; the selected one on top, with handles.
        for slice in &screen.slices {
            let color = if Some(slice.id()) == self.stage.slice {
                SELECTED
            } else {
                OTHER.gamma_multiply(if slice.enabled { 1.0 } else { 0.4 })
            };
            let corners = slice.input.corners().map(|point| to_screen(input, point));
            ui.painter().add(egui::Shape::closed_line(
                corners.to_vec(),
                Stroke::new(1.5, color),
            ));
            let quad = slice.output.0.map(|point| to_screen(output, point));
            ui.painter().add(egui::Shape::closed_line(
                quad.to_vec(),
                Stroke::new(1.5, color),
            ));
        }

        // Clicks pick the slice under the pointer, topmost first.
        let pointer = ui.input(|input| input.pointer.interact_pos());
        let clicked = ui.input(|input| input.pointer.primary_pressed());
        if clicked && let Some(position) = pointer {
            let picked = screen.slices.iter().rev().find(|slice| {
                (input.contains(position) && {
                    let point = to_fraction(input, position);
                    let area = slice.input;
                    (area.x..=area.x + area.width).contains(&point.x)
                        && (area.y..=area.y + area.height).contains(&point.y)
                }) || (output.contains(position)
                    && quad_contains(&slice.output, to_fraction(output, position)))
            });
            if let Some(slice) = picked
                && self.stage.slice != Some(slice.id())
            {
                self.stage.slice = Some(slice.id());
                return;
            }
        }

        let Some(slice_id) = self.stage.slice else {
            return;
        };
        let Some(slice) = self
            .document
            .composition_mut()
            .stage
            .screen_mut(screen_id)
            .and_then(|screen| screen.slice_mut(slice_id))
        else {
            return;
        };
        let base = ui.id().with(("slice", slice_id.0));

        // Inside the shapes: drag to move.
        let input_rect = Rect::from_two_pos(
            to_screen(input, slice.input.corners()[0]),
            to_screen(input, slice.input.corners()[2]),
        );
        let moving = ui.interact(input_rect, base.with("input move"), Sense::drag());
        if moving.dragged() {
            let delta = moving.drag_delta();
            slice.input = move_area(
                slice.input,
                Point::new(delta.x / input.width(), delta.y / input.height()),
            );
        }
        let bounds = Rect::from_points(&slice.output.0.map(|point| to_screen(output, point)));
        let moving = ui.interact(bounds, base.with("output move"), Sense::drag());
        if moving.dragged()
            && moving.interact_pointer_pos().is_some_and(|position| {
                quad_contains(
                    &slice.output,
                    to_fraction(output, position - moving.drag_delta()),
                )
            })
        {
            let delta = moving.drag_delta();
            for point in &mut slice.output.0 {
                point.x += delta.x / output.width();
                point.y += delta.y / output.height();
            }
        }

        // Corners: drag to reshape.
        for corner in 0..4 {
            let center = to_screen(input, slice.input.corners()[corner]);
            let response = handle(ui, base.with(("input", corner)), center, SELECTED);
            if response.dragged()
                && let Some(position) = response.interact_pointer_pos()
            {
                slice.input = drag_corner(slice.input, corner, to_fraction(input, position));
            }
            let center = to_screen(output, slice.output.0[corner]);
            let response = handle(ui, base.with(("output", corner)), center, SELECTED);
            if response.dragged()
                && let Some(position) = response.interact_pointer_pos()
            {
                slice.output.0[corner] = to_fraction(output, position);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corners_reshape_with_the_opposite_one_fixed() {
        let area = Area {
            x: 0.2,
            y: 0.2,
            width: 0.4,
            height: 0.4,
        };
        // Bottom right corner dragged further out.
        let grown = drag_corner(area, 2, Point::new(0.8, 0.9));
        assert!((grown.x - 0.2).abs() < 1e-6 && (grown.width - 0.6).abs() < 1e-6);
        assert!((grown.height - 0.7).abs() < 1e-6);
        // Top left corner dragged past the bottom right one: a thin area
        // stays.
        let thin = drag_corner(area, 0, Point::new(0.9, 0.9));
        assert!(thin.width >= MIN_SIZE * 0.99 && thin.height >= MIN_SIZE * 0.99);
        assert!((thin.x + thin.width - 0.6).abs() < 1e-6);
        // Outside the image: clamped.
        let clamped = drag_corner(area, 0, Point::new(-1.0, -1.0));
        assert_eq!((clamped.x, clamped.y), (0.0, 0.0));
        assert!((clamped.width - 0.6).abs() < 1e-6);
    }

    #[test]
    fn areas_move_inside_the_image() {
        let area = Area {
            x: 0.5,
            y: 0.5,
            width: 0.4,
            height: 0.4,
        };
        let moved = move_area(area, Point::new(0.3, -0.7));
        assert!((moved.x - 0.6).abs() < 1e-6);
        assert_eq!(moved.y, 0.0);
    }

    #[test]
    fn points_inside_quads() {
        let trapezoid = Quad([
            Point::new(0.25, 0.0),
            Point::new(0.75, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 1.0),
        ]);
        assert!(quad_contains(&trapezoid, Point::new(0.5, 0.5)));
        assert!(!quad_contains(&trapezoid, Point::new(0.1, 0.1)));
        assert!(quad_contains(&Quad::FULL, Point::new(0.99, 0.01)));
    }

    #[test]
    fn images_fit_inside_their_space() {
        let space = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 100.0));
        let fitted = fit(space, 2.0);
        assert_eq!(fitted.size(), Vec2::new(200.0, 100.0));
        assert_eq!(fitted.center(), space.center());
    }
}
