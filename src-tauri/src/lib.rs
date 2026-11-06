mod ipc;
mod capture;
mod device;
mod state;
mod stream;

use std::sync::Arc;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .setup(|app| {
      let app_handle = app.handle().clone();
      if cfg!(debug_assertions) {
        app_handle.plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }

      let stream_state = Arc::new(state::StreamState::new());
      let pipeline = stream::processor::start(app_handle.clone(), stream_state.clone());
      app.manage(stream_state);
      app.manage(pipeline);

      ipc::events::emit_test_event(app.handle());
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      ipc::commands::get_backend_status,
      ipc::commands::request_test_event,
      ipc::commands::start_capture,
      ipc::commands::stop_capture
    ])
    .build(tauri::generate_context!())
    .expect("error while building tauri application")
    .run(|app_handle, event| {
      if let tauri::RunEvent::Exit = event {
        let pipeline = app_handle
          .state::<stream::processor::StreamPipeline>()
          .inner();
        pipeline.stop();
        pipeline.join();
      }
    });
}
