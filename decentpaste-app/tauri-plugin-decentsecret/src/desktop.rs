use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::models::*;

pub fn init<R: Runtime, C: DeserializeOwned>(
  app: &AppHandle<R>,
  _api: PluginApi<R, C>,
) -> crate::Result<Decentsecret<R>> {
  Ok(Decentsecret(app.clone()))
}

/// Access to the decentsecret APIs.
pub struct Decentsecret<R: Runtime>(AppHandle<R>);

impl<R: Runtime> Decentsecret<R> {
  pub fn ping(&self, payload: PingRequest) -> crate::Result<PingResponse> {
    Ok(PingResponse {
      value: payload.value,
    })
  }
}
