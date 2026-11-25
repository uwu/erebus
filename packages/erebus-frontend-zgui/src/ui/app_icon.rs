//! Installed apps supply an icon name or a path. Themes get a monogram instead.
//! The icons used by erebus itself live in `octicons`.

use std::{
    cell::RefCell,
    collections::HashMap,
    path::{Path, PathBuf},
    rc::Rc,
};

use zgui::prelude::*;
use zgui::style;
use zgui_ui_icons::prelude::*;

use crate::components::octicons::BROWSER;

enum Source {
    Svg(Rc<str>),
    Raster(String),
}

thread_local! {
    static RESOLVED: RefCell<HashMap<String, Option<Rc<Source>>>> = RefCell::default();
}

style! { AppIconStyle => r#"
    /* overflow: zgui counts an image's natural size as overflow, which made lists scroll
       sideways */
    :scope {
        flex: 0 0 auto;
        object-fit: contain;
        overflow: hidden;
    }

    :scope.fallback {
        align-items: center;
        justify-content: center;
    }
"# }

/// An app's icon from an icon name or path, like a desktop entry's `Icon`.
#[component]
pub fn AppIcon(icon: Option<String>, #[prop(default = 24.0)] size: f32) -> impl IntoView {
    install_stylesheet(AppIconStyle::CLASS, AppIconStyle::CSS);
    let size = format!("{size}px");
    let source = icon.as_deref().and_then(lookup);

    match source.as_deref() {
        Some(Source::Svg(document)) => AnyView::new(view! {
            vector(
                class = AppIconStyle::CLASS,
                style:width = size.clone(),
                style:height = size,
                prop:svg = document.to_string(),
                a11y:hidden = true
            )
        }),
        Some(Source::Raster(path)) => AnyView::new(view! {
            image(
                class = AppIconStyle::CLASS,
                style:width = size.clone(),
                style:height = size,
                src = path.clone(),
                alt = String::new()
            )
        }),
        None => AnyView::new(view! {
            box(class = AppIconStyle::CLASS, class:fallback = true, style:width = size.clone(), style:height = size) {
                Icon(icon = BROWSER, label = "")
            }
        }),
    }
}

fn lookup(icon: &str) -> Option<Rc<Source>> {
    RESOLVED.with(|resolved| {
        resolved
            .borrow_mut()
            .entry(icon.to_owned())
            .or_insert_with(|| {
                let path = resolve(icon)?;
                let source = if path.extension().is_some_and(|ext| ext == "svg") {
                    Source::Svg(std::fs::read_to_string(&path).ok()?.into())
                } else {
                    Source::Raster(path.to_string_lossy().into_owned())
                };
                Some(Rc::new(source))
            })
            .clone()
    })
}

fn resolve(icon: &str) -> Option<PathBuf> {
    let path = Path::new(icon);
    if path.is_absolute() {
        return path.is_file().then(|| path.to_owned());
    }

    resolve_name(icon)
}

/// The Windows scanner has already extracted the icon to a file.
#[cfg(not(target_os = "linux"))]
fn resolve_name(_icon: &str) -> Option<PathBuf> {
    None
}

/// Look up named icons in hicolor, where apps install their own icons.
#[cfg(target_os = "linux")]
fn resolve_name(icon: &str) -> Option<PathBuf> {
    let dirs = erebus_frontend::inject::installed::Dirs::from_env().ok()?;
    let bases: Vec<PathBuf> = std::iter::once(dirs.data_home.clone())
        .chain(dirs.data_dirs.iter().cloned())
        .collect();

    const SIZES: [&str; 8] = [
        "scalable", "256x256", "128x128", "96x96", "64x64", "48x48", "32x32", "512x512",
    ];
    let themes = bases
        .iter()
        .map(|base| base.join("icons/hicolor"))
        .chain(std::iter::once(dirs.home.join(".icons/hicolor")));

    for theme in themes {
        for size in SIZES {
            for ext in ["svg", "png"] {
                let candidate = theme.join(size).join("apps").join(format!("{icon}.{ext}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }

    bases
        .iter()
        .map(|base| base.join("pixmaps"))
        .flat_map(|dir| ["svg", "png"].map(|ext| dir.join(format!("{icon}.{ext}"))))
        .find(|candidate| candidate.is_file())
}
