use crate::controller_profile_writer::ControllerWriteResult;
use crate::controller_profiles::ControllerProfile;
use crate::portable_paths::PortablePaths;
use std::fs;
use std::path::{Path, PathBuf};
use toml_edit::{value, DocumentMut};

const MELONDS_CONFIG_FILE: &str = "melonDS.toml";
const QT_KEYPAD_MODIFIER: i32 = 0x20000000;
const MELONDS_NO_BUTTON: i32 = 0xFFFF;
const MELONDS_AXIS_FLAG: i32 = 0x10000;

struct MelonDsInput {
    key: &'static str,
    aliases: &'static [&'static str],
}

const MELONDS_INPUTS: &[MelonDsInput] = &[
    MelonDsInput {
        key: "A",
        aliases: &["Bouton A", "A"],
    },
    MelonDsInput {
        key: "B",
        aliases: &["Bouton B", "B"],
    },
    MelonDsInput {
        key: "X",
        aliases: &["Bouton X", "X"],
    },
    MelonDsInput {
        key: "Y",
        aliases: &["Bouton Y", "Y"],
    },
    MelonDsInput {
        key: "L",
        aliases: &["L", "L1", "Gachette L"],
    },
    MelonDsInput {
        key: "R",
        aliases: &["R", "R1", "Gachette R"],
    },
    MelonDsInput {
        key: "Select",
        aliases: &["Select", "Minus"],
    },
    MelonDsInput {
        key: "Start",
        aliases: &["Start", "Plus"],
    },
    MelonDsInput {
        key: "Up",
        aliases: &["Croix Haut", "DPad Up"],
    },
    MelonDsInput {
        key: "Down",
        aliases: &["Croix Bas", "DPad Down"],
    },
    MelonDsInput {
        key: "Left",
        aliases: &["Croix Gauche", "DPad Left"],
    },
    MelonDsInput {
        key: "Right",
        aliases: &["Croix Droite", "DPad Right"],
    },
];

pub fn apply_melonds_profile(
    paths: &PortablePaths,
    profile: &ControllerProfile,
) -> Result<ControllerWriteResult, String> {
    let install_root = PathBuf::from(&paths.emu).join("melonDS");
    let config_dir = locate_melonds_config_dir(&install_root)?;
    fs::create_dir_all(&config_dir)
        .map_err(|error| format!("Impossible de creer le dossier melonDS: {}", error))?;

    let config_path = config_dir.join(MELONDS_CONFIG_FILE);
    let raw_config = fs::read_to_string(&config_path).unwrap_or_else(|_| String::new());
    let mut document = raw_config
        .parse::<DocumentMut>()
        .unwrap_or_else(|_| DocumentMut::new());

    if is_keyboard_profile(profile) {
        clear_melonds_section(&mut document, "Joystick");
        apply_melonds_keyboard_profile(&mut document, profile);
    } else {
        clear_melonds_section(&mut document, "Keyboard");
        apply_melonds_joystick_profile(&mut document, profile);
    }

    fs::write(&config_path, document.to_string())
        .map_err(|error| format!("Impossible d'ecrire melonDS.toml: {}", error))?;

    Ok(ControllerWriteResult {
        emulator_id: profile.emulator_id.clone(),
        profile_id: profile.id.clone(),
        profile_path: config_path.to_string_lossy().to_string(),
        game_ini_path: config_path.to_string_lossy().to_string(),
    })
}

fn locate_melonds_config_dir(install_root: &Path) -> Result<PathBuf, String> {
    if install_root.join("melonDS.exe").exists() {
        return Ok(install_root.to_path_buf());
    }

    Err(format!(
        "Impossible de localiser melonDS.exe dans {}",
        install_root.to_string_lossy()
    ))
}

fn apply_melonds_keyboard_profile(document: &mut DocumentMut, profile: &ControllerProfile) {
    for input in MELONDS_INPUTS {
        document["Instance0"]["Keyboard"][input.key] = value(i64::from(
            find_binding(profile, input.aliases).map_or(-1, melonds_keyboard_code),
        ));
    }
}

fn apply_melonds_joystick_profile(document: &mut DocumentMut, profile: &ControllerProfile) {
    document["Instance0"]["JoystickID"] = value(i64::from(melonds_joystick_id(profile)));

    for input in MELONDS_INPUTS {
        document["Instance0"]["Joystick"][input.key] = value(i64::from(
            find_binding(profile, input.aliases).map_or(-1, melonds_joystick_code),
        ));
    }
}

fn clear_melonds_section(document: &mut DocumentMut, section: &str) {
    for input in MELONDS_INPUTS {
        document["Instance0"][section][input.key] = value(-1);
    }
}

fn find_binding<'a>(profile: &'a ControllerProfile, aliases: &[&str]) -> Option<&'a str> {
    profile
        .bindings
        .iter()
        .find(|binding| {
            !binding.physical_input.trim().is_empty()
                && aliases
                    .iter()
                    .any(|alias| binding.emulated_input.eq_ignore_ascii_case(alias))
        })
        .map(|binding| binding.physical_input.trim())
}

fn is_keyboard_profile(profile: &ControllerProfile) -> bool {
    profile
        .physical_device_id
        .as_deref()
        .unwrap_or_default()
        .eq_ignore_ascii_case("keyboard")
}

fn melonds_joystick_id(profile: &ControllerProfile) -> i32 {
    profile
        .physical_device_id
        .as_deref()
        .and_then(|device_id| device_id.strip_prefix("gamepad:"))
        .and_then(|index| index.parse::<i32>().ok())
        .unwrap_or(0)
}

fn melonds_keyboard_code(input: &str) -> i32 {
    let trimmed = input.trim();
    let lower = trimmed.to_ascii_lowercase();

    match lower.as_str() {
        "escape" | "esc" => 0x01000000,
        "tab" => 0x01000001,
        "backspace" => 0x01000003,
        "enter" | "return" => 0x01000004,
        "numpad enter" => QT_KEYPAD_MODIFIER | 0x01000005,
        "insert" => 0x01000006,
        "delete" | "del" => 0x01000007,
        "home" => 0x01000010,
        "end" => 0x01000011,
        "dpad left" | "arrowleft" | "left" => 0x01000012,
        "dpad up" | "arrowup" | "up" => 0x01000013,
        "dpad right" | "arrowright" | "right" => 0x01000014,
        "dpad down" | "arrowdown" | "down" => 0x01000015,
        "pageup" | "page up" => 0x01000016,
        "pagedown" | "page down" => 0x01000017,
        "shift" => 0x01000020,
        "control" | "ctrl" => 0x01000021,
        "meta" | "windows" | "super" => 0x01000022,
        "alt" => 0x01000023,
        "space" | " " => 0x20,
        "numpad multiply" => QT_KEYPAD_MODIFIER | 0x2A,
        "numpad add" => QT_KEYPAD_MODIFIER | 0x2B,
        "numpad subtract" => QT_KEYPAD_MODIFIER | 0x2D,
        "numpad decimal" => QT_KEYPAD_MODIFIER | 0x2E,
        "numpad divide" => QT_KEYPAD_MODIFIER | 0x2F,
        other if other.starts_with("numpad ") && other.len() == 8 => {
            let digit = other.as_bytes()[7];
            if digit.is_ascii_digit() {
                QT_KEYPAD_MODIFIER | i32::from(digit)
            } else {
                -1
            }
        }
        other if other.starts_with('f') => function_key_code(other).unwrap_or(-1),
        _ => single_character_qt_code(trimmed).unwrap_or(-1),
    }
}

fn single_character_qt_code(input: &str) -> Option<i32> {
    let mut chars = input.chars();
    let character = chars.next()?;
    if chars.next().is_some() || !character.is_ascii() {
        return None;
    }

    Some(character.to_ascii_uppercase() as i32)
}

fn function_key_code(input: &str) -> Option<i32> {
    let number = input.strip_prefix('f')?.parse::<i32>().ok()?;
    if (1..=35).contains(&number) {
        Some(0x01000030 + number - 1)
    } else {
        None
    }
}

fn melonds_joystick_code(input: &str) -> i32 {
    let lower = input.trim().to_ascii_lowercase();

    match lower.as_str() {
        "a" => 0,
        "b" => 1,
        "x" => 2,
        "y" => 3,
        "lb" | "l1" => 4,
        "rb" | "r1" => 5,
        "select" | "back" | "minus" => 6,
        "start" | "plus" => 7,
        "left stick" | "thumb l" => 8,
        "right stick" | "thumb r" => 9,
        "home" | "guide" => 10,
        "dpad up" => 11,
        "dpad down" => 12,
        "dpad left" => 13,
        "dpad right" => 14,
        "left stick right" => melonds_axis_code(0, 0),
        "left stick left" => melonds_axis_code(0, 1),
        "left stick down" => melonds_axis_code(1, 0),
        "left stick up" => melonds_axis_code(1, 1),
        "right stick right" => melonds_axis_code(2, 0),
        "right stick left" => melonds_axis_code(2, 1),
        "right stick down" => melonds_axis_code(3, 0),
        "right stick up" => melonds_axis_code(3, 1),
        "left trigger" | "lt" | "l2" => melonds_axis_code(4, 2),
        "right trigger" | "rt" | "r2" => melonds_axis_code(5, 2),
        other => other
            .strip_prefix("button ")
            .and_then(|button| button.parse::<i32>().ok())
            .unwrap_or(-1),
    }
}

fn melonds_axis_code(axis: i32, axis_type: i32) -> i32 {
    MELONDS_NO_BUTTON | MELONDS_AXIS_FLAG | (axis_type << 20) | (axis << 24)
}
