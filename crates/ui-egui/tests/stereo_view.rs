use materialcraft_ui_egui::{i18n, stereo_view};

#[test]
fn native_resolution_retains_the_preview_grid_and_explicit_choices() {
    let rect = egui::Rect::from_min_size(egui::pos2(1.2, 2.3), egui::vec2(100.7, 50.6));
    assert_eq!(stereo_view::render_size(None, Some(rect), 1.5).unwrap(), [151, 75]);
    for rect in [rect, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 450.0))] {
        assert_eq!(stereo_view::render_size(Some([640, 360]), Some(rect), 2.0).unwrap(), [640, 360]);
    }
    assert!(stereo_view::render_size(None, None, 1.0).is_err());
    assert!(stereo_view::render_size(Some([0, 360]), Some(rect), 1.0).is_err());
    assert!(stereo_view::render_size(Some([4097, 360]), Some(rect), 1.0).is_err());
    assert!(stereo_view::render_size(Some([640, 360]), Some(rect), f32::NAN).is_err());
}

const LABELS: &[&str] = &[
    "Could not prepare the material scene.",
    "The stereo renderer stopped before completing its pair.",
    "Stereo View",
    "Enable 3D",
    "Render resolution per eye",
    "Desktop Display Settings",
    "Width",
    "Height",
    "Could not show the stereo view: {error}",
    "Could not open Display Settings: {error}",
    "Stereo depth is unavailable on this desktop; showing the flat model.",
    "The stereo view has invalid native dimensions.",
    "The stereo window closed before the pair was shown.",
    "The 3D view size preserves its current sampling; the desktop scales once.",
    "Use 3D view size",
    "Open the 3D View first.",
];

#[test]
fn required_catalogs_resolve_every_new_label_through_the_actual_lookup() {
    if std::env::var("CRAFT_I18N_REQUIRED").as_deref() != Ok("1") {
        return;
    }
    for code in ["pl", "pt-BR", "fr", "de", "es", "ja", "ko", "zh-Hans", "zh-Hant", "ar"] {
        assert!(i18n::available(code), "{code}");
        for key in LABELS {
            let text = i18n::text_for(code, key);
            assert!(!text.is_empty());
            assert_ne!(text, *key, "{code}: {key}");
            if key.contains("{error}") {
                assert!(text.contains("{error}"));
            }
        }
    }
    for (input, code) in [("es_MX.UTF-8", "es"), ("pt_BR", "pt-BR"), ("zh_TW", "zh-Hant"), ("zh-Hans", "zh-Hans"), ("ko_KR", "ko")] {
        assert_eq!(i18n::language_for_locale(input), code);
    }
    assert_ne!(i18n::text_for("zh-Hans", "Stereo View"), i18n::text_for("zh-Hant", "Stereo View"));
}

#[test]
fn new_cjk_labels_have_glyphs_in_both_actual_ui_families() {
    if std::env::var("CRAFT_I18N_REQUIRED").as_deref() != Ok("1") {
        return;
    }
    for code in ["ja", "ko", "zh-Hans", "zh-Hant"] {
        let ctx = egui::Context::default();
        let mut fonts = materialcraft_ui_egui::fonts::UiFonts::default();
        fonts.update(&ctx, code, false);
        ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
        ctx.fonts_mut(|atlas| {
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                let font = egui::FontId::new(18.0, family.clone());
                for key in LABELS {
                    assert!(atlas.has_glyphs(&font, i18n::text_for(code, key)), "{code} {family:?}: {key}");
                }
            }
        });
    }
}
