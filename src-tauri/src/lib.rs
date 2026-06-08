mod launcher;
mod profiles;

#[tauri::command]
fn list_profiles() -> Vec<profiles::Profile> {
    profiles::list()
}

#[tauri::command]
fn create_profile(name: String, color: String) -> Result<profiles::Profile, String> {
    profiles::create(&name, &color)
}

#[tauri::command]
fn stop_profile(id: String) -> Result<(), String> {
    let dir = profiles::profile_data_dir(&id);
    // If it's the primary, kill MSIX and save data back
    if launcher::get_primary_id().as_deref() == Some(id.as_str()) {
        launcher::stop_primary(&id);
    }
    // If it's a secondary, kill its copy processes
    launcher::stop_secondary(&dir);
    Ok(())
}

#[tauri::command]
fn delete_profile(id: String) -> Result<(), String> {
    // Stop the profile first if running
    stop_profile(id.clone())?;
    std::thread::sleep(std::time::Duration::from_millis(1500));
    profiles::delete(&id)
}

#[tauri::command]
fn launch_profile(id: String) -> Result<u32, String> {
    let dir = profiles::profile_data_dir(&id);
    launcher::launch(&id, &dir)
}

#[tauri::command]
fn get_running() -> Vec<String> {
    let all = profiles::list();
    let ids: Vec<String> = all.iter().map(|p| p.id.clone()).collect();
    launcher::get_running_ids(&ids)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            list_profiles,
            create_profile,
            delete_profile,
            stop_profile,
            launch_profile,
            get_running,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
