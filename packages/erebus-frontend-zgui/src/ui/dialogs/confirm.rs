use zgui::prelude::*;
use zgui_ui::prelude::*;

use crate::state::{AppState, Confirm};

#[component]
pub fn ConfirmDialog() -> impl IntoView {
    let app = AppState::get();
    let confirm = app.confirm;
    let open = Binding::controlled(
        Signal::derive_local(move || confirm.with(Option::is_some)),
        move |open: bool| {
            if !open {
                confirm.set(None);
            }
        },
    );
    let field = move |get: fn(&Confirm) -> String| {
        move || confirm.with(|confirm| confirm.as_ref().map(get).unwrap_or_default())
    };
    let body = move || {
        confirm.with(|confirm| {
            confirm
                .iter()
                .flat_map(|confirm| confirm.body.clone())
                .map(|paragraph| view! { AlertDialogDescription {{paragraph}} }.into_view())
                .collect::<Vec<_>>()
        })
    };

    view! {
        AlertDialog(open = open) {
            AlertDialogContent(size = AlertDialogSize::Sm) {
                AlertDialogHeader {
                    AlertDialogTitle {{field(|confirm| confirm.title.clone())}}
                    {body}
                }
                AlertDialogFooter {
                    AlertDialogCancel {"Cancel"}
                    AlertDialogAction(
                        variant = ButtonVariant::Destructive,
                        on:click = move |_| {
                            if let Some(pending) = confirm.get_untracked() {
                                confirm.set(None);
                                (pending.on_confirm)();
                            }
                        }
                    ) {
                        {field(|confirm| confirm.action.clone())}
                    }
                }
            }
        }
    }
}
