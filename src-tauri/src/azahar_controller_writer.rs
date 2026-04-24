use crate::controller_profiles::ControllerProfile;
use crate::dolphin_controller_writer::ControllerWriteResult;
use crate::portable_paths::PortablePaths;
use std::fmt::Write as _;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const AZAHAR_CONFIG_FILE: &str = "qt-config.ini";
const CONTROLS_SECTION: &str = "Controls";

struct AzaharButton {
    key: &'static str,
    aliases: &'static [&'static str],
}

struct AzaharAnalog {
    key: &'static str,
    up_aliases: &'static [&'static str],
    down_aliases: &'static [&'static str],
    left_aliases: &'static [&'static str],
    right_aliases: &'static [&'static str],
    native_axis_x: i32,
    native_axis_y: i32,
}

#[derive(Debug, Clone)]
struct AzaharSdlTarget {
    guid: String,
    port: i32,
    source: String,
}

const AZAHAR_BUTTONS: &[AzaharButton] = &[
    AzaharButton {
        key: "button_a",
        aliases: &["Bouton A", "A"],
    },
    AzaharButton {
        key: "button_b",
        aliases: &["Bouton B", "B"],
    },
    AzaharButton {
        key: "button_x",
        aliases: &["Bouton X", "X"],
    },
    AzaharButton {
        key: "button_y",
        aliases: &["Bouton Y", "Y"],
    },
    AzaharButton {
        key: "button_l",
        aliases: &["L", "L1", "Gachette L"],
    },
    AzaharButton {
        key: "button_r",
        aliases: &["R", "R1", "Gachette R"],
    },
    AzaharButton {
        key: "button_zl",
        aliases: &["ZL", "L2"],
    },
    AzaharButton {
        key: "button_zr",
        aliases: &["ZR", "R2"],
    },
    AzaharButton {
        key: "button_start",
        aliases: &["Start", "Plus"],
    },
    AzaharButton {
        key: "button_select",
        aliases: &["Select", "Minus"],
    },
    AzaharButton {
        key: "button_home",
        aliases: &["Home", "Guide"],
    },
    AzaharButton {
        key: "button_up",
        aliases: &["Croix Haut", "DPad Up"],
    },
    AzaharButton {
        key: "button_down",
        aliases: &["Croix Bas", "DPad Down"],
    },
    AzaharButton {
        key: "button_left",
        aliases: &["Croix Gauche", "DPad Left"],
    },
    AzaharButton {
        key: "button_right",
        aliases: &["Croix Droite", "DPad Right"],
    },
];

const AZAHAR_ANALOGS: &[AzaharAnalog] = &[
    AzaharAnalog {
        key: "circle_pad",
        up_aliases: &["Stick Haut", "Circle Pad Haut"],
        down_aliases: &["Stick Bas", "Circle Pad Bas"],
        left_aliases: &["Stick Gauche", "Circle Pad Gauche"],
        right_aliases: &["Stick Droite", "Circle Pad Droite"],
        native_axis_x: 0,
        native_axis_y: 1,
    },
    AzaharAnalog {
        key: "c_stick",
        up_aliases: &["C-Stick Haut", "Stick Droit Haut"],
        down_aliases: &["C-Stick Bas", "Stick Droit Bas"],
        left_aliases: &["C-Stick Gauche", "Stick Droit Gauche"],
        right_aliases: &["C-Stick Droite", "Stick Droit Droite"],
        native_axis_x: 2,
        native_axis_y: 3,
    },
];

pub fn apply_azahar_profile(
    paths: &PortablePaths,
    profile: &ControllerProfile,
) -> Result<ControllerWriteResult, String> {
    let install_root = PathBuf::from(&paths.emu).join("Azahar");
    log_azahar_controller(
        paths,
        &format!(
            "apply_azahar_profile profile_id={} physical_id={:?} physical_label={:?} install_root={}",
            profile.id,
            profile.physical_device_id,
            profile.physical_device_label,
            install_root.to_string_lossy()
        ),
    );

    let executable_dir = locate_azahar_executable_dir(&install_root)?;
    let user_dir = executable_dir.join("user");
    log_azahar_controller(
        paths,
        &format!(
            "azahar executable_dir={} user_dir={}",
            executable_dir.to_string_lossy(),
            user_dir.to_string_lossy()
        ),
    );

    apply_azahar_profile_to_user_dir(paths, profile, &user_dir)
}

pub fn apply_azahar_profile_to_user_dir(
    paths: &PortablePaths,
    profile: &ControllerProfile,
    user_dir: &Path,
) -> Result<ControllerWriteResult, String> {
    let config_dir = user_dir.join("config");
    log_azahar_controller(
        paths,
        &format!(
            "apply_azahar_profile_to_user_dir profile_id={} user_dir={} config_dir={}",
            profile.id,
            user_dir.to_string_lossy(),
            config_dir.to_string_lossy()
        ),
    );

    fs::create_dir_all(&config_dir)
        .map_err(|error| format!("Impossible de creer user/config Azahar: {}", error))?;

    let config_path = config_dir.join(AZAHAR_CONFIG_FILE);
    let mut content = fs::read_to_string(&config_path).unwrap_or_else(|_| String::new());
    log_azahar_controller(
        paths,
        &format!(
            "azahar config before write path={} existed={} bytes={}",
            config_path.to_string_lossy(),
            config_path.exists(),
            content.len()
        ),
    );

    let debug = apply_controls_to_ini(&mut content, profile);

    fs::write(&config_path, content)
        .map_err(|error| format!("Impossible d'ecrire qt-config.ini Azahar: {}", error))?;
    log_azahar_controller(
        paths,
        &format!(
            "azahar config written path={} bytes={} debug={}",
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

fn locate_azahar_executable_dir(install_root: &Path) -> Result<PathBuf, String> {
    let direct_exe = install_root.join("azahar.exe");
    if direct_exe.exists() {
        return Ok(install_root.to_path_buf());
    }

    let entries = fs::read_dir(install_root).map_err(|error| {
        format!(
            "Impossible de lire le dossier Azahar {}: {}",
            install_root.to_string_lossy(),
            error
        )
    })?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.join("azahar.exe").exists() {
            return Ok(path);
        }
    }

    Err(format!(
        "Impossible de localiser azahar.exe dans {}",
        install_root.to_string_lossy()
    ))
}

fn apply_controls_to_ini(content: &mut String, profile: &ControllerProfile) -> String {
    let sdl_target = resolve_azahar_sdl_target(profile, content);
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

    set_controls_value(content, "use_artic_base_controller\\default", "false");
    set_controls_value(content, "use_artic_base_controller", "false");
    set_controls_value(content, "profile\\default", "false");
    set_controls_value(content, "profile", "0");
    set_controls_value(content, "profiles\\size", "1");
    set_controls_value(content, "profiles\\1\\name\\default", "false");
    set_controls_value(content, "profiles\\1\\name", "EmuManager");

    for button in AZAHAR_BUTTONS {
        let value = find_binding(profile, button.aliases)
            .map(|input| azahar_button_param(profile, input, sdl_target.as_ref()))
            .unwrap_or_else(empty_param);
        let _ = write!(debug, " {}={}", button.key, compact_debug_value(&value));
        set_controls_value(
            content,
            &format!("profiles\\1\\{}\\default", button.key),
            "false",
        );
        set_controls_value(
            content,
            &format!("profiles\\1\\{}", button.key),
            &quote_ini(&value),
        );
    }

    for analog in AZAHAR_ANALOGS {
        let value =
            azahar_analog_param(profile, analog, sdl_target.as_ref()).unwrap_or_else(empty_param);
        let _ = write!(debug, " {}={}", analog.key, compact_debug_value(&value));
        set_controls_value(
            content,
            &format!("profiles\\1\\{}\\default", analog.key),
            "false",
        );
        set_controls_value(
            content,
            &format!("profiles\\1\\{}", analog.key),
            &quote_ini(&value),
        );
    }

    set_controls_value(content, "profiles\\1\\motion_device\\default", "false");
    if let Some(target) = sdl_target.as_ref() {
        let motion_device = format!("engine:sdl,guid:{},port:{}", target.guid, target.port);
        set_controls_value(
            content,
            "profiles\\1\\motion_device",
            &quote_ini(&motion_device),
        );
    } else {
        set_controls_value(
            content,
            "profiles\\1\\motion_device",
            &quote_ini("engine:motion_emu,update_period:100,sensitivity:0.01,tilt_clamp:90.0"),
        );
    }
    set_controls_value(content, "profiles\\1\\touch_device\\default", "true");
    set_controls_value(content, "profiles\\1\\touch_device", "engine:emu_window");
    set_controls_value(content, "profiles\\1\\use_touchpad\\default", "true");
    set_controls_value(content, "profiles\\1\\use_touchpad", "false");
    set_controls_value(
        content,
        "profiles\\1\\controller_touch_device\\default",
        "false",
    );
    set_controls_value(
        content,
        "profiles\\1\\controller_touch_device",
        &empty_param(),
    );
    set_controls_value(
        content,
        "profiles\\1\\use_touch_from_button\\default",
        "true",
    );
    set_controls_value(content, "profiles\\1\\use_touch_from_button", "false");
    set_controls_value(
        content,
        "profiles\\1\\touch_from_button_map\\default",
        "true",
    );
    set_controls_value(content, "profiles\\1\\touch_from_button_map", "0");
    set_controls_value(content, "profiles\\1\\udp_input_address\\default", "true");
    set_controls_value(content, "profiles\\1\\udp_input_address", "127.0.0.1");
    set_controls_value(content, "profiles\\1\\udp_input_port\\default", "true");
    set_controls_value(content, "profiles\\1\\udp_input_port", "26760");
    set_controls_value(content, "profiles\\1\\udp_pad_index\\default", "true");
    set_controls_value(content, "profiles\\1\\udp_pad_index", "0");

    debug
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

fn azahar_button_param(
    profile: &ControllerProfile,
    input: &str,
    sdl_target: Option<&AzaharSdlTarget>,
) -> String {
    if is_keyboard_profile(profile) {
        return keyboard_param(input);
    }

    gamepad_button_param(input, sdl_target)
}

fn azahar_analog_param(
    profile: &ControllerProfile,
    analog: &AzaharAnalog,
    sdl_target: Option<&AzaharSdlTarget>,
) -> Option<String> {
    if !is_keyboard_profile(profile)
        && analog_matches_native_stick(profile, analog)
        && sdl_target.is_some()
    {
        return Some(gamepad_analog_param(
            sdl_target.unwrap(),
            analog.native_axis_x,
            analog.native_axis_y,
        ));
    }

    let up = find_binding(profile, analog.up_aliases)
        .map(|input| azahar_button_param(profile, input, sdl_target));
    let down = find_binding(profile, analog.down_aliases)
        .map(|input| azahar_button_param(profile, input, sdl_target));
    let left = find_binding(profile, analog.left_aliases)
        .map(|input| azahar_button_param(profile, input, sdl_target));
    let right = find_binding(profile, analog.right_aliases)
        .map(|input| azahar_button_param(profile, input, sdl_target));

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
    params.push("modifier_scale:0.500000".to_string());

    Some(params.join(","))
}

fn analog_matches_native_stick(profile: &ControllerProfile, analog: &AzaharAnalog) -> bool {
    let expected = if analog.key == "circle_pad" {
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
    format!("code:{},engine:keyboard", azahar_keyboard_code(input))
}

fn gamepad_button_param(input: &str, sdl_target: Option<&AzaharSdlTarget>) -> String {
    let fallback = AzaharSdlTarget {
        guid: "0".to_string(),
        port: 0,
        source: "missing-sdl-target".to_string(),
    };
    let target = sdl_target.unwrap_or(&fallback);
    let lower = input.trim().to_ascii_lowercase();

    match lower.as_str() {
        "left trigger" | "lt" | "l2" => {
            format!(
                "axis:4,direction:+,engine:sdl,guid:{},port:{},threshold:0.5",
                target.guid, target.port
            )
        }
        "right trigger" | "rt" | "r2" => {
            format!(
                "axis:5,direction:+,engine:sdl,guid:{},port:{},threshold:0.5",
                target.guid, target.port
            )
        }
        "left stick up" => gamepad_axis_button_param(target, 1, "-", "-0.5"),
        "left stick down" => gamepad_axis_button_param(target, 1, "+", "0.5"),
        "left stick left" => gamepad_axis_button_param(target, 0, "-", "-0.5"),
        "left stick right" => gamepad_axis_button_param(target, 0, "+", "0.5"),
        "right stick up" => gamepad_axis_button_param(target, 3, "-", "-0.5"),
        "right stick down" => gamepad_axis_button_param(target, 3, "+", "0.5"),
        "right stick left" => gamepad_axis_button_param(target, 2, "-", "-0.5"),
        "right stick right" => gamepad_axis_button_param(target, 2, "+", "0.5"),
        other => {
            let button = gamepad_button_index(other);
            format!(
                "button:{},engine:sdl,guid:{},port:{}",
                button, target.guid, target.port
            )
        }
    }
}

fn gamepad_axis_button_param(
    target: &AzaharSdlTarget,
    axis: i32,
    direction: &str,
    threshold: &str,
) -> String {
    format!(
        "axis:{},direction:{},engine:sdl,guid:{},port:{},threshold:{}",
        axis, direction, target.guid, target.port, threshold
    )
}

fn gamepad_analog_param(target: &AzaharSdlTarget, axis_x: i32, axis_y: i32) -> String {
    format!(
        "axis_x:{},axis_y:{},deadzone:0.100000,engine:sdl,guid:{},port:{}",
        axis_x, axis_y, target.guid, target.port
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

fn resolve_azahar_sdl_target(
    profile: &ControllerProfile,
    content: &str,
) -> Option<AzaharSdlTarget> {
    if is_keyboard_profile(profile) {
        return None;
    }

    read_existing_azahar_sdl_target(profile, content).or_else(|| {
        azahar_sdl_guid(profile).map(|guid| AzaharSdlTarget {
            guid,
            port: 0,
            source: "fallback-from-device-label".to_string(),
        })
    })
}

fn read_existing_azahar_sdl_target(
    profile: &ControllerProfile,
    content: &str,
) -> Option<AzaharSdlTarget> {
    let generated_guid = generated_vendor_product_guid(profile);
    let mut best: Option<(i32, AzaharSdlTarget)> = None;

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
        if line.contains("motion_device") {
            score += 2;
        }

        if best
            .as_ref()
            .map(|(best_score, _)| score > *best_score)
            .unwrap_or(true)
        {
            best = Some((
                score,
                AzaharSdlTarget {
                    guid,
                    port,
                    source: "existing-qt-config".to_string(),
                },
            ));
        }
    }

    best.map(|(_, target)| target)
}

fn azahar_sdl_guid(profile: &ControllerProfile) -> Option<String> {
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

fn compact_debug_value(value: &str) -> String {
    const MAX_LEN: usize = 120;
    if value.len() <= MAX_LEN {
        return value.to_string();
    }

    format!("{}...", &value[..MAX_LEN])
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

fn azahar_keyboard_code(input: &str) -> i32 {
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

fn empty_param() -> String {
    "[empty]".to_string()
}

fn log_azahar_controller(paths: &PortablePaths, message: &str) {
    let logs_dir = Path::new(&paths.data).join("Logs");
    if fs::create_dir_all(&logs_dir).is_err() {
        return;
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);

    let log_path = logs_dir.join("controller-mapping.log");
    let line = format!("[{}] [azahar] {}\n", timestamp, message);

    if let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        let _ = file.write_all(line.as_bytes());
    }
}
