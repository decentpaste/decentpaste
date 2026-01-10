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
use desktop::Decentsecret;
#[cfg(mobile)]
use mobile::Decentsecret;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the decentsecret APIs.
pub trait DecentsecretExt<R: Runtime> {
  fn decentsecret(&self) -> &Decentsecret<R>;
}

impl<R: Runtime, T: Manager<R>> crate::DecentsecretExt<R> for T {
  fn decentsecret(&self) -> &Decentsecret<R> {
    self.state::<Decentsecret<R>>().inner()
  }
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
  Builder::new("decentsecret")
    .invoke_handler(tauri::generate_handler![commands::ping])
    .setup(|app, api| {
      #[cfg(mobile)]
      let decentsecret = mobile::init(app, api)?;
      #[cfg(desktop)]
      let decentsecret = desktop::init(app, api)?;
      app.manage(decentsecret);
      Ok(())
    })
    .build()
}
