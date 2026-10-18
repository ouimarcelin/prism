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
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Frame, Margin, RichText,
    Stroke, Style, TextStyle, Vec2, Visuals,
};

/// Colours of the theme, dark mode only.
pub(crate) mod color {
    use egui::Color32;

    /// Panels and windows.
    pub(crate) const BG: Color32 = Color32::from_rgb(0x2c, 0x2c, 0x2c);
    /// Fields, cards, hovered rows.
    pub(crate) const BG_SECONDARY: Color32 = Color32::from_rgb(0x38, 0x38, 0x38);
    /// Hovered fields and cards, slider rails.
    pub(crate) const BG_TERTIARY: Color32 = Color32::from_rgb(0x44, 0x44, 0x44);
    /// Behind the clip grid and the stage, and in menus and tooltips.
    pub(crate) const CANVAS: Color32 = Color32::from_rgb(0x1e, 0x1e, 0x1e);
    /// Separators and outlines.
    pub(crate) const BORDER: Color32 = Color32::from_rgb(0x44, 0x44, 0x44);
    /// Outlines of menus and of empty cells.
    pub(crate) const BORDER_SUBTLE: Color32 = Color32::from_rgb(0x38, 0x38, 0x38);

    /// Main text.
    pub(crate) const TEXT: Color32 = Color32::WHITE;
    /// Labels: white at 70 % over [`BG`].
    pub(crate) const TEXT_SECONDARY: Color32 = Color32::from_rgb(0xbf, 0xbf, 0xbf);
    /// Hints and placeholders: white at 40 % over [`BG`].
    pub(crate) const TEXT_TERTIARY: Color32 = Color32::from_rgb(0x80, 0x80, 0x80);

    /// Accent: selection, toggles on, slider fill, primary buttons.
    pub(crate) const BRAND: Color32 = Color32::from_rgb(0x0c, 0x8c, 0xe9);
    /// Accent, pressed; marks over the accent.
    pub(crate) const BRAND_DARK: Color32 = Color32::from_rgb(0x0a, 0x6d, 0xc2);
    /// Accent for text and thin lines over dark surfaces.
    pub(crate) const BRAND_TEXT: Color32 = Color32::from_rgb(0x7c, 0xc4, 0xf8);

    /// Playing, sent, connected.
    pub(crate) const SUCCESS: Color32 = Color32::from_rgb(0x19, 0x8f, 0x51);
    /// Success, as text.
    pub(crate) const SUCCESS_TEXT: Color32 = Color32::from_rgb(0x79, 0xd2, 0x97);
    /// Attention.
    pub(crate) const WARNING: Color32 = Color32::from_rgb(0xf3, 0xc1, 0x1b);
    /// Errors and missing media, as text.
    pub(crate) const DANGER_TEXT: Color32 = Color32::from_rgb(0xfc, 0xa3, 0x97);
    /// Third colour for shapes drawn by hand, after [`BRAND`] and grey.
    pub(crate) const ASSISTIVE: Color32 = Color32::from_rgb(0xf3, 0x16, 0xb0);
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
    // Dark whatever the system's appearance: the interface is used in
    // dark rooms, next to the output.
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.set_visuals_of(egui::Theme::Dark, visuals());
    ctx.all_styles_mut(style);
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
        self.family(semibold()).color(color::TEXT)
    }
}

/// Frame of the menu bar and of the transport bar.
pub(crate) fn bar_frame(style: &Style) -> Frame {
    Frame::side_top_panel(style).inner_margin(Margin::symmetric(SPACE_2 as i8, SPACE_1 as i8))
}

/// Frame of the side panels.
pub(crate) fn side_frame(style: &Style) -> Frame {
    Frame::side_top_panel(style).inner_margin(Margin::symmetric(SPACE_3 as i8, SPACE_2 as i8))
}

/// Frame of the central area, darker than the panels around it.
pub(crate) fn canvas_frame(style: &Style) -> Frame {
    Frame::central_panel(style)
        .fill(color::CANVAS)
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

fn visuals() -> Visuals {
    let mut visuals = Visuals::dark();
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
        color::BG,
        Stroke::new(1.0, color::BORDER),
        color::TEXT_SECONDARY,
    );
    widgets.inactive = widget(color::BG_SECONDARY, Stroke::NONE, color::TEXT);
    // Checkbox boxes and slider rails use `bg_fill`: a step lighter than
    // the fields, to show on the panels and on the cards.
    widgets.inactive.bg_fill = color::BG_TERTIARY;
    widgets.hovered = widget(color::BG_TERTIARY, Stroke::NONE, color::TEXT);
    widgets.hovered.bg_fill = Color32::from_rgb(0x55, 0x55, 0x55);
    widgets.active = widget(
        color::BG_TERTIARY,
        Stroke::new(1.0, color::BRAND),
        color::TEXT,
    );
    widgets.open = widget(color::BG_TERTIARY, Stroke::NONE, color::TEXT);

    visuals.selection.bg_fill = color::BRAND;
    visuals.selection.stroke = Stroke::new(1.0, color::TEXT);
    visuals.text_cursor.stroke = Stroke::new(1.5, color::BRAND_TEXT);
    visuals.weak_text_color = Some(color::TEXT_TERTIARY);
    visuals.hyperlink_color = color::BRAND_TEXT;
    visuals.warn_fg_color = color::WARNING;
    visuals.error_fg_color = color::DANGER_TEXT;

    visuals.panel_fill = color::BG;
    visuals.faint_bg_color = color::BG_SECONDARY;
    visuals.extreme_bg_color = color::CANVAS;
    visuals.text_edit_bg_color = Some(color::BG_SECONDARY);
    visuals.code_bg_color = color::BG_SECONDARY;

    visuals.window_fill = color::CANVAS;
    visuals.window_stroke = Stroke::new(1.0, color::BORDER_SUBTLE);
    visuals.window_corner_radius = CornerRadius::same(RADIUS_LARGE);
    visuals.menu_corner_radius = CornerRadius::same(RADIUS_LARGE);
    visuals.window_shadow = Shadow {
        offset: [0, 10],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(115),
    };
    visuals.popup_shadow = Shadow {
        offset: [0, 10],
        blur: 16,
        spread: 0,
        color: Color32::from_black_alpha(90),
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
        assert_eq!(style.visuals.panel_fill, color::BG);
        assert_eq!(style.text_styles[&TextStyle::Heading].family, semibold());
    }
}
