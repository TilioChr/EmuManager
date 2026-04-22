use crate::controller_profiles::ControllerProfile;
use crate::portable_paths::PortablePaths;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerWriteResult {
    pub emulator_id: String,
    pub profile_id: String,
    pub profile_path: String,
    pub game_ini_path: String,
}

pub fn apply_controller_profile(
    paths: &PortablePaths,
    profile: &ControllerProfile,
) -> Result<ControllerWriteResult, String> {
    match profile.emulator_id.as_str() {
        "dolphin" => apply_dolphin_profile(paths, profile),
        _ => Err(format!(
            "Écriture de profil non implémentée pour {}",
            profile.emulator_id
        )),
    }
}

fn apply_dolphin_profile(
    paths: &PortablePaths,
    profile: &ControllerProfile,
) -> Result<ControllerWriteResult, String> {
    let install_root = PathBuf::from(&paths.emu).join("Dolphin");
    let executable_dir = locate_dolphin_executable_dir(&install_root)?;
    let user_dir = executable_dir.join("User");
    let config_dir = user_dir.join("Config");
    let profiles_dir = config_dir.join("Profiles").join("GCPad");

    fs::create_dir_all(&profiles_dir)
        .map_err(|error| format!("Impossible de créer Profiles/GCPad: {}", error))?;
    fs::create_dir_all(&config_dir)
        .map_err(|error| format!("Impossible de créer User/Config: {}", error))?;

    let profile_file_name = format!("{}.ini", sanitize_profile_name(&profile.name));
    let profile_path = profiles_dir.join(profile_file_name);
    let game_ini_path = config_dir.join("GCPadNew.ini");

    let profile_content = build_dolphin_gc_profile(profile);
    fs::write(&profile_path, profile_content)
        .map_err(|error| format!("Impossible d'écrire le profil Dolphin: {}", error))?;

    let gcpad_content = build_gcpad_ini(profile);
    fs::write(&game_ini_path, gcpad_content)
        .map_err(|error| format!("Impossible d'écrire GCPadNew.ini: {}", error))?;

    Ok(ControllerWriteResult {
        emulator_id: profile.emulator_id.clone(),
        profile_id: profile.id.clone(),
        profile_path: profile_path.to_string_lossy().to_string(),
        game_ini_path: game_ini_path.to_string_lossy().to_string(),
    })
}

fn locate_dolphin_executable_dir(install_root: &Path) -> Result<PathBuf, String> {
    let direct_exe = install_root.join("Dolphin.exe");
    if direct_exe.exists() {
        return Ok(install_root.to_path_buf());
    }

    let nested = install_root.join("Dolphin-x64");
    if nested.join("Dolphin.exe").exists() {
        return Ok(nested);
    }

    Err(format!(
        "Impossible de localiser Dolphin.exe dans {}",
        install_root.to_string_lossy()
    ))
}

fn sanitize_profile_name(input: &str) -> String {
    let cleaned: String = input
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => character,
        })
        .collect();

    if cleaned.trim().is_empty() {
        "EmuManager Profile".to_string()
    } else {
        cleaned
    }
}

fn build_dolphin_gc_profile(profile: &ControllerProfile) -> String {
    let mut lines = vec![
        "[Profile]".to_string(),
        format!("Device = XInput2/0/Virtual pad"),
        format!("Buttons/A = {}", find_binding(profile, "Bouton A", "Button A")),
        format!("Buttons/B = {}", find_binding(profile, "Bouton B", "Button B")),
        format!("Buttons/X = {}", find_binding(profile, "Bouton X", "Button X")),
        format!("Buttons/Y = {}", find_binding(profile, "Bouton Y", "Button Y")),
        format!("Buttons/Z = {}", find_binding(profile, "Z", "Trigger L")),
        format!("Buttons/Start = {}", find_binding(profile, "Start", "Button Start")),
        format!("Triggers/L = {}", find_binding(profile, "Gâchette L", "Trigger L")),
        format!("Triggers/R = {}", find_binding(profile, "Gâchette R", "Trigger R")),
        format!("Main Stick/Up = {}", find_binding(profile, "Stick Haut", "Left Y+")),
        format!("Main Stick/Down = {}", find_binding(profile, "Stick Bas", "Left Y-")),
        format!("Main Stick/Left = {}", find_binding(profile, "Stick Gauche", "Left X-")),
        format!("Main Stick/Right = {}", find_binding(profile, "Stick Droite", "Left X+")),
        format!("C-Stick/Up = {}", find_binding(profile, "C Haut", "Right Y+")),
        format!("C-Stick/Down = {}", find_binding(profile, "C Bas", "Right Y-")),
        format!("C-Stick/Left = {}", find_binding(profile, "C Gauche", "Right X-")),
        format!("C-Stick/Right = {}", find_binding(profile, "C Droite", "Right X+")),
        format!("D-Pad/Up = {}", find_binding(profile, "Croix Haut", "Pad N")),
        format!("D-Pad/Down = {}", find_binding(profile, "Croix Bas", "Pad S")),
        format!("D-Pad/Left = {}", find_binding(profile, "Croix Gauche", "Pad W")),
        format!("D-Pad/Right = {}", find_binding(profile, "Croix Droite", "Pad E")),
    ];

    lines.push(String::new());
    lines.join("\n")
}

fn build_gcpad_ini(profile: &ControllerProfile) -> String {
    let profile_name = sanitize_profile_name(&profile.name);

    [
        "[GCPad1]".to_string(),
        "Device = XInput2/0/Virtual pad".to_string(),
        "Buttons/A = `Profile`".to_string(),
        "Buttons/B = `Profile`".to_string(),
        format!("Profile1 = {}", profile_name),
        "SDL/Background Input = False".to_string(),
        String::new(),
    ]
    .join("\n")
}

fn find_binding(profile: &ControllerProfile, emulated_input: &str, fallback: &str) -> String {
    profile
        .bindings
        .iter()
        .find(|binding| binding.emulated_input.eq_ignore_ascii_case(emulated_input))
        .map(|binding| normalize_physical_input(&binding.physical_input))
        .unwrap_or_else(|| fallback.to_string())
}

fn normalize_physical_input(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "a" => "Button A".to_string(),
        "b" => "Button B".to_string(),
        "x" => "Button X".to_string(),
        "y" => "Button Y".to_string(),
        "lb" | "l1" => "Trigger L".to_string(),
        "rb" | "r1" => "Trigger R".to_string(),
        "lt" | "l2" => "Trigger L".to_string(),
        "rt" | "r2" => "Trigger R".to_string(),
        "start" => "Button Start".to_string(),
        "back" => "Button Back".to_string(),
        "dpad up" => "Pad N".to_string(),
        "dpad down" => "Pad S".to_string(),
        "dpad left" => "Pad W".to_string(),
        "dpad right" => "Pad E".to_string(),
        "left stick up" => "Left Y+".to_string(),
        "left stick down" => "Left Y-".to_string(),
        "left stick left" => "Left X-".to_string(),
        "left stick right" => "Left X+".to_string(),
        "right stick up" => "Right Y+".to_string(),
        "right stick down" => "Right Y-".to_string(),
        "right stick left" => "Right X-".to_string(),
        "right stick right" => "Right X+".to_string(),
        other => other.to_string(),
    }
}