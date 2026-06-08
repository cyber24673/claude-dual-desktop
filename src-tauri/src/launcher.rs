use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use windows::core::HSTRING;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Shell::IApplicationActivationManager;

const PACKAGE_FAMILY: &str = "Claude_pzs8sxrjxfjjc";
const AUMID: &str = "Claude_pzs8sxrjxfjjc!Claude";

/// Files/dirs that define a profile's identity (auth, state, settings).
const PROFILE_ITEMS: &[&str] = &[
    "config.json",
    "Local Storage",
    "Session Storage",
    "IndexedDB",
    "Network",
    "Preferences",
    "Local State",
    "ant-did",
    "lockfile",
    "WebStorage",
    "blob_storage",
];

/// Find the Claude Desktop MSIX install dir dynamically.
/// Scans WindowsApps for the matching package folder.
fn find_msix_app_dir() -> Result<PathBuf, String> {
    let windows_apps = Path::new(r"C:\Program Files\WindowsApps");
    if !windows_apps.exists() {
        return Err("WindowsApps no encontrado".into());
    }

    // Use PowerShell to list the directory (we may not have direct read access)
    let output = Command::new("powershell")
        .args(["-Command", &format!(
            "(Get-AppxPackage -Name 'Claude' | Select-Object -First 1).InstallLocation"
        )])
        .output()
        .map_err(|e| format!("Error buscando Claude: {e}"))?;

    let install_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if install_dir.is_empty() {
        return Err("Claude Desktop no esta instalado".into());
    }

    let app_dir = PathBuf::from(&install_dir).join("app");
    if app_dir.exists() {
        Ok(app_dir)
    } else {
        // Some versions might not have an "app" subfolder
        Ok(PathBuf::from(install_dir))
    }
}

/// The real Electron userData dir for Claude Desktop (MSIX).
fn claude_data_dir() -> PathBuf {
    let local = dirs::data_local_dir().expect("no local app data");
    local.join(format!(r"Packages\{PACKAGE_FAMILY}\LocalCache\Roaming\Claude"))
}

fn app_copy_dir() -> PathBuf {
    crate::profiles::profiles_base().join("_app")
}

fn active_file() -> PathBuf {
    crate::profiles::profiles_base().join("_active.txt")
}

/// Read which profile ID is currently the "primary" (MSIX) instance.
pub fn get_primary_id() -> Option<String> {
    fs::read_to_string(active_file()).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn set_primary_id(id: &str) {
    fs::write(active_file(), id).ok();
}

/// Check if a profile has auth (config.json with oauth:tokenCache).
fn profile_has_auth(profile_dir: &Path) -> bool {
    let config = profile_dir.join("config.json");
    if let Ok(content) = fs::read_to_string(config) {
        return content.contains("oauth:tokenCache");
    }
    false
}

/// Ensure the app copy exists. Returns the path to the copied Claude.exe.
fn ensure_app_copy() -> Result<PathBuf, String> {
    let dst = app_copy_dir();
    let exe = dst.join("Claude.exe");
    if exe.exists() {
        return Ok(exe);
    }

    let src = find_msix_app_dir()?;
    copy_dir_recursive(&src, &dst).map_err(|e| format!("Error copiando app (~465 MB, puede tardar): {e}"))?;
    Ok(exe)
}

/// Launch a profile.
/// - If no other profile is running → use COM (MSIX) with swap approach
/// - If another profile is already primary → use copied exe with --user-data-dir
/// - If this profile is already running → do nothing (already open)
pub fn launch(profile_id: &str, profile_data_dir: &Path) -> Result<u32, String> {
    fs::create_dir_all(profile_data_dir).map_err(|e| e.to_string())?;

    let primary = get_primary_id();
    let msix_running = check_msix_running();

    if !msix_running {
        // No MSIX instance running → launch as primary via COM with swap
        launch_as_primary(profile_id, profile_data_dir)
    } else if primary.as_deref() == Some(profile_id) {
        // This profile IS the primary, MSIX running → relaunch it
        launch_as_primary(profile_id, profile_data_dir)
    } else if profile_has_auth(profile_data_dir) {
        // Another profile is primary, this one has auth → launch via copied exe
        launch_as_secondary(profile_data_dir)
    } else {
        // No auth yet → need to do swap approach for OAuth to work
        // Kill MSIX, swap, launch as primary, user logs in
        launch_as_primary(profile_id, profile_data_dir)
    }
}

/// Launch as primary: kill MSIX, swap data, launch via COM.
fn launch_as_primary(profile_id: &str, profile_data_dir: &Path) -> Result<u32, String> {
    // Kill existing MSIX instance
    kill_claude_msix();
    std::thread::sleep(std::time::Duration::from_millis(2000));

    // Save current MSIX data to previous primary profile
    if let Some(prev_id) = get_primary_id() {
        if prev_id != profile_id {
            let prev_dir = crate::profiles::profile_data_dir(&prev_id);
            save_to_profile(&prev_dir);
        }
    }

    // Load target profile into claude data dir
    load_from_profile(profile_data_dir);
    set_primary_id(profile_id);

    // Launch via COM
    activate_claude_com()
}

/// Launch as secondary: use copied exe with --user-data-dir.
fn launch_as_secondary(profile_data_dir: &Path) -> Result<u32, String> {
    let exe = ensure_app_copy()?;
    let dir_str = profile_data_dir.to_string_lossy().to_string();

    let child = Command::new(exe)
        .arg(format!("--user-data-dir={dir_str}"))
        .arg("--disable-gpu-compositing")
        .arg("--no-sandbox")
        .spawn()
        .map_err(|e| format!("Error lanzando: {e}"))?;

    Ok(child.id())
}

/// Check which profile IDs are currently running.
pub fn get_running_ids(ids: &[String]) -> Vec<String> {
    let mut running = Vec::new();

    // Check primary (MSIX instance)
    if check_msix_running() {
        if let Some(primary) = get_primary_id() {
            if ids.contains(&primary) {
                running.push(primary);
            }
        }
    } else {
        // MSIX died → save data back
        if let Some(primary) = get_primary_id() {
            let dir = crate::profiles::profile_data_dir(&primary);
            save_to_profile(&dir);
            fs::remove_file(active_file()).ok();
        }
    }

    // Check secondary instances (copied exe) via lockfile
    let copy_running = check_copy_running();
    if copy_running {
        for id in ids {
            if running.contains(id) { continue; }
            let lockfile = crate::profiles::profile_data_dir(id).join("lockfile");
            if lockfile.exists() {
                if let Err(_) = fs::OpenOptions::new().read(true).write(true).open(&lockfile) {
                    running.push(id.clone());
                }
            }
        }
    }

    running
}

fn is_profile_running(profile_id: &str, profile_data_dir: &Path) -> bool {
    // Check if it's the primary
    if check_msix_running() {
        if get_primary_id().as_deref() == Some(profile_id) {
            return true;
        }
    }
    // Check if it's a secondary (lockfile locked)
    let lockfile = profile_data_dir.join("lockfile");
    if lockfile.exists() {
        if fs::OpenOptions::new().read(true).write(true).open(&lockfile).is_err() {
            return true;
        }
    }
    false
}

/// Stop the primary (MSIX) instance and save its data back to the profile.
pub fn stop_primary(profile_id: &str) {
    kill_claude_msix();
    std::thread::sleep(std::time::Duration::from_millis(1500));
    let profile_dir = crate::profiles::profile_data_dir(profile_id);
    save_to_profile(&profile_dir);
    fs::remove_file(active_file()).ok();
}

/// Stop a secondary (copied exe) instance.
pub fn stop_secondary(profile_data_dir: &Path) {
    kill_secondary(profile_data_dir);
}

// --- Process detection ---

fn check_msix_running() -> bool {
    find_claude_pids_by_path("WindowsApps").len() > 0
}

fn check_copy_running() -> bool {
    find_claude_pids_by_path("_app").len() > 0
}

fn find_claude_pids_by_path(path_contains: &str) -> Vec<u32> {
    use windows::Win32::System::Diagnostics::ToolHelp::*;
    use windows::Win32::System::Threading::*;
    use windows::Win32::Foundation::CloseHandle;

    let mut pids = Vec::new();
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    let snapshot = match snapshot {
        Ok(h) => h,
        Err(_) => return pids,
    };

    let mut entry = PROCESSENTRY32W::default();
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

    let my_pid = std::process::id();
    unsafe {
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let name = String::from_utf16_lossy(
                    &entry.szExeFile[..entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len())],
                );
                let pid = entry.th32ProcessID;
                if name.eq_ignore_ascii_case("claude.exe") && pid != my_pid {
                    if let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                        let mut buf = [0u16; 1024];
                        let mut size = buf.len() as u32;
                        if QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, windows::core::PWSTR(buf.as_mut_ptr()), &mut size).is_ok() {
                            let path = String::from_utf16_lossy(&buf[..size as usize]);
                            if path.contains(path_contains) {
                                pids.push(pid);
                            }
                        }
                        CloseHandle(handle).ok();
                    }
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        CloseHandle(snapshot).ok();
    }
    pids
}

fn kill_claude_msix() {
    use windows::Win32::System::Threading::*;
    use windows::Win32::Foundation::CloseHandle;

    for pid in find_claude_pids_by_path("WindowsApps") {
        unsafe {
            if let Ok(handle) = OpenProcess(PROCESS_TERMINATE, false, pid) {
                TerminateProcess(handle, 0).ok();
                CloseHandle(handle).ok();
            }
        }
    }
}

/// Kill a secondary instance by killing processes from _app whose lockfile matches.
pub fn kill_secondary(profile_data_dir: &Path) {
    // The lockfile in the profile dir is held by the process.
    // We can't easily match PID to dir, so kill ALL _app processes.
    // This is OK because each secondary relaunches easily.
    use windows::Win32::System::Threading::*;
    use windows::Win32::Foundation::CloseHandle;

    for pid in find_claude_pids_by_path("_app") {
        unsafe {
            if let Ok(handle) = OpenProcess(PROCESS_TERMINATE, false, pid) {
                TerminateProcess(handle, 0).ok();
                CloseHandle(handle).ok();
            }
        }
    }
    // Clean up lockfile
    let lockfile = profile_data_dir.join("lockfile");
    std::thread::sleep(std::time::Duration::from_millis(500));
    fs::remove_file(lockfile).ok();
}

// --- Profile data swap ---

fn save_to_profile(profile_dir: &Path) {
    let claude_dir = claude_data_dir();
    fs::create_dir_all(profile_dir).ok();

    for item in PROFILE_ITEMS {
        let src = claude_dir.join(item);
        let dst = profile_dir.join(item);
        if src.exists() {
            if dst.exists() {
                if dst.is_dir() { fs::remove_dir_all(&dst).ok(); }
                else { fs::remove_file(&dst).ok(); }
            }
            if src.is_dir() {
                copy_dir_recursive(&src, &dst).ok();
            } else {
                fs::copy(&src, &dst).ok();
            }
        }
    }
}

fn load_from_profile(profile_dir: &Path) {
    let claude_dir = claude_data_dir();

    for item in PROFILE_ITEMS {
        let target = claude_dir.join(item);
        if target.exists() {
            if target.is_dir() { fs::remove_dir_all(&target).ok(); }
            else { fs::remove_file(&target).ok(); }
        }
    }

    for item in PROFILE_ITEMS {
        let src = profile_dir.join(item);
        let dst = claude_dir.join(item);
        if src.exists() {
            if src.is_dir() {
                copy_dir_recursive(&src, &dst).ok();
            } else {
                fs::copy(&src, &dst).ok();
            }
        }
    }
}

// --- COM activation ---

fn activate_claude_com() -> Result<u32, String> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok();

        let aam: IApplicationActivationManager = CoCreateInstance(
            &windows::Win32::UI::Shell::ApplicationActivationManager,
            None,
            CLSCTX_LOCAL_SERVER,
        )
        .map_err(|e| format!("COM create failed: {e}"))?;

        let aumid = HSTRING::from(AUMID);
        let args = HSTRING::new();

        let pid = aam
            .ActivateApplication(&aumid, &args, windows::Win32::UI::Shell::AO_NONE)
            .map_err(|e| format!("ActivateApplication failed: {e}"))?;

        Ok(pid)
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
