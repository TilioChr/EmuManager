use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorDefinition {
    pub id: &'static str,
    pub name: &'static str,
    pub platform_label: &'static str,
    pub install_dir_name: &'static str,
    pub executable_rel_path: &'static str,
    pub supported: bool,
}

pub fn built_in_emulators() -> Vec<EmulatorDefinition> {
    vec![
        EmulatorDefinition {
            id: "dolphin",
            name: "Dolphin",
            platform_label: "GameCube / Wii",
            install_dir_name: "Dolphin",
            executable_rel_path: "Dolphin-x64\\Dolphin.exe",
            supported: true,
        },
        EmulatorDefinition {
            id: "ppsspp",
            name: "PPSSPP",
            platform_label: "PSP",
            install_dir_name: "PPSSPP",
            executable_rel_path: "PPSSPPWindows64\\PPSSPPWindows64.exe",
            supported: true,
        },
        EmulatorDefinition {
            id: "pcsx2",
            name: "PCSX2",
            platform_label: "PS2",
            install_dir_name: "PCSX2",
            executable_rel_path: "pcsx2\\pcsx2-qt.exe",
            supported: true,
        },
        EmulatorDefinition {
            id: "duckstation",
            name: "DuckStation",
            platform_label: "PS1",
            install_dir_name: "DuckStation",
            executable_rel_path: "duckstation\\duckstation-qt-x64-ReleaseLTCG.exe",
            supported: true,
        },
    ]
}