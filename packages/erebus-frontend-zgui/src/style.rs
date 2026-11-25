use std::sync::Arc;

use zgui::app::Fonts;
use zgui_ui_tokens::Theme;

/// Geist Serif is bundled for the wordmark under the OFL (`fonts/OFL.txt`). Other faces come
/// from the system.
pub fn fonts() -> Fonts {
    let fonts = Fonts::system();
    fonts
        .register(Arc::new(include_bytes!("../fonts/GeistSerif.ttf")), None)
        .expect("the shipped font is valid");
    fonts
}

const EREBUS_CUSTOM_THEME: &str = "
    --zui-type-family-sans: \"Adwaita Sans\", system-ui, sans-serif;
    --zui-type-family-mono: ui-monospace, monospace;

    --zui-color-background: oklch(0.155 0.014 295);
    --zui-color-foreground: oklch(0.96 0.01 295);
    --zui-color-card: oklch(0.195 0.018 295);
    --zui-color-card-foreground: oklch(0.96 0.01 295);
    --zui-color-popover: oklch(0.215 0.02 295);
    --zui-color-popover-foreground: oklch(0.96 0.01 295);

    --zui-color-tooltip: var(--zui-color-popover);
    --zui-color-tooltip-foreground: var(--zui-color-popover-foreground);
    --zui-color-tooltip-border: var(--zui-color-border);

    --zui-color-primary: oklch(0.72 0.17 305);
    --zui-color-primary-foreground: oklch(0.16 0.05 300);
    --zui-color-secondary: oklch(0.26 0.025 295);
    --zui-color-secondary-foreground: oklch(0.96 0.01 295);
    --zui-color-muted: oklch(0.24 0.022 295);
    --zui-color-muted-foreground: oklch(0.72 0.025 295);
    --zui-color-accent: oklch(0.27 0.03 295);
    --zui-color-accent-foreground: oklch(0.97 0.01 295);

    --zui-color-border: oklch(0.29 0.025 295);
    --zui-color-input: oklch(0.42 0.03 295);
    --zui-color-ring: oklch(0.72 0.17 305);
    --zui-color-scrim: oklch(0.08 0.01 295 / 60%);

    --zui-color-sidebar: oklch(0.2 0.022 295);
    --zui-color-sidebar-foreground: oklch(0.95 0.012 295);
    --zui-color-sidebar-primary: oklch(0.72 0.17 305);
    --zui-color-sidebar-primary-foreground: oklch(0.16 0.05 300);
    --zui-color-sidebar-accent: oklch(0.245 0.026 297);
    --zui-color-sidebar-accent-foreground: oklch(0.95 0.012 295);
    --zui-color-sidebar-border: var(--zui-color-border);
    --zui-color-sidebar-ring: oklch(0.72 0.17 305);
";

pub fn theme() -> Theme {
    Theme::dark().with_css(EREBUS_CUSTOM_THEME)
}

pub const SHEET: &str = zgui::css!(
    ":root {
        background-color: transparent;
        color: var(--zui-color-foreground);
        font-family: var(--zui-type-family-sans);
        font-size: var(--zui-type-size-md);
    }"
);
