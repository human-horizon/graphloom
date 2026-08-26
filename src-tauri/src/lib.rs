pub mod analyzer;
pub mod commands;
pub mod dsl;
pub mod graph;
pub mod llm;
pub mod pipeline;
pub mod render;
pub mod semantic;
pub mod settings;
pub mod state;
pub mod testrun;
pub mod ucm;
pub mod validate;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::check_connection,
            commands::get_palette,
            commands::save_palette,
            commands::analyze_project,
            commands::analyze_function,
            commands::analyze_file,
            commands::get_update_plan,
            commands::update_file,
            commands::rerender_report,
            commands::get_file_tree,
            commands::get_symbols,
            commands::rebuild_model,
            commands::list_reports,
            commands::read_report,
            commands::detect_test_commands,
            commands::run_test,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
