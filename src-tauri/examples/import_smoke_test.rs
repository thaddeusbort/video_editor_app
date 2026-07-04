//! Manual end-to-end smoke test: boots a real (non-mock) Tauri app - so
//! sidecar resolution, capability permissions, and app-data-dir resolution
//! are all the genuine runtime paths - then calls `import_folder` directly
//! against a folder passed on the command line and prints the outcome.
//!
//! Not part of the shipped app; run with:
//!   IMPORT_SMOKE_TEST_DIR=/path/to/videos cargo run --example import_smoke_test

fn main() {
    let folder = std::env::var("IMPORT_SMOKE_TEST_DIR")
        .expect("set IMPORT_SMOKE_TEST_DIR to a folder of test videos");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let handle = app.handle().clone();
            let folder = folder.clone();
            tauri::async_runtime::spawn(async move {
                let outcome = video_editor_app_lib::commands::import_folder(handle.clone(), folder)
                    .await;
                match outcome {
                    Ok(outcome) => {
                        println!("{}", serde_json::to_string_pretty(&outcome).unwrap());
                    }
                    Err(e) => {
                        eprintln!("import_folder failed: {e}");
                    }
                }
                handle.exit(0);
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
