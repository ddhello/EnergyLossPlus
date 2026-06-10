mod api;
mod cache;
mod command;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            command::calculate_goal,
            command::load_cached_snapshot,
            command::sync_snapshot,
            command::save_session,
            command::clear_session,
            command::auth_post,
            command::update_goal,
            command::create_food,
            command::create_exercise,
            command::create_weight,
            command::update_food,
            command::delete_food,
            command::update_exercise,
            command::delete_exercise,
            command::update_weight,
            command::delete_weight
        ])
        .run(tauri::generate_context!())
        .expect("failed to run EnergyLossPlus");
}
