use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorDefinition {
    pub id: &'static str,
    pub name: &'static str,
    pub platform_label: &'static str,
    pub catalog_version: &'static str,
    pub install_dir_name: &'static str,
    pub executable_rel_path: &'static str,
    pub supported: bool,
    pub download_url: Option<&'static str>,
    pub archive_format: Option<&'static str>,
    pub executable_name_candidates: Vec<&'static str>,
}

pub fn built_in_emulators() -> Vec<EmulatorDefinition> {
    vec![
        EmulatorDefinition {
            id: "dolphin",
            name: "Dolphin",
            platform_label: "GameCube / Wii",
            catalog_version: "2603a",
            install_dir_name: "Dolphin",
            executable_rel_path: "Dolphin-x64\\Dolphin.exe",
            supported: true,
            download_url: Some("https://dl.dolphin-emu.org/releases/2603a/dolphin-2603a-x64.7z"),
            archive_format: Some("7z"),
            executable_name_candidates: vec!["Dolphin.exe"],
        },
        EmulatorDefinition {
            id: "melonds",
            name: "melonDS",
            platform_label: "Nintendo DS",
            catalog_version: "1.1",
            install_dir_name: "melonDS",
            executable_rel_path: "melonDS.exe",
            supported: true,
            download_url: Some("https://github.com/melonDS-emu/melonDS/releases/download/1.1/melonDS-1.1-windows-x86_64.zip"),
            archive_format: Some("zip"),
            executable_name_candidates: vec!["melonDS.exe"],
        },
        EmulatorDefinition {
            id: "azahar",
            name: "Azahar",
            platform_label: "Nintendo 3DS",
            catalog_version: "2125.1.1",
            install_dir_name: "Azahar",
            executable_rel_path: "azahar.exe",
            supported: true,
            download_url: Some("https://github.com/azahar-emu/azahar/releases/download/2125.1.1/azahar-windows-msys2-2125.1.1.zip"),
            archive_format: Some("zip"),
            executable_name_candidates: vec!["azahar.exe", "Azahar.exe"],
        },
        EmulatorDefinition {
            id: "eden",
            name: "Eden",
            platform_label: "Nintendo Switch",
            catalog_version: "0.2.0-rc2",
            install_dir_name: "Eden",
            executable_rel_path: "eden.exe",
            supported: true,
            download_url: Some("https://git.eden-emu.dev/eden-emu/eden/releases/download/v0.2.0-rc2/Eden-Windows-v0.2.0-rc2-amd64-msvc-standard.zip"),
            archive_format: Some("zip"),
            executable_name_candidates: vec!["eden.exe", "Eden.exe"],
        },
        EmulatorDefinition {
            id: "pcsx2",
            name: "PCSX2",
            platform_label: "PS2",
            catalog_version: "2.7.281",
            install_dir_name: "PCSX2",
            executable_rel_path: "pcsx2-qt.exe",
            supported: true,
            download_url: Some("https://github.com/PCSX2/pcsx2/releases/download/v2.7.281/pcsx2-v2.7.281-windows-x64-Qt.7z"),
            archive_format: Some("7z"),
            executable_name_candidates: vec!["pcsx2-qt.exe"],
        },
    ]
}