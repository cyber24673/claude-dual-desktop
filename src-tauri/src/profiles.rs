use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub color: String,
    pub created: String,
}

pub fn profiles_base() -> PathBuf {
    dirs::home_dir()
        .expect("no home dir")
        .join(".claude-desktop-profiles")
}

fn profiles_json() -> PathBuf {
    profiles_base().join("profiles.json")
}

pub fn profile_data_dir(id: &str) -> PathBuf {
    profiles_base().join(id)
}

fn read_all() -> Vec<Profile> {
    let path = profiles_json();
    if !path.exists() {
        return vec![];
    }
    let data = fs::read_to_string(&path).unwrap_or_default();
    serde_json::from_str(&data).unwrap_or_default()
}

fn write_all(profiles: &[Profile]) {
    let base = profiles_base();
    fs::create_dir_all(&base).ok();
    let data = serde_json::to_string_pretty(profiles).unwrap();
    fs::write(profiles_json(), data).ok();
}

pub fn list() -> Vec<Profile> {
    read_all()
}

pub fn create(name: &str, color: &str) -> Result<Profile, String> {
    let mut all = read_all();

    if all.iter().any(|p| p.name == name) {
        return Err(format!("Ya existe un perfil con el nombre '{name}'"));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let data_dir = profile_data_dir(&id);
    fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;

    let profile = Profile {
        id,
        name: name.to_string(),
        color: color.to_string(),
        created: chrono::Local::now().to_rfc3339(),
    };
    all.push(profile.clone());
    write_all(&all);
    Ok(profile)
}

pub fn delete(id: &str) -> Result<(), String> {
    let mut all = read_all();
    let before = all.len();
    all.retain(|p| p.id != id);
    if all.len() == before {
        return Err("Perfil no encontrado".into());
    }
    write_all(&all);

    let data_dir = profile_data_dir(id);
    if data_dir.exists() {
        fs::remove_dir_all(&data_dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}
