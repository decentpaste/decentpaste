use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

pub use models::*;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod commands;
mod error;
mod models;

pub use error::{Error, Result};

#[cfg(desktop)]
use desktop::Decentshare;
#[cfg(mobile)]
use mobile::Decentshare;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the decentshare APIs.
pub trait DecentshareExt<R: Runtime> {
    fn decentshare(&self) -> &Decentshare<R>;
}

impl<R: Runtime, T: Manager<R>> crate::DecentshareExt<R> for T {
    fn decentshare(&self) -> &Decentshare<R> {
        self.state::<Decentshare<R>>().inner()
    }
}

/// Initializes the decentshare plugin.
///
/// This plugin enables Android "share with" functionality:
/// - Registers as a share target in Android's share sheet
/// - Intercepts shared text via onNewIntent()
/// - Provides commands to check for pending shared content
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("decentshare")
        .invoke_handler(tauri::generate_handler![
            commands::get_pending_share,
            commands::clear_pending_share,
        ])
        .setup(|app, api| {
            #[cfg(mobile)]
            let decentshare = mobile::init(app, api)?;
            #[cfg(desktop)]
            let decentshare = desktop::init(app, api)?;
            app.manage(decentshare);
            Ok(())
        })
        .build()
}
