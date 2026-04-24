use crate::controller_profile_writer::ControllerWriteResult;
use crate::controller_profiles::ControllerProfile;
use crate::portable_paths::PortablePaths;
use std::fmt::Write as _;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const EDEN_CONFIG_FILE: &str = "qt-config.ini";
const CONTROLS_SECTION: &str = "Controls";

struct EdenButton {
    key: &'static str,
    aliases: &'static [&'static str],
}

struct EdenAnalog {
    key: &'static str,
    up_aliases: &'static [&'static str],
    down_aliases: &'static [&'static str],
    left_aliases: &'static [&'static str],
    right_aliases: &'static [&'static str],
    native_axis_x: i32,
    native_axis_y: i32,
}

#[derive(Debug, Clone)]
struct EdenSdlTarget {
    guid: String,
    port: i32,
    source: String,
}

const EDEN_BUTTONS: &[EdenButton] = &[
    EdenButton {
        key: "button_a",
        aliases: &["Bouton A", "A"],
    },
    EdenButton {
        key: "button_b",
        aliases: &["Bouton B", "B"],
    },
    EdenButton {
        key: "button_x",
        aliases: &["Bouton X", "X"],
    },
    EdenButton {
        key: "button_y",
        aliases: &["Bouton Y", "Y"],
    },
    EdenButton {
        key: "button_l",
        aliases: &["L", "L1", "LB"],
    },
    EdenButton {
        key: "button_r",
        aliases: &["R", "R1", "RB"],
    },
    EdenButton {
        key: "button_zl",
        aliases: &["ZL", "L2"],
    },
    EdenButton {
        key: "button_zr",
        aliases: &["ZR", "R2"],
    },
    EdenButton {
        key: "button_plus",
        aliases: &["Plus", "Start"],
    },
    EdenButton {
        key: "button_minus",
        aliases: &["Minus", "Select"],
    },
    EdenButton {
        key: "button_home",
        aliases: &["Home", "Guide"],
    },
    EdenButton {
        key: "button_screenshot",
        aliases: &["Capture", "Screenshot"],
    },
    EdenButton {
        key: "button_lstick",
        aliases: &["Stick Gauche Bouton", "Left Stick", "Thumb L"],
    },
    EdenButton {
        key: "button_rstick",
        aliases: &["Stick Droit Bouton", "Right Stick", "Thumb R"],
    },
    EdenButton {
        key: "button_dup",
        aliases: &["Croix Haut", "DPad Up"],
    },
    EdenButton {
        key: "button_ddown",
        aliases: &["Croix Bas", "DPad Down"],
    },
    EdenButton {
        key: "button_dleft",
        aliases: &["Croix Gauche", "DPad Left"],
    },
    EdenButton {
        key: "button_dright",
        aliases: &["Croix Droite", "DPad Right"],
    },
];

const EDEN_ANALOGS: &[EdenAnalog] = &[
    EdenAnalog {
        key: "lstick",
        up_aliases: &["Stick Haut"],
        down_aliases: &["Stick Bas"],
        left_aliases: &["Stick Gauche"],
        right_aliases: &["Stick Droite"],
        native_axis_x: 0,
        native_axis_y: 1,
    },
    EdenAnalog {
        key: "rstick",
        up_aliases: &["Stick Droit Haut"],
        down_aliases: &["Stick Droit Bas"],
        left_aliases: &["Stick Droit Gauche"],
        right_aliases: &["Stick Droit Droite"],
        native_axis_x: 2,
        native_axis_y: 3,
    },
];

pub fn apply_eden_profile(
    paths: &PortablePaths,
    profile: &ControllerProfile,
) -> Result<ControllerWriteResult, String> {
    let install_root = PathBuf::from(&paths.emu).join("Eden");
    let user_dir = install_root.join("user");
    log_eden_controller(
        paths,
        &format!(
            "apply_eden_profile profile_id={} physical_id={:?} physical_label={:?} user_dir={}",
            profile.id,
            profile.physical_device_id,
            profile.physical_device_label,
            user_dir.to_string_lossy()
        ),
    );

    apply_eden_profile_to_user_dir(paths, profile, &user_dir)
}

pub fn apply_eden_profile_to_user_dir(
    paths: &PortablePaths,
    profile: &ControllerProfile,
    user_dir: &Path,
) -> Result<ControllerWriteResult, String> {
    let config_dir = user_dir.join("config");
    fs::create_dir_all(&config_dir)
        .map_err(|error| format!("Impossible de creer user/config Eden: {}", error))?;

    let config_path = config_dir.join(EDEN_CONFIG_FILE);
    let mut content = fs::read_to_string(&config_path).unwrap_or_else(|_| String::new());
    log_eden_controller(
        paths,
        &format!(
            "eden config before write path={} existed={} bytes={}",
            config_path.to_string_lossy(),
            config_path.exists(),
            content.len()
        ),
    );

    let debug = apply_controls_to_ini(&mut content, profile);

    fs::write(&config_path, content)
        .map_err(|error| format!("Impossible d'ecrire qt-config.ini Eden: {}", error))?;
    log_eden_controller(
        paths,
        &format!(
            "eden config written path={} bytes={} debug={}",
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
    let sdl_target = resolve_eden_sdl_target(profile, content);
    let mut debug = String::new();
    let _ = write!(
        debug,
        "keyboard={} sdl_target={}",
        is_keyboard_profile(profile),
        sdl_target
            .as_ref()
            .map(|target| format!(
                "guid:{} port:{} source:{}",
                target.guid, target.port, target.source
            ))
            .unwrap_or_else(|| "none".to_string())
    );

    set_controls_value(content, "keyboard_enabled\\default", "false");
    set_controls_value(
        content,
        "keyboard_enabled",
        if is_keyboard_profile(profile) {
            "true"
        } else {
            "false"
        },
    );
    set_controls_value(content, "controller_navigation\\default", "false");
    set_controls_value(content, "controller_navigation", "true");
    set_controls_value(content, "enable_procon_driver\\default", "false");
    set_controls_value(content, "enable_procon_driver", "false");
    set_controls_value(content, "enable_joycon_driver\\default", "false");
    set_controls_value(content, "enable_joycon_driver", "true");
    set_controls_value(content, "vibration_enabled\\default", "false");
    set_controls_value(content, "vibration_enabled", "true");
    set_controls_value(content, "motion_enabled\\default", "false");
    set_controls_value(content, "motion_enabled", "true");
    set_controls_value(content, "player_0_type\\default", "false");
    set_controls_value(
        content,
        "player_0_type",
        &eden_controller_type(profile).to_string(),
    );
    set_controls_value(content, "player_0_profile_name\\default", "false");
    set_controls_value(content, "player_0_profile_name", "EmuManager");
    set_controls_value(content, "player_0_connected\\default", "false");
    set_controls_value(content, "player_0_connected", "true");
    set_controls_value(content, "player_0_vibration_enabled\\default", "false");
    set_controls_value(content, "player_0_vibration_enabled", "true");
    set_controls_value(content, "player_0_vibration_strength\\default", "false");
    set_controls_value(content, "player_0_vibration_strength", "100");

    for player in 1..=7 {
        set_controls_value(
            content,
            &format!("player_{}_connected\\default", player),
            "false",
        );
        set_controls_value(content, &format!("player_{}_connected", player), "false");
    }

    for button in EDEN_BUTTONS {
        let value = find_binding(profile, button.aliases)
            .map(|input| eden_button_param(profile, input, sdl_target.as_ref()))
            .unwrap_or_else(empty_button_param);
        let _ = write!(debug, " {}={}", button.key, compact_debug_value(&value));
        set_controls_value(
            content,
            &format!("player_0_{}\\default", button.key),
            "false",
        );
        set_controls_value(
            content,
            &format!("player_0_{}", button.key),
            &quote_ini(&value),
        );
    }

    for analog in EDEN_ANALOGS {
        let value = eden_analog_param(profile, analog, sdl_target.as_ref())
            .unwrap_or_else(empty_analog_param);
        let _ = write!(debug, " {}={}", analog.key, compact_debug_value(&value));
        set_controls_value(
            content,
            &format!("player_0_{}\\default", analog.key),
            "false",
        );
        set_controls_value(
            content,
            &format!("player_0_{}", analog.key),
            &quote_ini(&value),
        );
    }

    let motion = sdl_target
        .as_ref()
        .map(|target| format!("engine:sdl,guid:{},port:{}", target.guid, target.port))
        .unwrap_or_else(empty_button_param);
    set_controls_value(content, "player_0_motionleft\\default", "false");
    set_controls_value(content, "player_0_motionleft", &quote_ini(&motion));
    set_controls_value(content, "player_0_motionright\\default", "false");
    set_controls_value(content, "player_0_motionright", &quote_ini(&motion));

    debug
}

fn eden_controller_type(profile: &ControllerProfile) -> i32 {
    match profile.emulated_controller_id.as_deref() {
        Some("joycon_pair") => 1,
        _ => 0,
    }
}

fn set_controls_value(content: &mut String, key: &str, value: &str) {
    let had_final_newline = content.ends_with('\n') || content.ends_with("\r\n");
    let mut lines: Vec<String> = content.lines().map(str::to_string).collect();

    let section_index = lines
        .iter()
        .position(|line| line.trim().eq_ignore_ascii_case("[Controls]"))
        .unwrap_or_else(|| {
            if !lines.is_empty() {
                lines.push(String::new());
            }
            lines.push(format!("[{}]", CONTROLS_SECTION));
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
                .is_some_and(|(candidate, _)| candidate == key)
        })
    {
        *line = format!("{}={}", key, value);
    } else {
        lines.insert(next_section_index, format!("{}={}", key, value));
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

fn eden_button_param(
    profile: &ControllerProfile,
    input: &str,
    sdl_target: Option<&EdenSdlTarget>,
) -> String {
    if is_keyboard_profile(profile) {
        return keyboard_param(input);
    }

    gamepad_button_param(input, sdl_target)
}

fn eden_analog_param(
    profile: &ControllerProfile,
    analog: &EdenAnalog,
    sdl_target: Option<&EdenSdlTarget>,
) -> Option<String> {
    if let Some(target) = sdl_target
        .filter(|_| !is_keyboard_profile(profile) && analog_matches_native_stick(profile, analog))
    {
        return Some(gamepad_analog_param(
            target,
            analog.native_axis_x,
            analog.native_axis_y,
        ));
    }

    let up = find_binding(profile, analog.up_aliases)
        .map(|input| eden_button_param(profile, input, sdl_target));
    let down = find_binding(profile, analog.down_aliases)
        .map(|input| eden_button_param(profile, input, sdl_target));
    let left = find_binding(profile, analog.left_aliases)
        .map(|input| eden_button_param(profile, input, sdl_target));
    let right = find_binding(profile, analog.right_aliases)
        .map(|input| eden_button_param(profile, input, sdl_target));

    if up.is_none() && down.is_none() && left.is_none() && right.is_none() {
        return None;
    }

    let mut params = vec!["engine:analog_from_button".to_string()];
    if let Some(value) = up {
        params.push(format!("up:{}", escape_param_value(&value)));
    }
    if let Some(value) = down {
        params.push(format!("down:{}", escape_param_value(&value)));
    }
    if let Some(value) = left {
        params.push(format!("left:{}", escape_param_value(&value)));
    }
    if let Some(value) = right {
        params.push(format!("right:{}", escape_param_value(&value)));
    }
    params.push(format!(
        "modifier:{}",
        escape_param_value(&empty_button_param())
    ));
    params.push("modifier_scale:0.500000".to_string());

    Some(params.join(","))
}

fn analog_matches_native_stick(profile: &ControllerProfile, analog: &EdenAnalog) -> bool {
    let expected = if analog.key == "lstick" {
        [
            (analog.up_aliases, "left stick up"),
            (analog.down_aliases, "left stick down"),
            (analog.left_aliases, "left stick left"),
            (analog.right_aliases, "left stick right"),
        ]
    } else {
        [
            (analog.up_aliases, "right stick up"),
            (analog.down_aliases, "right stick down"),
            (analog.left_aliases, "right stick left"),
            (analog.right_aliases, "right stick right"),
        ]
    };

    expected.iter().all(|(aliases, expected_input)| {
        find_binding(profile, aliases)
            .map(|input| input.eq_ignore_ascii_case(expected_input))
            .unwrap_or(false)
    })
}

fn keyboard_param(input: &str) -> String {
    format!(
        "engine:keyboard,code:{},toggle:0",
        eden_keyboard_code(input)
    )
}

fn gamepad_button_param(input: &str, sdl_target: Option<&EdenSdlTarget>) -> String {
    let fallback = EdenSdlTarget {
        guid: "0".to_string(),
        port: 0,
        source: "missing-sdl-target".to_string(),
    };
    let target = sdl_target.unwrap_or(&fallback);
    let lower = input.trim().to_ascii_lowercase();

    match lower.as_str() {
        "left trigger" | "lt" | "l2" => gamepad_axis_button_param(target, 4, "+", "0.5"),
        "right trigger" | "rt" | "r2" => gamepad_axis_button_param(target, 5, "+", "0.5"),
        "left stick up" => gamepad_axis_button_param(target, 1, "-", "-0.5"),
        "left stick down" => gamepad_axis_button_param(target, 1, "+", "0.5"),
        "left stick left" => gamepad_axis_button_param(target, 0, "-", "-0.5"),
        "left stick right" => gamepad_axis_button_param(target, 0, "+", "0.5"),
        "right stick up" => gamepad_axis_button_param(target, 3, "-", "-0.5"),
        "right stick down" => gamepad_axis_button_param(target, 3, "+", "0.5"),
        "right stick left" => gamepad_axis_button_param(target, 2, "-", "-0.5"),
        "right stick right" => gamepad_axis_button_param(target, 2, "+", "0.5"),
        other => format!(
            "engine:sdl,port:{},guid:{},button:{}",
            target.port,
            target.guid,
            gamepad_button_index(other)
        ),
    }
}

fn gamepad_axis_button_param(
    target: &EdenSdlTarget,
    axis: i32,
    direction: &str,
    threshold: &str,
) -> String {
    format!(
        "engine:sdl,port:{},guid:{},axis:{},direction:{},threshold:{}",
        target.port, target.guid, axis, direction, threshold
    )
}

fn gamepad_analog_param(target: &EdenSdlTarget, axis_x: i32, axis_y: i32) -> String {
    format!(
        "engine:sdl,port:{},guid:{},axis_x:{},axis_y:{},invert_x:+,invert_y:+",
        target.port, target.guid, axis_x, axis_y
    )
}

fn gamepad_button_index(input: &str) -> i32 {
    match input {
        "a" => 0,
        "b" => 1,
        "x" => 2,
        "y" => 3,
        "lb" | "l1" => 4,
        "rb" | "r1" => 5,
        "select" | "back" | "minus" => 6,
        "start" | "plus" => 7,
        "home" | "guide" => 8,
        "left stick" | "thumb l" => 9,
        "right stick" | "thumb r" => 10,
        "dpad up" => 11,
        "dpad down" => 12,
        "dpad left" => 13,
        "dpad right" => 14,
        other => other
            .strip_prefix("button ")
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(0),
    }
}

fn resolve_eden_sdl_target(profile: &ControllerProfile, content: &str) -> Option<EdenSdlTarget> {
    if is_keyboard_profile(profile) {
        return None;
    }

    read_existing_eden_sdl_target(profile, content).or_else(|| {
        eden_sdl_guid(profile).map(|guid| EdenSdlTarget {
            guid,
            port: 0,
            source: "fallback-from-device-label".to_string(),
        })
    })
}

fn read_existing_eden_sdl_target(
    profile: &ControllerProfile,
    content: &str,
) -> Option<EdenSdlTarget> {
    let generated_guid = generated_vendor_product_guid(profile);
    let mut best: Option<(i32, EdenSdlTarget)> = None;

    for line in content.lines().filter(|line| line.contains("engine:sdl")) {
        let Some(guid) = extract_param_value(line, "guid:") else {
            continue;
        };
        if !looks_like_guid(&guid) || !guid_matches_profile(&guid, profile) {
            continue;
        }

        let port = extract_param_value(line, "port:")
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(0);
        let is_generated_guid = generated_guid
            .as_ref()
            .map(|generated| generated.eq_ignore_ascii_case(&guid))
            .unwrap_or(false);
        let mut score = if is_generated_guid { 1 } else { 3 };
        if line.contains("player_0_") {
            score += 1;
        }

        if best
            .as_ref()
            .map(|(best_score, _)| score > *best_score)
            .unwrap_or(true)
        {
            best = Some((
                score,
                EdenSdlTarget {
                    guid,
                    port,
                    source: "existing-qt-config".to_string(),
                },
            ));
        }
    }

    best.map(|(_, target)| target)
}

fn eden_sdl_guid(profile: &ControllerProfile) -> Option<String> {
    let label = profile.physical_device_label.to_ascii_lowercase();

    if label.contains("vendor: 054c product: 09cc") || is_dualshock_4_label_without_product(&label)
    {
        return Some("03008fe54c050000cc09000000006800".to_string());
    }

    if label.contains("vendor: 054c product: 05c4") {
        return Some("03008fe54c050000c405000000006800".to_string());
    }

    if label.contains("xinput") || label.contains("xbox 360") {
        return Some("030000005e0400008e02000000000000".to_string());
    }

    generated_vendor_product_guid(profile)
}

fn generated_vendor_product_guid(profile: &ControllerProfile) -> Option<String> {
    let label = profile.physical_device_label.to_ascii_lowercase();

    if let (Some(vendor), Some(product)) = (
        extract_hex_field(&label, "vendor:"),
        extract_hex_field(&label, "product:"),
    ) {
        return Some(format!(
            "03000000{}0000{}000000000000",
            le_hex_word(&vendor),
            le_hex_word(&product)
        ));
    }

    None
}

fn is_dualshock_4_label_without_product(lower_label: &str) -> bool {
    lower_label.contains("dualshock")
        || lower_label.contains("dual shock")
        || lower_label.contains("ps4 controller")
}

fn guid_matches_profile(guid: &str, profile: &ControllerProfile) -> bool {
    let label = profile.physical_device_label.to_ascii_lowercase();

    let (Some(vendor), Some(product)) = (
        extract_hex_field(&label, "vendor:"),
        extract_hex_field(&label, "product:"),
    ) else {
        return true;
    };

    guid.get(8..12)
        .is_some_and(|value| value.eq_ignore_ascii_case(&le_hex_word(&vendor)))
        && guid
            .get(16..20)
            .is_some_and(|value| value.eq_ignore_ascii_case(&le_hex_word(&product)))
}

fn extract_param_value(line: &str, marker: &str) -> Option<String> {
    let start = line.find(marker)? + marker.len();
    let value: String = line[start..]
        .chars()
        .take_while(|character| {
            !matches!(character, ',' | '"' | '\'' | '$' | '\r' | '\n' | ' ' | '\t')
        })
        .collect();

    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn looks_like_guid(value: &str) -> bool {
    value.len() == 32 && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn extract_hex_field(label: &str, marker: &str) -> Option<String> {
    let start = label.find(marker)? + marker.len();
    let value: String = label[start..]
        .chars()
        .skip_while(|character| character.is_whitespace())
        .take_while(|character| character.is_ascii_hexdigit())
        .take(4)
        .collect();

    if value.len() == 4 {
        Some(value)
    } else {
        None
    }
}

fn le_hex_word(value: &str) -> String {
    format!("{}{}", &value[2..4], &value[0..2])
}

fn eden_keyboard_code(input: &str) -> i32 {
    let trimmed = input.trim();
    let lower = trimmed.to_ascii_lowercase();

    match lower.as_str() {
        "escape" | "esc" => 0x01000000,
        "tab" => 0x01000001,
        "backspace" => 0x01000003,
        "enter" | "return" => 0x01000004,
        "numpad enter" => 0x20000000 | 0x01000005,
        "insert" => 0x01000006,
        "delete" | "del" => 0x01000007,
        "home" => 0x01000010,
        "end" => 0x01000011,
        "dpad left" | "arrowleft" | "left" => 0x01000012,
        "dpad up" | "arrowup" | "up" => 0x01000013,
        "dpad right" | "arrowright" | "right" => 0x01000014,
        "dpad down" | "arrowdown" | "down" => 0x01000015,
        "shift" => 0x01000020,
        "control" | "ctrl" => 0x01000021,
        "alt" => 0x01000023,
        "space" | " " => 0x20,
        "numpad multiply" => 0x20000000 | 0x2A,
        "numpad add" => 0x20000000 | 0x2B,
        "numpad subtract" => 0x20000000 | 0x2D,
        "numpad decimal" => 0x20000000 | 0x2E,
        "numpad divide" => 0x20000000 | 0x2F,
        other if other.starts_with("numpad ") && other.len() == 8 => {
            let digit = other.as_bytes()[7];
            if digit.is_ascii_digit() {
                0x20000000 | i32::from(digit)
            } else {
                0
            }
        }
        other if other.starts_with('f') => function_key_code(other).unwrap_or(0),
        _ => single_character_qt_code(trimmed).unwrap_or(0),
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

fn escape_param_value(input: &str) -> String {
    input
        .replace('$', "$2")
        .replace(',', "$1")
        .replace(':', "$0")
}

fn quote_ini(value: &str) -> String {
    if value.contains(',') {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

fn empty_button_param() -> String {
    "engine:keyboard,code:0,toggle:0".to_string()
}

fn empty_analog_param() -> String {
    let empty = escape_param_value(&empty_button_param());
    format!(
        "engine:analog_from_button,up:{},down:{},left:{},right:{},modifier:{},modifier_scale:0.500000",
        empty, empty, empty, empty, empty
    )
}

fn compact_debug_value(value: &str) -> String {
    const MAX_LEN: usize = 120;
    if value.len() <= MAX_LEN {
        return value.to_string();
    }

    format!("{}...", &value[..MAX_LEN])
}

fn log_eden_controller(paths: &PortablePaths, message: &str) {
    let logs_dir = Path::new(&paths.data).join("Logs");
    if fs::create_dir_all(&logs_dir).is_err() {
        return;
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);

    let log_path = logs_dir.join("controller-mapping.log");
    let line = format!("[{}] [eden] {}\n", timestamp, message);

    if let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        let _ = file.write_all(line.as_bytes());
    }
}
