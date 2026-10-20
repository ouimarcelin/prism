//! Look of the interface: colours, type, sizes and spacing, in one place.
//!
//! The values follow a published dark interface kit (see ADR 0040): grey
//! surfaces in three steps, one blue accent, white text in three strengths,
//! 24-point controls, 5-point corners and a 4/8/16 spacing scale. Widgets
//! take the theme from egui's [`egui::Style`]; the few shapes drawn by hand
//! (clip cells, stage handles, learn overlays) take the colours from
//! [`color`].

use std::sync::Arc;

use egui::epaint::Shadow;
use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Frame, Margin, Rect,
    RichText, Stroke, Style, TextStyle, Vec2, Visuals,
};

/// Colours of one mode of the theme.
pub(crate) struct Palette {
    /// Panels and windows.
    pub(crate) bg: Color32,
    /// Fields, cards, hovered rows.
    pub(crate) bg_secondary: Color32,
    /// Hovered fields and cards, slider rails.
    pub(crate) bg_tertiary: Color32,
    /// Pressed fields; checkbox boxes when hovered.
    pub(crate) bg_strong: Color32,
    /// Behind the clip grid and the stage.
    pub(crate) canvas: Color32,
    /// Clip cards on the canvas.
    pub(crate) card: Color32,
    /// Clip cards under the pointer.
    pub(crate) card_hover: Color32,
    /// Menus, tooltips and windows.
    pub(crate) menu: Color32,
    /// Separators and outlines.
    pub(crate) border: Color32,
    /// Outlines of menus and of empty cells.
    pub(crate) border_subtle: Color32,

    /// Main text.
    pub(crate) text: Color32,
    /// Labels: the main text at 70 % (dark) or 50 % (light) over [`Palette::bg`].
    pub(crate) text_secondary: Color32,
    /// Hints and placeholders: at 40 % (dark) or 30 % (light).
    pub(crate) text_tertiary: Color32,

    /// Accent: selection, toggles on, slider fill, primary buttons.
    pub(crate) brand: Color32,
    /// Accent for text and thin lines.
    pub(crate) brand_text: Color32,

    /// Playing, sent, connected.
    pub(crate) success: Color32,
    /// Success, as text.
    pub(crate) success_text: Color32,
    /// Attention.
    pub(crate) warning: Color32,
    /// Errors and missing media, as text.
    pub(crate) danger_text: Color32,
    /// Third colour for shapes drawn by hand, after the accent and grey.
    pub(crate) assistive: Color32,
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// The dark mode, the default: the interface is used in dark rooms, next
/// to the output.
pub(crate) const DARK: Palette = Palette {
    bg: rgb(0x2c2c2c),
    bg_secondary: rgb(0x383838),
    bg_tertiary: rgb(0x444444),
    bg_strong: rgb(0x555555),
    canvas: rgb(0x1e1e1e),
    card: rgb(0x383838),
    card_hover: rgb(0x444444),
    menu: rgb(0x1e1e1e),
    border: rgb(0x444444),
    border_subtle: rgb(0x383838),
    text: rgb(0xffffff),
    text_secondary: rgb(0xbfbfbf),
    text_tertiary: rgb(0x808080),
    brand: rgb(0x0c8ce9),
    brand_text: rgb(0x7cc4f8),
    success: rgb(0x198f51),
    success_text: rgb(0x79d297),
    warning: rgb(0xf3c11b),
    danger_text: rgb(0xfca397),
    assistive: rgb(0xf316b0),
};

/// The light mode, for daylight and preparation.
pub(crate) const LIGHT: Palette = Palette {
    bg: rgb(0xffffff),
    bg_secondary: rgb(0xf5f5f5),
    bg_tertiary: rgb(0xe6e6e6),
    bg_strong: rgb(0xd9d9d9),
    canvas: rgb(0xf5f5f5),
    card: rgb(0xffffff),
    card_hover: rgb(0xe6e6e6),
    menu: rgb(0xffffff),
    border: rgb(0xe6e6e6),
    border_subtle: rgb(0xe6e6e6),
    text: rgb(0x1a1a1a),
    text_secondary: rgb(0x808080),
    text_tertiary: rgb(0xb3b3b3),
    brand: rgb(0x0d99ff),
    brand_text: rgb(0x007be5),
    success: rgb(0x14ae5c),
    success_text: rgb(0x009951),
    warning: rgb(0xffcd29),
    danger_text: rgb(0xdc3412),
    assistive: rgb(0xff24bd),
};

/// The palette of the mode `ui` is drawn in.
pub(crate) fn palette(ui: &egui::Ui) -> &'static Palette {
    if ui.visuals().dark_mode {
        &DARK
    } else {
        &LIGHT
    }
}

/// Corner radius of controls and cards, in points.
pub(crate) const RADIUS: u8 = 5;
/// Corner radius of windows and menus, in points.
const RADIUS_LARGE: u8 = 13;
/// Height of a control, in points.
pub(crate) const CONTROL_HEIGHT: f32 = 24.0;

/// Spacing scale, in points.
pub(crate) const SPACE_1: f32 = 4.0;
pub(crate) const SPACE_2: f32 = 8.0;
pub(crate) const SPACE_3: f32 = 16.0;

/// Text sizes, in points.
const SIZE_SMALL: f32 = 9.0;
const SIZE_BODY: f32 = 11.0;
const SIZE_HEADING: f32 = 13.0;

const MEDIUM: &str = "Inter-Medium";
const SEMIBOLD: &str = "Inter-SemiBold";

/// Applies the look of Prism to `ctx`.
pub fn apply_theme(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    ctx.set_visuals_of(egui::Theme::Dark, visuals(&DARK));
    ctx.set_visuals_of(egui::Theme::Light, visuals(&LIGHT));
    ctx.all_styles_mut(style);
    set_light(ctx, false);
}

/// Shows `ctx` in the light mode, or in the dark one, whatever the
/// system's appearance.
pub fn set_light(ctx: &egui::Context, light: bool) {
    let theme = if light {
        egui::Theme::Light
    } else {
        egui::Theme::Dark
    };
    if ctx.theme() != theme {
        ctx.set_theme(theme);
    }
}

/// Font family of headings and emphasis.
pub(crate) fn semibold() -> FontFamily {
    FontFamily::Name(SEMIBOLD.into())
}

/// Emphasis for labels: semibold, in the main text colour.
pub(crate) trait Emphasis {
    /// Semibold text, for names and section titles.
    fn semibold(self) -> Self;
}

impl Emphasis for RichText {
    fn semibold(self) -> Self {
        self.family(semibold()).strong()
    }
}

/// A switch and its label on one row; clicking either flips `on`.
pub(crate) fn switch_row(ui: &mut egui::Ui, on: &mut bool, label: &str) -> egui::Response {
    let palette = palette(ui);
    let galley = egui::WidgetText::from(label).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        TextStyle::Button,
    );
    let switch = Vec2::new(28.0, 16.0);
    let width = SPACE_1 + switch.x + SPACE_2 + galley.size().x + SPACE_2;
    let (rect, mut response) =
        ui.allocate_exact_size(Vec2::new(width, CONTROL_HEIGHT), egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *on, label)
    });
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if response.hovered() {
            painter.rect_filled(rect, RADIUS, ui.visuals().widgets.hovered.weak_bg_fill);
        }
        let track = Rect::from_min_size(
            egui::pos2(rect.left() + SPACE_1, rect.center().y - switch.y / 2.0),
            switch,
        );
        let radius = switch.y / 2.0;
        let fill = if *on {
            palette.brand
        } else {
            palette.bg_strong
        };
        painter.rect_filled(track, CornerRadius::same(radius as u8), fill);
        let how_on = ui.ctx().animate_bool_responsive(response.id, *on);
        let x = egui::lerp(track.left() + radius..=track.right() - radius, how_on);
        painter.circle_filled(
            egui::pos2(x, track.center().y),
            radius - 2.0,
            Color32::WHITE,
        );
        painter.galley(
            egui::pos2(
                track.right() + SPACE_2,
                rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            ui.visuals().widgets.inactive.text_color(),
        );
    }
    response
}

/// Frame of the menu bar and of the transport bar.
pub(crate) fn bar_frame(style: &Style) -> Frame {
    Frame::side_top_panel(style).inner_margin(Margin::symmetric(SPACE_2 as i8, SPACE_1 as i8))
}

/// Frame of the side panels.
pub(crate) fn side_frame(style: &Style) -> Frame {
    Frame::side_top_panel(style).inner_margin(Margin::symmetric(SPACE_3 as i8, SPACE_2 as i8))
}

/// Frame of the central area, set back from the panels around it.
pub(crate) fn canvas_frame(style: &Style) -> Frame {
    let palette = if style.visuals.dark_mode {
        &DARK
    } else {
        &LIGHT
    };
    Frame::central_panel(style)
        .fill(palette.canvas)
        .inner_margin(SPACE_3)
}

/// Inter for text, egui's fonts after it for the symbols Inter lacks.
fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        MEDIUM.into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../../assets/fonts/Inter-Medium.ttf"
        ))),
    );
    fonts.font_data.insert(
        SEMIBOLD.into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../../assets/fonts/Inter-SemiBold.ttf"
        ))),
    );
    let fallbacks = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mut proportional = vec![MEDIUM.to_owned()];
    proportional.extend(fallbacks.iter().cloned());
    let mut strong = vec![SEMIBOLD.to_owned()];
    strong.extend(fallbacks);
    fonts
        .families
        .insert(FontFamily::Proportional, proportional);
    fonts.families.insert(semibold(), strong);
    fonts
}

fn visuals(color: &Palette) -> Visuals {
    let mut visuals = if color.text == DARK.text {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    let radius = CornerRadius::same(RADIUS);
    let widget = |fill: Color32, stroke: Stroke, text: Color32| egui::style::WidgetVisuals {
        bg_fill: fill,
        weak_bg_fill: fill,
        bg_stroke: stroke,
        corner_radius: radius,
        fg_stroke: Stroke::new(1.0, text),
        expansion: 0.0,
    };
    let widgets = &mut visuals.widgets;
    widgets.noninteractive = widget(
        color.bg,
        Stroke::new(1.0, color.border),
        color.text_secondary,
    );
    widgets.inactive = widget(color.bg_secondary, Stroke::NONE, color.text);
    // Checkbox boxes and slider rails use `bg_fill`: a step lighter than
    // the fields, to show on the panels and on the cards.
    widgets.inactive.bg_fill = color.bg_tertiary;
    widgets.hovered = widget(color.bg_tertiary, Stroke::NONE, color.text);
    widgets.hovered.bg_fill = color.bg_strong;
    widgets.active = widget(color.bg_tertiary, Stroke::new(1.0, color.brand), color.text);
    widgets.open = widget(color.bg_tertiary, Stroke::NONE, color.text);

    visuals.selection.bg_fill = color.brand;
    visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.text_cursor.stroke = Stroke::new(1.5, color.brand_text);
    visuals.weak_text_color = Some(color.text_tertiary);
    visuals.hyperlink_color = color.brand_text;
    visuals.warn_fg_color = color.warning;
    visuals.error_fg_color = color.danger_text;

    visuals.panel_fill = color.bg;
    visuals.faint_bg_color = color.bg_secondary;
    visuals.extreme_bg_color = color.canvas;
    visuals.text_edit_bg_color = Some(color.bg_secondary);
    visuals.code_bg_color = color.bg_secondary;

    visuals.window_fill = color.menu;
    visuals.window_stroke = Stroke::new(1.0, color.border_subtle);
    visuals.window_corner_radius = CornerRadius::same(RADIUS_LARGE);
    visuals.menu_corner_radius = CornerRadius::same(RADIUS_LARGE);
    visuals.window_shadow = Shadow {
        offset: [0, 10],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(if visuals.dark_mode { 115 } else { 46 }),
    };
    visuals.popup_shadow = Shadow {
        offset: [0, 10],
        blur: 16,
        spread: 0,
        color: Color32::from_black_alpha(if visuals.dark_mode { 90 } else { 31 }),
    };

    visuals.indent_has_left_vline = false;
    visuals.striped = false;
    visuals
}

fn style(style: &mut Style) {
    use FontFamily::{Monospace, Proportional};
    style.text_styles = [
        (TextStyle::Small, FontId::new(SIZE_SMALL, Proportional)),
        (TextStyle::Body, FontId::new(SIZE_BODY, Proportional)),
        (TextStyle::Button, FontId::new(SIZE_BODY, Proportional)),
        (TextStyle::Monospace, FontId::new(SIZE_BODY, Monospace)),
        (TextStyle::Heading, FontId::new(SIZE_HEADING, semibold())),
    ]
    .into();

    let spacing = &mut style.spacing;
    spacing.item_spacing = Vec2::new(SPACE_2, SPACE_1);
    spacing.button_padding = Vec2::new(SPACE_2, SPACE_1);
    spacing.interact_size = Vec2::new(40.0, CONTROL_HEIGHT);
    spacing.window_margin = Margin::same(SPACE_3 as i8);
    spacing.menu_margin = Margin::same(SPACE_2 as i8);
    spacing.indent = SPACE_3;
    spacing.icon_width = 14.0;
    spacing.icon_width_inner = 8.0;
    spacing.icon_spacing = 6.0;
    spacing.combo_height = 320.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_loads_its_fonts() {
        let ctx = egui::Context::default();
        apply_theme(&ctx);
        // Panics if a family of the theme has no font.
        let mut output = ctx.run_ui(Default::default(), |ui| {
            ui.label("Prism");
            ui.label(RichText::new("Couche").semibold());
            ui.heading("Clip");
        });
        output.textures_delta.clear();
        let style = ctx.global_style();
        assert_eq!(style.spacing.interact_size.y, CONTROL_HEIGHT);
        assert_eq!(style.visuals.panel_fill, DARK.bg);
        assert_eq!(style.text_styles[&TextStyle::Heading].family, semibold());
        assert_eq!(ctx.theme(), egui::Theme::Dark);
        set_light(&ctx, true);
        assert_eq!(ctx.theme(), egui::Theme::Light);
        assert_eq!(ctx.global_style().visuals.panel_fill, LIGHT.bg);
    }
}
