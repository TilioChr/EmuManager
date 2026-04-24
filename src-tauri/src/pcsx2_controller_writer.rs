use crate::controller_profile_writer::ControllerWriteResult;
use crate::controller_profiles::ControllerProfile;
use crate::portable_paths::PortablePaths;
use std::fmt::Write as _;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const PCSX2_CONFIG_FILE: &str = "PCSX2.ini";

struct Pcsx2Button {
    key: &'static str,
    aliases: &'static [&'static str],
    fallback: &'static str,
}

const PCSX2_BUTTONS: &[Pcsx2Button] = &[
    Pcsx2Button {
        key: "Up",
        aliases: &["Croix Haut", "DPad Up"],
        fallback: "Keyboard/Up",
    },
    Pcsx2Button {
        key: "Right",
        aliases: &["Croix Droite", "DPad Right"],
        fallback: "Keyboard/Right",
    },
    Pcsx2Button {
        key: "Down",
        aliases: &["Croix Bas", "DPad Down"],
        fallback: "Keyboard/Down",
    },
    Pcsx2Button {
        key: "Left",
        aliases: &["Croix Gauche", "DPad Left"],
        fallback: "Keyboard/Left",
    },
    Pcsx2Button {
        key: "Triangle",
        aliases: &["Triangle"],
        fallback: "Keyboard/I",
    },
    Pcsx2Button {
        key: "Circle",
        aliases: &["Rond", "Circle"],
        fallback: "Keyboard/L",
    },
    Pcsx2Button {
        key: "Cross",
        aliases: &["Croix", "Cross"],
        fallback: "Keyboard/K",
    },
    Pcsx2Button {
        key: "Square",
        aliases: &["Carre", "Carré", "Square"],
        fallback: "Keyboard/J",
    },
    Pcsx2Button {
        key: "Select",
        aliases: &["Select"],
        fallback: "Keyboard/Backspace",
    },
    Pcsx2Button {
        key: "Start",
        aliases: &["Start"],
        fallback: "Keyboard/Return",
    },
    Pcsx2Button {
        key: "L1",
        aliases: &["L1", "L"],
        fallback: "Keyboard/Q",
    },
    Pcsx2Button {
        key: "L2",
        aliases: &["L2"],
        fallback: "Keyboard/1",
    },
    Pcsx2Button {
        key: "R1",
        aliases: &["R1", "R"],
        fallback: "Keyboard/E",
    },
    Pcsx2Button {
        key: "R2",
        aliases: &["R2"],
        fallback: "Keyboard/3",
    },
    Pcsx2Button {
        key: "L3",
        aliases: &["Stick Gauche Bouton", "Left Stick", "Thumb L"],
        fallback: "Keyboard/2",
    },
    Pcsx2Button {
        key: "R3",
        aliases: &["Stick Droit Bouton", "Right Stick", "Thumb R"],
        fallback: "Keyboard/4",
    },
    Pcsx2Button {
        key: "LUp",
        aliases: &["Stick Haut"],
        fallback: "Keyboard/W",
    },
    Pcsx2Button {
        key: "LRight",
        aliases: &["Stick Droite"],
        fallback: "Keyboard/D",
    },
    Pcsx2Button {
        key: "LDown",
        aliases: &["Stick Bas"],
        fallback: "Keyboard/S",
    },
    Pcsx2Button {
        key: "LLeft",
        aliases: &["Stick Gauche"],
        fallback: "Keyboard/A",
    },
    Pcsx2Button {
        key: "RUp",
        aliases: &["Stick Droit Haut"],
        fallback: "Keyboard/T",
    },
    Pcsx2Button {
        key: "RRight",
        aliases: &["Stick Droit Droite"],
        fallback: "Keyboard/H",
    },
    Pcsx2Button {
        key: "RDown",
        aliases: &["Stick Droit Bas"],
        fallback: "Keyboard/G",
    },
    Pcsx2Button {
        key: "RLeft",
        aliases: &["Stick Droit Gauche"],
        fallback: "Keyboard/F",
    },
];

pub fn apply_pcsx2_profile(
    paths: &PortablePaths,
    profile: &ControllerProfile,
) -> Result<ControllerWriteResult, String> {
    let install_root = PathBuf::from(&paths.emu).join("PCSX2");
    apply_pcsx2_profile_to_install_dir(paths, profile, &install_root)
}

pub fn apply_pcsx2_profile_to_install_dir(
    paths: &PortablePaths,
    profile: &ControllerProfile,
    install_root: &Path,
) -> Result<ControllerWriteResult, String> {
    let config_dir = install_root.join("inis");
    fs::create_dir_all(&config_dir)
        .map_err(|error| format!("Impossible de creer inis PCSX2: {}", error))?;

    let config_path = config_dir.join(PCSX2_CONFIG_FILE);
    let mut content = fs::read_to_string(&config_path).unwrap_or_else(|_| String::new());
    log_pcsx2_controller(
        paths,
        &format!(
            "pcsx2 config before write profile_id={} path={} existed={} bytes={}",
            profile.id,
            config_path.to_string_lossy(),
            config_path.exists(),
            content.len()
        ),
    );

    let debug = apply_controls_to_ini(&mut content, profile);

    fs::write(&config_path, content)
        .map_err(|error| format!("Impossible d'ecrire PCSX2.ini: {}", error))?;
    log_pcsx2_controller(
        paths,
        &format!(
            "pcsx2 config written path={} bytes={} debug={}",
            config_path.to_string_lossy(),
            fs::metadata(&config_path)
                .map(|metadata| metadata.len())
                .unwrap_or(0),
            debug
        ),
    );

    Ok(ControllerWriteResult {
        emulator_id: profile.emulator_id.clone(),
        profile_id: profile.id.clone(),
        profile_path: config_path.to_string_lossy().to_string(),
        game_ini_path: config_path.to_string_lossy().to_string(),
    })
}

fn apply_controls_to_ini(content: &mut String, profile: &ControllerProfile) -> String {
    let keyboard = is_keyboard_profile(profile);
    let existing_sdl_index = if keyboard {
        None
    } else {
        existing_pcsx2_sdl_index(content)
    };
    let sdl_index = existing_sdl_index.unwrap_or_else(|| pcsx2_sdl_index(profile));
    let mut debug = String::new();
    let _ = write!(
        debug,
        "keyboard={} sdl_index={} existing_sdl_index={:?}",
        keyboard, sdl_index, existing_sdl_index
    );

    set_ini_value(content, "InputSources", "Keyboard", "true");
    set_ini_value(content, "InputSources", "Mouse", "true");
    set_ini_value(content, "InputSources", "SDL", "true");
    set_ini_value(content, "InputSources", "DInput", "false");
    set_ini_value(content, "InputSources", "XInput", "false");
    set_ini_value(content, "InputSources", "SDLControllerEnhancedMode", "true");

    set_ini_value(content, "Pad1", "Type", "DualShock2");
    set_ini_value(content, "Pad1", "InvertL", "0");
    set_ini_value(content, "Pad1", "InvertR", "0");
    set_ini_value(content, "Pad1", "Deadzone", "0");
    set_ini_value(content, "Pad1", "AxisScale", "1.33");
    set_ini_value(content, "Pad1", "LargeMotorScale", "1");
    set_ini_value(content, "Pad1", "SmallMotorScale", "1");
    set_ini_value(content, "Pad1", "ButtonDeadzone", "0");
    set_ini_value(content, "Pad1", "PressureModifier", "0.5");

    for button in PCSX2_BUTTONS {
        let value = find_binding(profile, button.aliases)
            .map(|input| pcsx2_input_binding(profile, input, sdl_index))
            .unwrap_or_else(|| button.fallback.to_string());
        let _ = write!(debug, " {}={}", button.key, value);
        set_ini_value(content, "Pad1", button.key, &value);
    }

    if !is_keyboard_profile(profile) {
        set_ini_value(
            content,
            "Pad1",
            "Analog",
            &format!("SDL-{}/Guide", sdl_index),
        );
        set_ini_value(
            content,
            "Pad1",
            "LargeMotor",
            &format!("SDL-{}/LargeMotor", sdl_index),
        );
        set_ini_value(
            content,
            "Pad1",
            "SmallMotor",
            &format!("SDL-{}/SmallMotor", sdl_index),
        );
    }

    for pad in 2..=8 {
        set_ini_value(content, &format!("Pad{}", pad), "Type", "None");
    }

    debug
}

fn set_ini_value(content: &mut String, section: &str, key: &str, value: &str) {
    let had_final_newline = content.ends_with('\n') || content.ends_with("\r\n");
    let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
    let section_header = format!("[{}]", section);

    let section_index = lines
        .iter()
        .position(|line| line.trim().eq_ignore_ascii_case(&section_header))
        .unwrap_or_else(|| {
            if !lines.is_empty() {
                lines.push(String::new());
            }
            lines.push(section_header);
            lines.len() - 1
        });

    let next_section_index = lines
        .iter()
        .enumerate()
        .skip(section_index + 1)
        .find(|(_, line)| {
            let trimmed = line.trim();
            trimmed.starts_with('[') && trimmed.ends_with(']')
        })
        .map(|(index, _)| index)
        .unwrap_or(lines.len());

    if let Some(line) = lines
        .iter_mut()
        .take(next_section_index)
        .skip(section_index + 1)
        .find(|line| {
            line.split_once('=')
                .is_some_and(|(candidate, _)| candidate.trim() == key)
        })
    {
        *line = format!("{} = {}", key, value);
    } else {
        lines.insert(next_section_index, format!("{} = {}", key, value));
    }

    *content = lines.join("\n");
    if had_final_newline || !content.is_empty() {
        content.push('\n');
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

fn pcsx2_input_binding(profile: &ControllerProfile, input: &str, sdl_index: i32) -> String {
    if is_keyboard_profile(profile) {
        return keyboard_binding(input);
    }

    gamepad_binding(input, sdl_index)
}

fn gamepad_binding(input: &str, sdl_index: i32) -> String {
    let lower = input.trim().to_ascii_lowercase();
    let control = match lower.as_str() {
        "a" => "A",
        "b" => "B",
        "x" => "X",
        "y" => "Y",
        "lb" | "l1" => "LeftShoulder",
        "rb" | "r1" => "RightShoulder",
        "lt" | "l2" | "left trigger" => "+LeftTrigger",
        "rt" | "r2" | "right trigger" => "+RightTrigger",
        "select" | "back" | "minus" => "Back",
        "start" | "plus" => "Start",
        "home" | "guide" => "Guide",
        "left stick" | "thumb l" => "LeftStick",
        "right stick" | "thumb r" => "RightStick",
        "dpad up" => "DPadUp",
        "dpad down" => "DPadDown",
        "dpad left" => "DPadLeft",
        "dpad right" => "DPadRight",
        "left stick up" => "-LeftY",
        "left stick down" => "+LeftY",
        "left stick left" => "-LeftX",
        "left stick right" => "+LeftX",
        "right stick up" => "-RightY",
        "right stick down" => "+RightY",
        "right stick left" => "-RightX",
        "right stick right" => "+RightX",
        other => other.strip_prefix("button ").unwrap_or("A"),
    };

    format!("SDL-{}/{}", sdl_index, control)
}

fn keyboard_binding(input: &str) -> String {
    format!("Keyboard/{}", pcsx2_keyboard_name(input))
}

fn pcsx2_keyboard_name(input: &str) -> String {
    let trimmed = input.trim();
    let lower = trimmed.to_ascii_lowercase();

    match lower.as_str() {
        "escape" | "esc" => "Escape".to_string(),
        "tab" => "Tab".to_string(),
        "backspace" => "Backspace".to_string(),
        "enter" | "return" | "numpad enter" => "Return".to_string(),
        "space" | " " => "Space".to_string(),
        "shift" => "Shift".to_string(),
        "control" | "ctrl" => "Control".to_string(),
        "alt" => "Alt".to_string(),
        "delete" | "del" => "Delete".to_string(),
        "dpad up" | "arrowup" | "up" => "Up".to_string(),
        "dpad down" | "arrowdown" | "down" => "Down".to_string(),
        "dpad left" | "arrowleft" | "left" => "Left".to_string(),
        "dpad right" | "arrowright" | "right" => "Right".to_string(),
        "numpad multiply" | "*" => "Multiply".to_string(),
        "numpad add" => "Keypad+Plus".to_string(),
        "numpad subtract" => "Keypad+Minus".to_string(),
        "numpad decimal" => "Keypad+Period".to_string(),
        "numpad divide" => "Divide".to_string(),
        other if other.starts_with("numpad ") && other.len() == 8 => {
            format!("Keypad+{}", other.chars().last().unwrap_or('0'))
        }
        other if other.starts_with('f') => other.to_ascii_uppercase(),
        _ => {
            if trimmed.len() == 1 {
                trimmed.to_ascii_uppercase()
            } else {
                trimmed.to_string()
            }
        }
    }
}

fn pcsx2_sdl_index(profile: &ControllerProfile) -> i32 {
    profile
        .physical_device_id
        .as_deref()
        .and_then(|device_id| device_id.strip_prefix("gamepad:"))
        .and_then(|index| index.parse::<i32>().ok())
        .unwrap_or(0)
}

fn existing_pcsx2_sdl_index(content: &str) -> Option<i32> {
    let mut in_pad1 = false;
    let mut fallback = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_pad1 = trimmed.eq_ignore_ascii_case("[Pad1]");
            continue;
        }

        if let Some(index) = parse_pcsx2_sdl_index(trimmed) {
            if in_pad1 {
                return Some(index);
            }
            fallback.get_or_insert(index);
        }
    }

    fallback
}

fn parse_pcsx2_sdl_index(line: &str) -> Option<i32> {
    let value = line
        .split_once('=')
        .map(|(_, value)| value.trim())
        .unwrap_or_else(|| line.trim());
    let start = value.find("SDL-")?;
    let rest = &value[start + 4..];
    let digit_len = rest.bytes().take_while(u8::is_ascii_digit).count();

    if digit_len == 0 || !rest[digit_len..].starts_with('/') {
        return None;
    }

    rest[..digit_len].parse().ok()
}

fn log_pcsx2_controller(paths: &PortablePaths, message: &str) {
    let logs_dir = Path::new(&paths.data).join("Logs");
    if fs::create_dir_all(&logs_dir).is_err() {
        return;
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);

    let log_path = logs_dir.join("controller-mapping.log");
    let line = format!("[{}] [pcsx2] {}\n", timestamp, message);

    if let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        let _ = file.write_all(line.as_bytes());
    }
}
