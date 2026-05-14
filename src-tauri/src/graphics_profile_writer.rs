use crate::emulator_installer::resolve_emulator_executable;
use crate::graphics_profiles::{load_graphics_profiles, GraphicsProfile};
use crate::portable_paths::PortablePaths;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use toml_edit::{value, DocumentMut};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphicsWriteResult {
    pub emulator_id: String,
    pub profile_id: String,
    pub config_paths: Vec<String>,
}

pub fn apply_graphics_profile(
    paths: &PortablePaths,
    profile: &GraphicsProfile,
) -> Result<GraphicsWriteResult, String> {
    match profile.emulator_id.as_str() {
        "dolphin" => apply_dolphin_graphics(paths, profile),
        "pcsx2" => apply_pcsx2_graphics(paths, profile),
        "eden" => apply_eden_graphics(paths, profile),
        "azahar" => apply_azahar_graphics(paths, profile),
        "melonds" => apply_melonds_graphics(paths, profile),
        _ => Err(format!(
            "Configuration graphique non implementee pour {}",
            profile.emulator_id
        )),
    }
}

pub fn apply_saved_graphics_profile(
    paths: &PortablePaths,
    emulator_id: &str,
) -> Result<Option<GraphicsWriteResult>, String> {
    let profiles = load_graphics_profiles(paths)?;
    let Some(profile) = profiles
        .iter()
        .find(|profile| profile.emulator_id == emulator_id)
    else {
        return Ok(None);
    };

    apply_graphics_profile(paths, profile).map(Some)
}

fn apply_dolphin_graphics(
    paths: &PortablePaths,
    profile: &GraphicsProfile,
) -> Result<GraphicsWriteResult, String> {
    let executable_path = resolve_emulator_executable(paths, "dolphin")?;
    let executable_dir = executable_path
        .parent()
        .ok_or_else(|| "Impossible de determiner le dossier Dolphin".to_string())?;
    let config_dir = executable_dir.join("User").join("Config");
    fs::create_dir_all(&config_dir)
        .map_err(|error| format!("Impossible de creer User/Config Dolphin: {}", error))?;

    let dolphin_ini = config_dir.join("Dolphin.ini");
    let gfx_ini = config_dir.join("GFX.ini");

    update_ini_file(&dolphin_ini, |content| {
        set_ini_value(
            content,
            "Core",
            "GFXBackend",
            dolphin_backend(&profile.graphics_api),
        );
        set_ini_value(
            content,
            "Display",
            "Fullscreen",
            bool_text(profile.fullscreen),
        );
    })?;

    update_ini_file(&gfx_ini, |content| {
        set_ini_value(content, "Hardware", "VSync", bool_text(profile.vsync));
        set_ini_value(
            content,
            "Settings",
            "InternalResolution",
            &profile.resolution_scale.to_string(),
        );
        set_ini_value(
            content,
            "Settings",
            "AspectRatio",
            dolphin_aspect(&profile.aspect_ratio),
        );
        set_ini_value(
            content,
            "Settings",
            "MSAA",
            anti_aliasing_value(&profile.anti_aliasing),
        );
        set_ini_value(
            content,
            "Settings",
            "ShaderCache",
            bool_text(profile.shader_cache),
        );
        set_ini_value(
            content,
            "Settings",
            "wideScreenHack",
            bool_text(profile.widescreen_hack),
        );
        set_ini_value(
            content,
            "Enhancements",
            "MaxAnisotropy",
            dolphin_anisotropy_value(&profile.anisotropic_filtering),
        );
        set_ini_value(
            content,
            "Enhancements",
            "ForceTextureFiltering",
            dolphin_texture_filtering(&profile.texture_filtering),
        );
    })?;

    write_result(profile, vec![dolphin_ini, gfx_ini])
}

fn apply_pcsx2_graphics(
    paths: &PortablePaths,
    profile: &GraphicsProfile,
) -> Result<GraphicsWriteResult, String> {
    let executable_path = resolve_emulator_executable(paths, "pcsx2")?;
    let install_dir = executable_path
        .parent()
        .ok_or_else(|| "Impossible de determiner le dossier PCSX2".to_string())?;
    let config_dir = install_dir.join("inis");
    fs::create_dir_all(&config_dir)
        .map_err(|error| format!("Impossible de creer inis PCSX2: {}", error))?;
    let config_path = config_dir.join("PCSX2.ini");

    update_ini_file(&config_path, |content| {
        set_ini_value(
            content,
            "UI",
            "StartFullscreen",
            bool_lower(profile.fullscreen),
        );
        set_ini_value(
            content,
            "EmuCore/GS",
            "Renderer",
            pcsx2_renderer(&profile.graphics_api),
        );
        set_ini_value(
            content,
            "EmuCore/GS",
            "upscale_multiplier",
            &profile.resolution_scale.to_string(),
        );
        set_ini_value(
            content,
            "EmuCore/GS",
            "AspectRatio",
            pcsx2_aspect(&profile.aspect_ratio),
        );
        set_ini_value(
            content,
            "EmuCore/GS",
            "VsyncEnable",
            bool_lower(profile.vsync),
        );
        set_ini_value(
            content,
            "EmuCore/GS",
            "filter",
            pcsx2_texture_filtering(&profile.texture_filtering),
        );
        set_ini_value(
            content,
            "EmuCore/GS",
            "MaxAnisotropy",
            anisotropy_value(&profile.anisotropic_filtering),
        );
        set_ini_value(
            content,
            "EmuCore/GS",
            "fxaa",
            bool_lower(profile.anti_aliasing == "fxaa"),
        );
        set_ini_value(
            content,
            "EmuCore/GS",
            "disable_shader_cache",
            bool_lower(!profile.shader_cache),
        );
        set_ini_value(
            content,
            "EmuCore",
            "EnableWideScreenPatches",
            bool_lower(profile.widescreen_hack),
        );
    })?;

    write_result(profile, vec![config_path])
}

fn apply_eden_graphics(
    paths: &PortablePaths,
    profile: &GraphicsProfile,
) -> Result<GraphicsWriteResult, String> {
    let executable_path = resolve_emulator_executable(paths, "eden")?;
    let executable_dir = executable_path
        .parent()
        .ok_or_else(|| "Impossible de determiner le dossier Eden".to_string())?;
    let config_dir = executable_dir.join("user").join("config");
    fs::create_dir_all(&config_dir)
        .map_err(|error| format!("Impossible de creer user/config Eden: {}", error))?;
    let config_path = config_dir.join("qt-config.ini");

    update_ini_file(&config_path, |content| {
        set_qt_value(
            content,
            "Renderer",
            "backend",
            eden_backend(&profile.graphics_api),
        );
        set_qt_value(
            content,
            "Renderer",
            "resolution_setup",
            &eden_resolution_setup(profile.resolution_scale).to_string(),
        );
        set_qt_value(content, "Renderer", "use_vsync", eden_vsync(profile.vsync));
        set_qt_value(
            content,
            "Renderer",
            "use_disk_shader_cache",
            bool_lower(profile.shader_cache),
        );
        set_qt_value(
            content,
            "Renderer",
            "scaling_filter",
            eden_scaling_filter(&profile.texture_filtering),
        );
        set_qt_value(
            content,
            "Renderer",
            "anti_aliasing",
            eden_anti_aliasing(&profile.anti_aliasing),
        );
        set_qt_value(
            content,
            "Renderer",
            "aspect_ratio",
            eden_aspect(&profile.aspect_ratio),
        );
        set_qt_value(
            content,
            "Renderer",
            "max_anisotropy",
            eden_anisotropy(&profile.anisotropic_filtering),
        );
        set_qt_value(content, "UI", "fullscreen", bool_lower(profile.fullscreen));
    })?;

    write_result(profile, vec![config_path])
}

fn apply_azahar_graphics(
    paths: &PortablePaths,
    profile: &GraphicsProfile,
) -> Result<GraphicsWriteResult, String> {
    let executable_path = resolve_emulator_executable(paths, "azahar")?;
    let executable_dir = executable_path
        .parent()
        .ok_or_else(|| "Impossible de determiner le dossier Azahar".to_string())?;
    let config_dir = executable_dir.join("user").join("config");
    fs::create_dir_all(&config_dir)
        .map_err(|error| format!("Impossible de creer user/config Azahar: {}", error))?;
    let config_path = config_dir.join("qt-config.ini");

    update_ini_file(&config_path, |content| {
        set_qt_value(
            content,
            "Renderer",
            "graphics_api",
            azahar_graphics_api(&profile.graphics_api),
        );
        set_qt_value(
            content,
            "Renderer",
            "resolution_factor",
            &profile.resolution_scale.to_string(),
        );
        set_qt_value(
            content,
            "Renderer",
            "use_vsync_new",
            bool_lower(profile.vsync),
        );
        set_qt_value(
            content,
            "Renderer",
            "use_disk_shader_cache",
            bool_lower(profile.shader_cache),
        );
        set_qt_value(
            content,
            "Renderer",
            "texture_filter",
            azahar_texture_filter(&profile.texture_filtering),
        );
        set_qt_value(
            content,
            "Renderer",
            "texture_filter_name",
            azahar_texture_filter_name(&profile.texture_filtering),
        );
        set_qt_value(
            content,
            "Layout",
            "layout_option",
            azahar_layout(&profile.aspect_ratio),
        );
        set_qt_value(content, "UI", "fullscreen", bool_lower(profile.fullscreen));
    })?;

    write_result(profile, vec![config_path])
}

fn apply_melonds_graphics(
    paths: &PortablePaths,
    profile: &GraphicsProfile,
) -> Result<GraphicsWriteResult, String> {
    let executable_path = resolve_emulator_executable(paths, "melonds")?;
    let config_dir = executable_path
        .parent()
        .ok_or_else(|| "Impossible de determiner le dossier melonDS".to_string())?;
    let config_path = config_dir.join("melonDS.toml");
    let raw_config = fs::read_to_string(&config_path).unwrap_or_else(|_| String::new());
    let mut document = raw_config
        .parse::<DocumentMut>()
        .unwrap_or_else(|_| DocumentMut::new());

    document["Screen"]["VSync"] = value(profile.vsync);
    document["Screen"]["VSyncInterval"] = value(1);
    document["Screen"]["UseGL"] = value(profile.graphics_api != "software");
    document["Screen"]["Filter"] = value(profile.texture_filtering != "nearest");
    document["3D"]["Renderer"] = value(i64::from(melonds_renderer(&profile.graphics_api)));
    document["3D"]["GL"]["ScaleFactor"] = value(i64::from(profile.resolution_scale));
    document["Window0"]["IntegerScaling"] = value(profile.integer_scaling);
    document["Window0"]["ScreenAspectTop"] =
        value(i64::from(melonds_aspect(&profile.aspect_ratio)));
    document["Window0"]["ScreenAspectBot"] =
        value(i64::from(melonds_aspect(&profile.aspect_ratio)));

    fs::write(&config_path, document.to_string())
        .map_err(|error| format!("Impossible d'ecrire melonDS.toml: {}", error))?;

    write_result(profile, vec![config_path])
}

fn write_result(
    profile: &GraphicsProfile,
    paths: Vec<PathBuf>,
) -> Result<GraphicsWriteResult, String> {
    Ok(GraphicsWriteResult {
        emulator_id: profile.emulator_id.clone(),
        profile_id: profile.id.clone(),
        config_paths: paths
            .into_iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect(),
    })
}

fn update_ini_file(path: &Path, update: impl FnOnce(&mut String)) -> Result<(), String> {
    let mut content = fs::read_to_string(path).unwrap_or_else(|_| String::new());
    update(&mut content);
    fs::write(path, content).map_err(|error| {
        format!(
            "Impossible d'ecrire la configuration graphique {}: {}",
            path.to_string_lossy(),
            error
        )
    })
}

fn set_ini_value(content: &mut String, section: &str, key: &str, value: &str) {
    let had_final_newline = content.ends_with('\n') || content.ends_with("\r\n");
    let mut lines = content.lines().map(str::to_string).collect::<Vec<_>>();
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
                .is_some_and(|(candidate, _)| candidate.trim().eq_ignore_ascii_case(key))
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

fn set_qt_value(content: &mut String, section: &str, key: &str, value: &str) {
    set_ini_value(content, section, key, value);
    set_ini_value(content, section, &format!("{}\\default", key), "false");
}

fn bool_text(value: bool) -> &'static str {
    if value {
        "True"
    } else {
        "False"
    }
}

fn bool_lower(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

fn dolphin_backend(value: &str) -> &'static str {
    match value {
        "opengl" => "OGL",
        "direct3d11" => "D3D",
        "direct3d12" => "D3D12",
        "software" => "Software Renderer",
        _ => "Vulkan",
    }
}

fn dolphin_aspect(value: &str) -> &'static str {
    match value {
        "16:9" => "1",
        "4:3" => "2",
        "stretch" => "3",
        _ => "0",
    }
}

fn dolphin_texture_filtering(value: &str) -> &'static str {
    match value {
        "nearest" => "1",
        "linear" => "2",
        _ => "0",
    }
}

fn dolphin_anisotropy_value(value: &str) -> &'static str {
    match value {
        "2x" => "1",
        "4x" => "2",
        "8x" => "3",
        "16x" => "4",
        _ => "-1",
    }
}

fn pcsx2_renderer(value: &str) -> &'static str {
    match value {
        "opengl" => "12",
        "direct3d11" => "3",
        "direct3d12" => "15",
        "software" => "13",
        _ => "14",
    }
}

fn pcsx2_aspect(value: &str) -> &'static str {
    match value {
        "4:3" => "4:3",
        "16:9" => "16:9",
        "stretch" => "Stretch",
        _ => "Auto 4:3/3:2",
    }
}

fn pcsx2_texture_filtering(value: &str) -> &'static str {
    match value {
        "nearest" => "0",
        "forced" => "1",
        _ => "2",
    }
}

fn eden_backend(value: &str) -> &'static str {
    match value {
        "opengl" => "0",
        _ => "1",
    }
}

fn eden_resolution_setup(scale: u32) -> u32 {
    match scale {
        1 => 3,
        2 => 6,
        3 => 7,
        4 => 8,
        5 => 9,
        6 => 10,
        7 => 11,
        _ => 12,
    }
}

fn eden_vsync(enabled: bool) -> &'static str {
    if enabled {
        "2"
    } else {
        "0"
    }
}

fn eden_scaling_filter(value: &str) -> &'static str {
    match value {
        "nearest" => "0",
        "bicubic" => "2",
        "gaussian" => "3",
        "lanczos" => "4",
        "scaleforce" => "5",
        "fsr" => "6",
        _ => "1",
    }
}

fn eden_anti_aliasing(value: &str) -> &'static str {
    match value {
        "fxaa" | "2x" | "4x" | "8x" => "1",
        "smaa" => "2",
        _ => "0",
    }
}

fn eden_aspect(value: &str) -> &'static str {
    match value {
        "4:3" => "1",
        "21:9" => "2",
        "16:10" => "3",
        "stretch" => "4",
        _ => "0",
    }
}

fn eden_anisotropy(value: &str) -> &'static str {
    match value {
        "2x" => "2",
        "4x" => "3",
        "8x" => "4",
        "16x" => "5",
        _ => "8",
    }
}

fn azahar_graphics_api(value: &str) -> &'static str {
    match value {
        "software" => "0",
        "opengl" => "1",
        _ => "2",
    }
}

fn azahar_texture_filter(value: &str) -> &'static str {
    match value {
        "anime4k" => "1",
        "bicubic" => "2",
        "scaleforce" => "3",
        "xbrz" => "4",
        "mmpx" => "5",
        _ => "0",
    }
}

fn azahar_texture_filter_name(value: &str) -> &'static str {
    match value {
        "anime4k" => "Anime4K",
        "bicubic" => "Bicubic",
        "scaleforce" => "ScaleForce",
        "xbrz" => "xBRZ",
        "mmpx" => "MMPX",
        _ => "Linear (Default)",
    }
}

fn azahar_layout(value: &str) -> &'static str {
    match value {
        "single" => "1",
        "large" => "2",
        "side" => "3",
        "hybrid" => "5",
        _ => "0",
    }
}

fn melonds_renderer(value: &str) -> i32 {
    match value {
        "opengl" => 1,
        _ => 0,
    }
}

fn melonds_aspect(value: &str) -> i32 {
    match value {
        "16:9" => 1,
        "stretch" => 3,
        _ => 0,
    }
}

fn anisotropy_value(value: &str) -> &'static str {
    match value {
        "2x" => "2",
        "4x" => "4",
        "8x" => "8",
        "16x" => "16",
        _ => "0",
    }
}

fn anti_aliasing_value(value: &str) -> &'static str {
    match value {
        "2x" => "2",
        "4x" => "4",
        "8x" => "8",
        _ => "1",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn writes_current_dolphin_keys() {
        let paths = test_paths("dolphin");
        let install_dir = Path::new(&paths.emu).join("Dolphin");
        fs::create_dir_all(&install_dir).unwrap();
        fs::write(install_dir.join("Dolphin.exe"), "").unwrap();

        let mut profile = profile("dolphin");
        profile.graphics_api = "direct3d12".to_string();
        profile.fullscreen = true;
        profile.vsync = true;
        profile.resolution_scale = 3;
        profile.aspect_ratio = "16:9".to_string();
        profile.anti_aliasing = "4x".to_string();
        profile.anisotropic_filtering = "16x".to_string();
        profile.texture_filtering = "linear".to_string();
        profile.shader_cache = false;
        profile.widescreen_hack = true;

        apply_graphics_profile(&paths, &profile).unwrap();

        let config_dir = install_dir.join("User").join("Config");
        let dolphin_ini = fs::read_to_string(config_dir.join("Dolphin.ini")).unwrap();
        let gfx_ini = fs::read_to_string(config_dir.join("GFX.ini")).unwrap();

        assert!(dolphin_ini.contains("GFXBackend = D3D12"));
        assert!(dolphin_ini.contains("Fullscreen = True"));
        assert!(gfx_ini.contains("[Hardware]"));
        assert!(gfx_ini.contains("VSync = True"));
        assert!(gfx_ini.contains("InternalResolution = 3"));
        assert!(gfx_ini.contains("AspectRatio = 1"));
        assert!(gfx_ini.contains("MSAA = 4"));
        assert!(gfx_ini.contains("ShaderCache = False"));
        assert!(gfx_ini.contains("wideScreenHack = True"));
        assert!(gfx_ini.contains("MaxAnisotropy = 4"));
        assert!(gfx_ini.contains("ForceTextureFiltering = 2"));

        cleanup(paths);
    }

    #[test]
    fn writes_pcsx2_qt_keys_without_legacy_graphics_names() {
        let paths = test_paths("pcsx2");
        let install_dir = Path::new(&paths.emu).join("PCSX2");
        fs::create_dir_all(&install_dir).unwrap();
        fs::write(install_dir.join("pcsx2-qt.exe"), "").unwrap();

        let mut profile = profile("pcsx2");
        profile.graphics_api = "vulkan".to_string();
        profile.fullscreen = true;
        profile.vsync = true;
        profile.resolution_scale = 4;
        profile.aspect_ratio = "stretch".to_string();
        profile.anti_aliasing = "fxaa".to_string();
        profile.anisotropic_filtering = "8x".to_string();
        profile.texture_filtering = "xbrz".to_string();
        profile.shader_cache = false;
        profile.widescreen_hack = true;

        apply_graphics_profile(&paths, &profile).unwrap();

        let ini = fs::read_to_string(install_dir.join("inis").join("PCSX2.ini")).unwrap();
        assert!(ini.contains("StartFullscreen = true"));
        assert!(ini.contains("Renderer = 14"));
        assert!(ini.contains("upscale_multiplier = 4"));
        assert!(ini.contains("AspectRatio = Stretch"));
        assert!(ini.contains("filter = 2"));
        assert!(ini.contains("MaxAnisotropy = 8"));
        assert!(ini.contains("fxaa = true"));
        assert!(ini.contains("disable_shader_cache = true"));
        assert!(ini.contains("EnableWideScreenPatches = true"));
        assert!(!ini.contains("TextureFiltering"));
        assert!(!ini.contains("AntiAliasing"));
        assert!(!ini.contains("WidescreenHack"));

        cleanup(paths);
    }

    #[test]
    fn writes_separate_eden_and_azahar_qt_keys() {
        let paths = test_paths("qt");
        let eden_dir = Path::new(&paths.emu).join("Eden");
        let azahar_dir = Path::new(&paths.emu).join("Azahar");
        fs::create_dir_all(&eden_dir).unwrap();
        fs::create_dir_all(&azahar_dir).unwrap();
        fs::write(eden_dir.join("eden.exe"), "").unwrap();
        fs::write(azahar_dir.join("azahar.exe"), "").unwrap();

        let mut eden = profile("eden");
        eden.graphics_api = "opengl".to_string();
        eden.resolution_scale = 3;
        eden.vsync = true;
        eden.texture_filtering = "fsr".to_string();
        eden.anti_aliasing = "smaa".to_string();
        eden.anisotropic_filtering = "16x".to_string();
        eden.aspect_ratio = "stretch".to_string();
        eden.fullscreen = true;
        apply_graphics_profile(&paths, &eden).unwrap();

        let eden_ini =
            fs::read_to_string(eden_dir.join("user").join("config").join("qt-config.ini")).unwrap();
        assert!(eden_ini.contains("backend = 0"));
        assert!(eden_ini.contains("resolution_setup = 7"));
        assert!(eden_ini.contains("use_vsync = 2"));
        assert!(eden_ini.contains("scaling_filter = 6"));
        assert!(eden_ini.contains("anti_aliasing = 2"));
        assert!(eden_ini.contains("aspect_ratio = 4"));
        assert!(eden_ini.contains("max_anisotropy = 5"));
        assert!(!eden_ini.contains("api ="));

        let mut azahar = profile("azahar");
        azahar.graphics_api = "vulkan".to_string();
        azahar.resolution_scale = 4;
        azahar.vsync = false;
        azahar.texture_filtering = "xbrz".to_string();
        azahar.aspect_ratio = "hybrid".to_string();
        apply_graphics_profile(&paths, &azahar).unwrap();

        let azahar_ini =
            fs::read_to_string(azahar_dir.join("user").join("config").join("qt-config.ini"))
                .unwrap();
        assert!(azahar_ini.contains("graphics_api = 2"));
        assert!(azahar_ini.contains("resolution_factor = 4"));
        assert!(azahar_ini.contains("use_vsync_new = false"));
        assert!(azahar_ini.contains("texture_filter = 4"));
        assert!(azahar_ini.contains("texture_filter_name = xBRZ"));
        assert!(azahar_ini.contains("layout_option = 5"));
        assert!(!azahar_ini.contains("backend ="));

        cleanup(paths);
    }

    #[test]
    fn writes_melonds_supported_toml_keys() {
        let paths = test_paths("melonds");
        let install_dir = Path::new(&paths.emu).join("melonDS");
        fs::create_dir_all(&install_dir).unwrap();
        fs::write(install_dir.join("melonDS.exe"), "").unwrap();

        let mut profile = profile("melonds");
        profile.graphics_api = "opengl".to_string();
        profile.resolution_scale = 4;
        profile.vsync = true;
        profile.aspect_ratio = "16:9".to_string();
        profile.texture_filtering = "nearest".to_string();
        profile.integer_scaling = true;

        apply_graphics_profile(&paths, &profile).unwrap();

        let raw = fs::read_to_string(install_dir.join("melonDS.toml")).unwrap();
        let document = raw.parse::<DocumentMut>().unwrap();
        assert_eq!(document["Screen"]["VSync"].as_bool(), Some(true));
        assert_eq!(document["Screen"]["UseGL"].as_bool(), Some(true));
        assert_eq!(document["Screen"]["Filter"].as_bool(), Some(false));
        assert_eq!(document["3D"]["Renderer"].as_integer(), Some(1));
        assert_eq!(document["3D"]["GL"]["ScaleFactor"].as_integer(), Some(4));
        assert_eq!(document["Window0"]["IntegerScaling"].as_bool(), Some(true));
        assert_eq!(document["Window0"]["ScreenAspectTop"].as_integer(), Some(1));
        assert!(!raw.contains("Instance0.Video"));

        cleanup(paths);
    }

    fn profile(emulator_id: &str) -> GraphicsProfile {
        GraphicsProfile {
            id: format!("graphics-{}", emulator_id),
            emulator_id: emulator_id.to_string(),
            platform_label: "Test".to_string(),
            mode: "advanced".to_string(),
            preset: "greater".to_string(),
            resolution_scale: 2,
            graphics_api: "vulkan".to_string(),
            fullscreen: false,
            vsync: false,
            aspect_ratio: "auto".to_string(),
            anti_aliasing: "off".to_string(),
            anisotropic_filtering: "off".to_string(),
            texture_filtering: "linear".to_string(),
            shader_cache: true,
            widescreen_hack: false,
            integer_scaling: false,
        }
    }

    fn test_paths(name: &str) -> PortablePaths {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "emumanager-graphics-{}-{}-{}",
            name,
            std::process::id(),
            nonce
        ));
        let emu = root.join("Emu");
        let roms = root.join("Roms");
        let saves = root.join("Saves");
        let firmware = root.join("Firmware");
        let config = root.join("Config");
        let data = root.join("Data");

        for path in [&emu, &roms, &saves, &firmware, &config, &data] {
            fs::create_dir_all(path).unwrap();
        }

        PortablePaths {
            root: root.to_string_lossy().to_string(),
            emu: emu.to_string_lossy().to_string(),
            roms: roms.to_string_lossy().to_string(),
            saves: saves.to_string_lossy().to_string(),
            firmware: firmware.to_string_lossy().to_string(),
            config: config.to_string_lossy().to_string(),
            data: data.to_string_lossy().to_string(),
        }
    }

    fn cleanup(paths: PortablePaths) {
        let _ = fs::remove_dir_all(paths.root);
    }
}
