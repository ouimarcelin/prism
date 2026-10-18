//! Sliders in the theme: a pill-shaped track, the accent filling it from its
//! origin to the knob, a white knob with an accent dot, and snap points that
//! hold the knob when it passes near them (Alt held: no snapping).

use std::ops::RangeInclusive;

use egui::emath::{Numeric, remap_clamp};
use egui::epaint::Shadow;
use egui::{
    Color32, CornerRadius, DragValue, Key, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2,
    Widget, WidgetInfo, WidgetText, pos2,
};

use crate::theme::color;

/// Height of the track and diameter of the knob, in points.
const TRACK: f32 = 16.0;
/// Radius of the dot in the knob, in points.
const DOT: f32 = 3.0;
/// Radius of a snap point, in points.
const MARK: f32 = 2.0;
/// Distance under which the knob jumps to a snap point, in points.
const SNAP_DISTANCE: f32 = 6.0;
/// Part of the range an arrow key moves.
const KEY_STEP: f64 = 0.01;
/// On a logarithmic slider starting at zero, smallest value above zero, as
/// a part of the largest.
const LOG_FLOOR: f64 = 1e-3;

type GetSet<'a> = Box<dyn 'a + FnMut(Option<f64>) -> f64>;

/// A slider for a number, with its value box after it.
pub(crate) struct Slider<'a> {
    get_set: GetSet<'a>,
    min: f64,
    max: f64,
    integer: bool,
    logarithmic: bool,
    show_value: bool,
    suffix: String,
    max_decimals: Option<usize>,
    text: Option<WidgetText>,
    snaps: Vec<f64>,
    origin: Option<f64>,
}

impl<'a> Slider<'a> {
    /// A slider for `value` within `range`. Over a range around zero, the
    /// fill starts at zero and the knob is held there.
    pub(crate) fn new<N: Numeric>(value: &'a mut N, range: RangeInclusive<N>) -> Self {
        let integer = N::INTEGRAL;
        let get_set = Box::new(move |new: Option<f64>| {
            if let Some(new) = new {
                *value = N::from_f64(if integer { new.round() } else { new });
            }
            value.to_f64()
        });
        let (min, max) = (range.start().to_f64(), range.end().to_f64());
        let slider = Self {
            get_set,
            min,
            max,
            integer,
            logarithmic: false,
            show_value: true,
            suffix: String::new(),
            max_decimals: None,
            text: None,
            snaps: Vec::new(),
            origin: None,
        };
        if min < 0.0 && max > 0.0 {
            slider.origin(0.0)
        } else {
            slider
        }
    }

    /// Whether the value box is shown after the track.
    pub(crate) fn show_value(mut self, show: bool) -> Self {
        self.show_value = show;
        self
    }

    /// Text after the number in the value box.
    pub(crate) fn suffix(mut self, suffix: impl ToString) -> Self {
        self.suffix = suffix.to_string();
        self
    }

    /// Most decimals shown in the value box.
    pub(crate) fn max_decimals(mut self, decimals: usize) -> Self {
        self.max_decimals = Some(decimals);
        self
    }

    /// Spreads the values along the track by their logarithm: as much room
    /// from 0.1 to 1 as from 1 to 10.
    pub(crate) fn logarithmic(mut self, logarithmic: bool) -> Self {
        self.logarithmic = logarithmic;
        self
    }

    /// Label after the value box.
    pub(crate) fn text(mut self, text: impl Into<WidgetText>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Holds the knob at `values` when it passes near them. Values outside
    /// the range, or at its ends, are left out.
    pub(crate) fn snaps(mut self, values: impl IntoIterator<Item = f64>) -> Self {
        for value in values {
            if value > self.min && value < self.max && !self.snaps.contains(&value) {
                self.snaps.push(value);
            }
        }
        self
    }

    /// Snap points at a quarter, half and three quarters of the range.
    pub(crate) fn quarters(self) -> Self {
        let (min, max) = (self.min, self.max);
        self.snaps([0.25, 0.5, 0.75].map(|part| min + (max - min) * part))
    }

    /// Starts the fill at `value` instead of the left end, and holds the
    /// knob there: for a setting with a neutral middle, such as a balance.
    pub(crate) fn origin(mut self, value: f64) -> Self {
        self.origin = Some(value);
        self.snaps([value])
    }

    /// Where `value` lies along the track, from 0 to 1.
    fn fraction(&self, value: f64) -> f64 {
        if let Some(floor) = self.log_floor() {
            if value <= floor {
                return 0.0;
            }
            return ((value / floor).ln() / (self.max / floor).ln()).clamp(0.0, 1.0);
        }
        if self.max > self.min {
            ((value - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// The value at `fraction` of the track.
    fn value_at(&self, fraction: f64) -> f64 {
        if let Some(floor) = self.log_floor() {
            if fraction <= 0.0 {
                return self.min;
            }
            return floor * (self.max / floor).powf(fraction);
        }
        self.min + (self.max - self.min) * fraction
    }

    /// Smallest value a logarithmic slider spreads, when it is one.
    fn log_floor(&self) -> Option<f64> {
        let usable = self.logarithmic && self.min >= 0.0 && self.max > self.min;
        let floor = if self.min > 0.0 {
            self.min
        } else {
            self.max * LOG_FLOOR
        };
        usable.then_some(floor)
    }

    fn clamp(&self, value: f64) -> f64 {
        let value = value.clamp(self.min, self.max);
        if self.integer { value.round() } else { value }
    }

    fn track(&mut self, ui: &mut Ui) -> Response {
        let size = Vec2::new(ui.spacing().slider_width, ui.spacing().interact_size.y);
        let (rect, mut response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let track = Rect::from_center_size(rect.center(), Vec2::new(rect.width(), TRACK));
        let span = track.x_range().shrink(TRACK / 2.0);
        let x_of = |slider: &Self, value: f64| {
            egui::emath::lerp(span.min..=span.max, slider.fraction(value) as f32)
        };

        let old = (self.get_set)(None);
        let mut value = old;
        if let Some(pointer) = response.interact_pointer_pos() {
            let fraction = remap_clamp(pointer.x, span.min..=span.max, 0.0..=1.0);
            value = self.value_at(f64::from(fraction));
            if !ui.input(|input| input.modifiers.alt) {
                let nearest = self
                    .snaps
                    .iter()
                    .map(|&snap| (snap, (x_of(self, snap) - pointer.x).abs()))
                    .filter(|&(_, distance)| distance <= SNAP_DISTANCE)
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                if let Some((snap, _)) = nearest {
                    value = snap;
                }
            }
        }
        if response.has_focus() {
            let steps = ui.input(|input| {
                i32::from(input.key_pressed(Key::ArrowRight))
                    - i32::from(input.key_pressed(Key::ArrowLeft))
            });
            if steps != 0 {
                let step = if self.integer {
                    1.0
                } else {
                    (self.max - self.min) * KEY_STEP
                };
                value += f64::from(steps) * step;
            }
        }
        let value = self.clamp(value);
        if value != old {
            (self.get_set)(Some(value));
            response.mark_changed();
        }
        response.widget_info(|| WidgetInfo::slider(ui.is_enabled(), value, ""));

        if ui.is_rect_visible(rect) {
            let knob = x_of(self, value);
            let origin = self.origin.map(|origin| x_of(self, origin));
            let marks: Vec<f32> = self.snaps.iter().map(|&snap| x_of(self, snap)).collect();
            paint(ui, &response, track, knob, origin, &marks);
        }
        response
    }
}

impl Widget for Slider<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        ui.horizontal(|ui| {
            let mut response = self.track(ui);
            if self.show_value {
                let speed = (self.max - self.min) / f64::from(ui.spacing().slider_width.max(1.0));
                let (min, max, integer) = (self.min, self.max, self.integer);
                let suffix = std::mem::take(&mut self.suffix);
                let max_decimals = self.max_decimals;
                let get_set = &mut self.get_set;
                let mut value = DragValue::from_get_set(get_set)
                    .range(min..=max)
                    .speed(speed)
                    .suffix(suffix)
                    .max_decimals_opt(max_decimals);
                if integer {
                    value = value.fixed_decimals(0);
                }
                response |= ui.add(value);
            }
            if let Some(text) = self.text.take() {
                ui.label(text);
            }
            response
        })
        .inner
    }
}

/// Draws the track, the fill, the snap points and the knob, at the
/// horizontal positions given.
fn paint(ui: &Ui, response: &Response, track: Rect, knob: f32, origin: Option<f32>, marks: &[f32]) {
    let painter = ui.painter();
    let radius = TRACK / 2.0;
    let round = CornerRadius::same(radius as u8);
    let y = track.center().y;
    painter.rect(
        track,
        round,
        color::BG_SECONDARY,
        Stroke::new(1.0, color::BORDER),
        StrokeKind::Inside,
    );

    // From the left end, or from the origin, to under the knob.
    let (start, end) = match origin {
        Some(origin) => (origin.min(knob) - radius, origin.max(knob) + radius),
        None => (track.left(), knob + radius),
    };
    let fill = Rect::from_min_max(pos2(start, track.top()), pos2(end, track.bottom()));
    painter.rect_filled(fill, round, color::BRAND);

    for &mark in marks {
        let filled = mark >= start + radius && mark <= end - radius;
        let mark_color = if filled {
            color::BRAND_DARK
        } else {
            color::TEXT_TERTIARY
        };
        painter.circle_filled(pos2(mark, y), MARK, mark_color);
    }

    let center = pos2(knob, y);
    let active = response.hovered() || response.dragged();
    let knob_radius = if active { radius + 1.0 } else { radius };
    let shadow = Shadow {
        offset: [0, 1],
        blur: 4,
        spread: 0,
        color: Color32::from_black_alpha(110),
    };
    painter.add(shadow.as_shape(
        Rect::from_center_size(center, Vec2::splat(knob_radius * 2.0)),
        CornerRadius::same(knob_radius as u8),
    ));
    painter.circle_filled(center, knob_radius, Color32::WHITE);
    painter.circle(
        center,
        if response.dragged() { DOT + 1.0 } else { DOT },
        color::BRAND,
        Stroke::new(1.0, color::BRAND_DARK),
    );
    if response.has_focus() {
        painter.circle_stroke(
            center,
            knob_radius + 2.0,
            Stroke::new(1.5, color::BRAND_TEXT),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_points_stay_inside_the_range() {
        let mut value = 0.3_f32;
        let slider = Slider::new(&mut value, 0.0..=1.0)
            .quarters()
            .snaps([0.0, 1.0, 2.0, 0.5]);
        assert_eq!(slider.snaps, vec![0.25, 0.5, 0.75]);
    }

    #[test]
    fn a_range_around_zero_fills_from_zero() {
        let mut angle = 10.0_f32;
        let slider = Slider::new(&mut angle, -180.0..=180.0);
        assert_eq!(slider.origin, Some(0.0));
        assert_eq!(slider.snaps, vec![0.0]);
        let mut opacity = 1.0_f32;
        assert_eq!(Slider::new(&mut opacity, 0.0..=1.0).origin, None);
    }

    #[test]
    fn logarithmic_track_round_trips() {
        let mut speed = 1.0_f32;
        let slider = Slider::new(&mut speed, 0.0..=10.0).logarithmic(true);
        assert_eq!(slider.value_at(0.0), 0.0);
        assert!((slider.value_at(1.0) - 10.0).abs() < 1e-9);
        let middle = slider.fraction(1.0);
        assert!((slider.value_at(middle) - 1.0).abs() < 1e-9);
        assert!(middle > 0.5, "1 × is past the middle of 0 to 10 ×");
    }

    #[test]
    fn integers_are_rounded() {
        let mut count = 3_i32;
        let mut slider = Slider::new(&mut count, 0..=10);
        assert_eq!(slider.clamp(4.6), 5.0);
        (slider.get_set)(Some(7.4));
        drop(slider);
        assert_eq!(count, 7);
    }
}
