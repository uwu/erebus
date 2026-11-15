//! glue between the engine and the frontend connection, where we load themes from disk
//! in response to sync packets from the frontend

use std::{
    collections::HashMap,
    fs,
    sync::{Mutex, MutexGuard, Once, PoisonError},
};

use erebus_ipc::config::{ConfigStore, ThemeEntry};
use erebus_theme::settings::SettingValue;

use crate::{
    engine::{ThemeCtx, ThemeModule},
    ipc::{self, IpcBackend},
};

static ACTIVE_THEMES: Mutex<Vec<ThemeCtx>> = Mutex::new(Vec::new());
pub fn active_themes() -> MutexGuard<'static, Vec<ThemeCtx>> {
    ACTIVE_THEMES.lock().unwrap_or_else(PoisonError::into_inner)
}

struct Compiled {
    wasm: Vec<u8>,
    module: ThemeModule,
}

/// Loads the themes on disk, then keeps them in sync with whatever frontend is running.
/// `backend` is the name the frontend shows for it. Only the first call does anything.
pub fn start(backend: &str) {
    static START: Once = Once::new();

    START.call_once(|| {
        let mut cache = Vec::new();
        sync(&mut cache);

        if let Err(err) = IpcBackend::new(backend).spawn(move || sync(&mut cache)) {
            log::error!("failed to spawn ipc thread: {err}");
        }
    });
}

/// Rebuild the theme stack from the config directory on disk.
fn sync(cache: &mut Vec<Compiled>) {
    let store = match ConfigStore::load() {
        Ok(store) => store,
        Err(err) => {
            log::error!("failed to load config store: {err}");
            return;
        }
    };

    let mut compiled = Vec::new();
    let mut stack = Vec::new();

    let app = store.apps.get(ipc::process_name());
    let enabled = store.themes.iter().filter_map(|entry| {
        let theme = app?.themes.get(&entry.id).filter(|theme| theme.enabled)?;
        Some((entry, &theme.settings))
    });

    for (entry, settings) in enabled {
        match load(entry, settings, cache) {
            Ok((theme, ctx)) => {
                compiled.push(theme);
                stack.push(ctx);
            }
            Err(err) => log::error!("failed to load theme {:?}: {err}", entry.id),
        }
    }

    *cache = compiled;

    // swap first so the old stack is dropped outside the lock
    let previous = std::mem::replace(&mut *active_themes(), stack);
    drop(previous);
}

fn load(
    entry: &ThemeEntry,
    settings: &HashMap<String, SettingValue>,
    cache: &[Compiled],
) -> wasmtime::Result<(Compiled, ThemeCtx)> {
    let wasm = fs::read(entry.binary_path()?)?;

    let module = match cache.iter().find(|compiled| compiled.wasm == wasm) {
        Some(compiled) => compiled.module.clone(),
        None => ThemeModule::from_bytes(&wasm)?,
    };

    let mut ctx = module.instantiate()?;
    ctx.init(settings)?;

    Ok((Compiled { wasm, module }, ctx))
}
