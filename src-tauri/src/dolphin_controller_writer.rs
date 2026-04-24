use crate::controller_profiles::{load_controller_profiles, ControllerProfile};
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

#[derive(Debug, Clone, Copy)]
enum DolphinControllerKind {
    GameCube,
    Wiimote,
    WiimoteNunchuk,
    Classic,
}

pub fn apply_controller_profile(
    paths: &PortablePaths,
    profile: &ControllerProfile,
) -> Result<ControllerWriteResult, String> {
    match profile.emulator_id.as_str() {
        "dolphin" => apply_dolphin_profile(paths, profile),
        "melonds" => crate::melonds_controller_writer::apply_melonds_profile(paths, profile),
        _ => Err(format!(
            "Ecriture de profil non implementee pour {}",
            profile.emulator_id
        )),
    }
}

pub fn apply_saved_controller_profile(
    paths: &PortablePaths,
    emulator_id: &str,
) -> Result<Option<ControllerWriteResult>, String> {
    let profiles = load_controller_profiles(paths)?;
    let profile = profiles
        .iter()
        .find(|entry| entry.emulator_id == emulator_id);

    match profile {
        Some(profile) => apply_controller_profile(paths, profile).map(Some),
        None => Ok(None),
    }
}

pub fn apply_saved_controller_profile_to_user_dir(
    paths: &PortablePaths,
    emulator_id: &str,
    user_dir: &Path,
) -> Result<Option<ControllerWriteResult>, String> {
    let profiles = load_controller_profiles(paths)?;
    let profile = profiles
        .iter()
        .find(|entry| entry.emulator_id == emulator_id);

    match profile {
        Some(profile) => apply_dolphin_profile_to_user_dir(profile, user_dir).map(Some),
        None => Ok(None),
    }
}

fn apply_dolphin_profile(
    paths: &PortablePaths,
    profile: &ControllerProfile,
) -> Result<ControllerWriteResult, String> {
    let install_root = PathBuf::from(&paths.emu).join("Dolphin");
    let executable_dir = locate_dolphin_executable_dir(&install_root)?;
    let user_dir = executable_dir.join("User");
    apply_dolphin_profile_to_user_dir(profile, &user_dir)
}

fn apply_dolphin_profile_to_user_dir(
    profile: &ControllerProfile,
    user_dir: &Path,
) -> Result<ControllerWriteResult, String> {
    let config_dir = user_dir.join("Config");
    fs::create_dir_all(&config_dir)
        .map_err(|error| format!("Impossible de creer User/Config: {}", error))?;

    match resolve_dolphin_controller_kind(profile) {
        DolphinControllerKind::GameCube => apply_dolphin_gc_profile(profile, &config_dir),
        kind => apply_dolphin_wiimote_profile(profile, kind, &config_dir),
    }
}

fn apply_dolphin_gc_profile(
    profile: &ControllerProfile,
    config_dir: &Path,
) -> Result<ControllerWriteResult, String> {
    let profiles_dir = config_dir.join("Profiles").join("GCPad");
    fs::create_dir_all(&profiles_dir)
        .map_err(|error| format!("Impossible de creer Profiles/GCPad: {}", error))?;

    let profile_file_name = format!("{}.ini", sanitize_profile_name(&profile.name));
    let profile_path = profiles_dir.join(profile_file_name);
    let game_ini_path = config_dir.join("GCPadNew.ini");
    let device = resolve_dolphin_device(profile, &game_ini_path);

    fs::write(
        &profile_path,
        build_dolphin_gc_profile(profile, "[Profile]", &device),
    )
    .map_err(|error| format!("Impossible d'ecrire le profil Dolphin: {}", error))?;
    fs::write(
        &game_ini_path,
        build_dolphin_gc_profile(profile, "[GCPad1]", &device),
    )
    .map_err(|error| format!("Impossible d'ecrire GCPadNew.ini: {}", error))?;

    Ok(ControllerWriteResult {
        emulator_id: profile.emulator_id.clone(),
        profile_id: profile.id.clone(),
        profile_path: profile_path.to_string_lossy().to_string(),
        game_ini_path: game_ini_path.to_string_lossy().to_string(),
    })
}

fn apply_dolphin_wiimote_profile(
    profile: &ControllerProfile,
    kind: DolphinControllerKind,
    config_dir: &Path,
) -> Result<ControllerWriteResult, String> {
    let profiles_dir = config_dir.join("Profiles").join("Wiimote");
    fs::create_dir_all(&profiles_dir)
        .map_err(|error| format!("Impossible de creer Profiles/Wiimote: {}", error))?;

    let profile_file_name = format!("{}.ini", sanitize_profile_name(&profile.name));
    let profile_path = profiles_dir.join(profile_file_name);
    let game_ini_path = config_dir.join("WiimoteNew.ini");
    let device = resolve_dolphin_device(profile, &game_ini_path);

    fs::write(
        &profile_path,
        build_dolphin_wiimote_profile(profile, kind, "[Profile]", &device),
    )
    .map_err(|error| format!("Impossible d'ecrire le profil Wiimote Dolphin: {}", error))?;
    fs::write(
        &game_ini_path,
        build_dolphin_wiimote_ini(profile, kind, &device),
    )
    .map_err(|error| format!("Impossible d'ecrire WiimoteNew.ini: {}", error))?;

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

fn resolve_dolphin_controller_kind(profile: &ControllerProfile) -> DolphinControllerKind {
    match profile.emulated_controller_id.as_deref() {
        Some("wiimote") => DolphinControllerKind::Wiimote,
        Some("wiimote_nunchuk") => DolphinControllerKind::WiimoteNunchuk,
        Some("classic_controller") => DolphinControllerKind::Classic,
        Some("gamecube") => DolphinControllerKind::GameCube,
        _ => {
            let label = profile.emulated_device_label.to_ascii_lowercase();
            if label.contains("classic") {
                DolphinControllerKind::Classic
            } else if label.contains("nunchuk") {
                DolphinControllerKind::WiimoteNunchuk
            } else if label.contains("wiimote") {
                DolphinControllerKind::Wiimote
            } else {
                DolphinControllerKind::GameCube
            }
        }
    }
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

fn build_dolphin_gc_profile(
    profile: &ControllerProfile,
    section: &str,
    device: &str,
) -> String {
    let mut lines = vec![
        section.to_string(),
        format!("Device = {}", device),
        format!(
            "Buttons/A = {}",
            find_binding(profile, "Bouton A", "Button A")
        ),
        format!(
            "Buttons/B = {}",
            find_binding(profile, "Bouton B", "Button B")
        ),
        format!(
            "Buttons/X = {}",
            find_binding(profile, "Bouton X", "Button X")
        ),
        format!(
            "Buttons/Y = {}",
            find_binding(profile, "Bouton Y", "Button Y")
        ),
        format!("Buttons/Z = {}", find_binding(profile, "Z", "Trigger L")),
        format!(
            "Buttons/Start = {}",
            find_binding(profile, "Start", "Button Start")
        ),
        format!(
            "Triggers/L = {}",
            find_binding_any(profile, &["L", "Gachette L", "GÃ¢chette L"], "Trigger L")
        ),
        format!(
            "Triggers/R = {}",
            find_binding_any(profile, &["R", "Gachette R", "GÃ¢chette R"], "Trigger R")
        ),
        format!(
            "Main Stick/Up = {}",
            find_binding(profile, "Stick Haut", "Left Y+")
        ),
        format!(
            "Main Stick/Down = {}",
            find_binding(profile, "Stick Bas", "Left Y-")
        ),
        format!(
            "Main Stick/Left = {}",
            find_binding(profile, "Stick Gauche", "Left X-")
        ),
        format!(
            "Main Stick/Right = {}",
            find_binding(profile, "Stick Droite", "Left X+")
        ),
        format!(
            "C-Stick/Up = {}",
            find_binding(profile, "C Haut", "Right Y+")
        ),
        format!(
            "C-Stick/Down = {}",
            find_binding(profile, "C Bas", "Right Y-")
        ),
        format!(
            "C-Stick/Left = {}",
            find_binding(profile, "C Gauche", "Right X-")
        ),
        format!(
            "C-Stick/Right = {}",
            find_binding(profile, "C Droite", "Right X+")
        ),
        format!(
            "D-Pad/Up = {}",
            find_binding(profile, "Croix Haut", "Pad N")
        ),
        format!(
            "D-Pad/Down = {}",
            find_binding(profile, "Croix Bas", "Pad S")
        ),
        format!(
            "D-Pad/Left = {}",
            find_binding(profile, "Croix Gauche", "Pad W")
        ),
        format!(
            "D-Pad/Right = {}",
            find_binding(profile, "Croix Droite", "Pad E")
        ),
        "SDL/Background Input = False".to_string(),
    ];

    lines.push(String::new());
    lines.join("\n")
}

fn build_dolphin_wiimote_profile(
    profile: &ControllerProfile,
    kind: DolphinControllerKind,
    section: &str,
    device: &str,
) -> String {
    let mut lines = vec![
        section.to_string(),
        "Source = 1".to_string(),
        format!("Device = {}", device),
        format!(
            "Buttons/A = {}",
            find_binding(profile, "Bouton A", "Button A")
        ),
        format!(
            "Buttons/B = {}",
            find_binding(profile, "Bouton B", "Button B")
        ),
        format!(
            "Buttons/1 = {}",
            find_binding(profile, "Bouton 1", "Button X")
        ),
        format!(
            "Buttons/2 = {}",
            find_binding(profile, "Bouton 2", "Button Y")
        ),
        format!(
            "Buttons/- = {}",
            find_binding_any(profile, &["Minus", "Select"], "Button Back")
        ),
        format!(
            "Buttons/+ = {}",
            find_binding_any(profile, &["Plus", "Start"], "Button Start")
        ),
        format!("Buttons/Home = {}", find_binding(profile, "Home", "Guide")),
        format!(
            "D-Pad/Up = {}",
            find_binding(profile, "Croix Haut", "Pad N")
        ),
        format!(
            "D-Pad/Down = {}",
            find_binding(profile, "Croix Bas", "Pad S")
        ),
        format!(
            "D-Pad/Left = {}",
            find_binding(profile, "Croix Gauche", "Pad W")
        ),
        format!(
            "D-Pad/Right = {}",
            find_binding(profile, "Croix Droite", "Pad E")
        ),
        format!(
            "IR/Auto-Hide = {}",
            bool_to_dolphin(dolphin_ir_auto_hide(profile))
        ),
        format!(
            "IR/Relative Input = {}",
            bool_to_dolphin(dolphin_ir_relative_input(profile))
        ),
        format!("IR/Up = {}", find_binding(profile, "IR Haut", "Right Y+")),
        format!("IR/Down = {}", find_binding(profile, "IR Bas", "Right Y-")),
        format!(
            "IR/Left = {}",
            find_binding(profile, "IR Gauche", "Right X-")
        ),
        format!(
            "IR/Right = {}",
            find_binding(profile, "IR Droite", "Right X+")
        ),
        format!(
            "IR/Recenter = {}",
            find_binding(profile, "IR Recentrer", "Thumb R")
        ),
        format!(
            "IMUIR/Recenter = {}",
            find_binding(profile, "IR Recentrer", "Thumb R")
        ),
        format!(
            "Shake/X = {}",
            find_binding(profile, "Secouer X", "Button A")
        ),
        format!(
            "Shake/Y = {}",
            find_binding(profile, "Secouer Y", "Button B")
        ),
        format!(
            "Shake/Z = {}",
            find_binding(profile, "Secouer Z", "Button X")
        ),
    ];

    match kind {
        DolphinControllerKind::WiimoteNunchuk => {
            lines.extend([
                "Extension = Nunchuk".to_string(),
                format!(
                    "Nunchuk/Buttons/C = {}",
                    find_binding(profile, "Nunchuk C", "Trigger L")
                ),
                format!(
                    "Nunchuk/Buttons/Z = {}",
                    find_binding(profile, "Nunchuk Z", "Trigger R")
                ),
                format!(
                    "Nunchuk/Stick/Up = {}",
                    find_binding(profile, "Nunchuk Haut", "Left Y+")
                ),
                format!(
                    "Nunchuk/Stick/Down = {}",
                    find_binding(profile, "Nunchuk Bas", "Left Y-")
                ),
                format!(
                    "Nunchuk/Stick/Left = {}",
                    find_binding(profile, "Nunchuk Gauche", "Left X-")
                ),
                format!(
                    "Nunchuk/Stick/Right = {}",
                    find_binding(profile, "Nunchuk Droite", "Left X+")
                ),
                "Nunchuk/Stick/Calibration = 100.00 141.42 100.00 141.42 100.00 141.42 100.00 141.42".to_string(),
            ]);
        }
        DolphinControllerKind::Classic => {
            lines.extend([
                "Extension = Classic".to_string(),
                format!(
                    "Classic/Buttons/A = {}",
                    find_binding(profile, "Bouton A", "Button A")
                ),
                format!(
                    "Classic/Buttons/B = {}",
                    find_binding(profile, "Bouton B", "Button B")
                ),
                format!(
                    "Classic/Buttons/X = {}",
                    find_binding(profile, "Bouton X", "Button X")
                ),
                format!(
                    "Classic/Buttons/Y = {}",
                    find_binding(profile, "Bouton Y", "Button Y")
                ),
                format!(
                    "Classic/Buttons/- = {}",
                    find_binding_any(profile, &["Minus", "Select"], "Button Back")
                ),
                format!(
                    "Classic/Buttons/+ = {}",
                    find_binding_any(profile, &["Plus", "Start"], "Button Start")
                ),
                format!(
                    "Classic/Buttons/Home = {}",
                    find_binding(profile, "Home", "Guide")
                ),
                format!(
                    "Classic/Triggers/L = {}",
                    find_binding_any(profile, &["L", "Gachette L", "GÃ¢chette L"], "Trigger L")
                ),
                format!(
                    "Classic/Triggers/R = {}",
                    find_binding_any(profile, &["R", "Gachette R", "GÃ¢chette R"], "Trigger R")
                ),
                format!(
                    "Classic/Triggers/ZL = {}",
                    find_binding(profile, "ZL", "Trigger L")
                ),
                format!(
                    "Classic/Triggers/ZR = {}",
                    find_binding(profile, "ZR", "Trigger R")
                ),
                format!(
                    "Classic/D-Pad/Up = {}",
                    find_binding(profile, "Croix Haut", "Pad N")
                ),
                format!(
                    "Classic/D-Pad/Down = {}",
                    find_binding(profile, "Croix Bas", "Pad S")
                ),
                format!(
                    "Classic/D-Pad/Left = {}",
                    find_binding(profile, "Croix Gauche", "Pad W")
                ),
                format!(
                    "Classic/D-Pad/Right = {}",
                    find_binding(profile, "Croix Droite", "Pad E")
                ),
                format!(
                    "Classic/Left Stick/Up = {}",
                    find_binding(profile, "Stick Haut", "Left Y+")
                ),
                format!(
                    "Classic/Left Stick/Down = {}",
                    find_binding(profile, "Stick Bas", "Left Y-")
                ),
                format!(
                    "Classic/Left Stick/Left = {}",
                    find_binding(profile, "Stick Gauche", "Left X-")
                ),
                format!(
                    "Classic/Left Stick/Right = {}",
                    find_binding(profile, "Stick Droite", "Left X+")
                ),
                format!(
                    "Classic/Right Stick/Up = {}",
                    find_binding(profile, "Stick Droit Haut", "Right Y+")
                ),
                format!(
                    "Classic/Right Stick/Down = {}",
                    find_binding(profile, "Stick Droit Bas", "Right Y-")
                ),
                format!(
                    "Classic/Right Stick/Left = {}",
                    find_binding(profile, "Stick Droit Gauche", "Right X-")
                ),
                format!(
                    "Classic/Right Stick/Right = {}",
                    find_binding(profile, "Stick Droit Droite", "Right X+")
                ),
            ]);
        }
        _ => {}
    }

    lines.push(String::new());
    lines.join("\n")
}

fn build_dolphin_wiimote_ini(
    profile: &ControllerProfile,
    kind: DolphinControllerKind,
    device: &str,
) -> String {
    let mut content = build_dolphin_wiimote_profile(profile, kind, "[Wiimote1]", device);
    content.push_str("[Wiimote2]\nSource = 0\n");
    content.push_str(&format!("Device = {}\n", keyboard_device()));
    content.push_str("[Wiimote3]\nSource = 0\n");
    content.push_str(&format!("Device = {}\n", keyboard_device()));
    content.push_str("[Wiimote4]\nSource = 0\n");
    content.push_str(&format!("Device = {}\n", keyboard_device()));
    content.push_str("[BalanceBoard]\nSource = 0\n");
    content.push_str(&format!("Device = {}\n", keyboard_device()));
    content
}

fn find_binding(profile: &ControllerProfile, emulated_input: &str, fallback: &str) -> String {
    find_binding_any(profile, &[emulated_input], fallback)
}

fn find_binding_any(
    profile: &ControllerProfile,
    emulated_inputs: &[&str],
    fallback: &str,
) -> String {
    let physical_input = profile
        .bindings
        .iter()
        .find(|binding| {
            emulated_inputs
                .iter()
                .any(|candidate| binding.emulated_input.eq_ignore_ascii_case(candidate))
        })
        .map(|binding| normalize_physical_input(profile, &binding.physical_input))
        .unwrap_or_else(|| fallback.to_string());

    quote_dolphin_control(&physical_input)
}

fn dolphin_device(profile: &ControllerProfile) -> String {
    if profile
        .physical_device_id
        .as_deref()
        .unwrap_or_default()
        .eq_ignore_ascii_case("keyboard")
    {
        return keyboard_device();
    }

    let original_label = profile.physical_device_label.trim();
    let label = clean_physical_device_label(original_label);
    let original_lower_label = original_label.to_ascii_lowercase();
    let lower_label = label.to_ascii_lowercase();

    if is_dualshock_4_label(&original_lower_label) {
        return "SDL/0/PS4 Controller".to_string();
    }

    if is_dualsense_label(&original_lower_label) {
        return "SDL/0/PS5 Controller".to_string();
    }

    if lower_label.contains("xbox") || lower_label.contains("xinput") {
        return "XInput/0/Gamepad".to_string();
    }

    format!(
        "SDL/{}/{}",
        0,
        if label.is_empty() { "Gamepad" } else { &label }
    )
}

fn resolve_dolphin_device(profile: &ControllerProfile, current_ini_path: &Path) -> String {
    read_existing_dolphin_device(current_ini_path)
        .filter(|device| should_keep_existing_dolphin_device(profile, device))
        .unwrap_or_else(|| dolphin_device(profile))
}

fn read_existing_dolphin_device(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;

    raw.lines()
        .find_map(|line| line.trim().strip_prefix("Device = "))
        .map(str::trim)
        .filter(|device| !device.is_empty())
        .map(str::to_string)
}

fn should_keep_existing_dolphin_device(profile: &ControllerProfile, device: &str) -> bool {
    let lower_device = device.to_ascii_lowercase();
    let lower_label = profile.physical_device_label.to_ascii_lowercase();

    if lower_device.contains("[disconnected]") {
        return false;
    }

    if profile
        .physical_device_id
        .as_deref()
        .unwrap_or_default()
        .eq_ignore_ascii_case("keyboard")
    {
        return lower_device.contains("keyboard mouse");
    }

    if is_dualshock_4_label(&lower_label) {
        return lower_device == "sdl/0/ps4 controller";
    }

    if is_dualsense_label(&lower_label) {
        return lower_device == "sdl/0/ps5 controller";
    }

    lower_device.starts_with("sdl/")
}

fn keyboard_device() -> String {
    "DInput/0/Keyboard Mouse".to_string()
}

fn dolphin_ir_auto_hide(profile: &ControllerProfile) -> bool {
    profile
        .dolphin_settings
        .as_ref()
        .map(|settings| settings.ir_auto_hide)
        .unwrap_or(true)
}

fn dolphin_ir_relative_input(profile: &ControllerProfile) -> bool {
    profile
        .dolphin_settings
        .as_ref()
        .map(|settings| settings.ir_relative_input)
        .unwrap_or(true)
}

fn bool_to_dolphin(value: bool) -> &'static str {
    if value {
        "True"
    } else {
        "False"
    }
}

fn is_dualshock_4_label(lower_label: &str) -> bool {
    lower_label.contains("dualshock")
        || lower_label.contains("dual shock")
        || lower_label.contains("ps4 controller")
        || lower_label.contains("product: 09cc")
        || lower_label.contains("product: 05c4")
        || lower_label.contains("vendor: 054c product: 09cc")
        || lower_label.contains("vendor: 054c product: 05c4")
        || lower_label == "xbox 360 controller (xinput standard gamepad)"
}

fn is_dualsense_label(lower_label: &str) -> bool {
    lower_label.contains("dualsense")
        || lower_label.contains("dual sense")
        || lower_label.contains("ps5 controller")
        || lower_label.contains("vendor: 054c product: 0ce6")
}

fn clean_physical_device_label(label: &str) -> String {
    label
        .split(" (")
        .next()
        .unwrap_or(label)
        .trim()
        .to_string()
}

fn quote_dolphin_control(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.starts_with('`') || trimmed.contains('|') {
        return trimmed.to_string();
    }

    if trimmed.chars().any(char::is_whitespace) {
        return format!("`{}`", trimmed.replace('`', ""));
    }

    trimmed.to_string()
}

fn normalize_physical_input(profile: &ControllerProfile, value: &str) -> String {
    if profile
        .physical_device_id
        .as_deref()
        .unwrap_or_default()
        .eq_ignore_ascii_case("keyboard")
    {
        return normalize_keyboard_mouse_input(value);
    }

    normalize_gamepad_input(value)
}

fn normalize_keyboard_mouse_input(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "enter" | "return" | "numpad enter" => "RETURN".to_string(),
        "escape" | "esc" => "ESCAPE".to_string(),
        "space" | " " => "SPACE".to_string(),
        "tab" => "TAB".to_string(),
        "backspace" => "BACK".to_string(),
        "delete" | "del" => "DELETE".to_string(),
        "shift" => "LSHIFT".to_string(),
        "control" | "ctrl" => "LCONTROL".to_string(),
        "alt" => "LALT".to_string(),
        "arrowup" | "dpad up" => "UP".to_string(),
        "arrowdown" | "dpad down" => "DOWN".to_string(),
        "arrowleft" | "dpad left" => "LEFT".to_string(),
        "arrowright" | "dpad right" => "RIGHT".to_string(),
        "*" | "numpad multiply" => "BACKSLASH".to_string(),
        "mouse left" => "Click 0".to_string(),
        "mouse right" => "Click 1".to_string(),
        "mouse middle" => "Click 2".to_string(),
        "mouse back" => "Click 3".to_string(),
        "mouse forward" => "Click 4".to_string(),
        "mouse up move" => "Cursor Y-".to_string(),
        "mouse down move" => "Cursor Y+".to_string(),
        "mouse left move" => "Cursor X-".to_string(),
        "mouse right move" => "Cursor X+".to_string(),
        other if other.len() == 1 => other.to_ascii_uppercase(),
        other => other.to_ascii_uppercase(),
    }
}

fn normalize_gamepad_input(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "a" => "Button A".to_string(),
        "b" => "Button B".to_string(),
        "x" => "Button X".to_string(),
        "y" => "Button Y".to_string(),
        "lb" | "l1" => "Shoulder L".to_string(),
        "rb" | "r1" => "Shoulder R".to_string(),
        "lt" | "l2" | "left trigger" => "Trigger L".to_string(),
        "rt" | "r2" | "right trigger" => "Trigger R".to_string(),
        "start" | "plus" => "Start".to_string(),
        "back" | "select" | "minus" => "Back".to_string(),
        "home" | "guide" => "Guide".to_string(),
        "left stick" | "thumb l" => "Thumb L".to_string(),
        "right stick" | "thumb r" => "Thumb R".to_string(),
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
