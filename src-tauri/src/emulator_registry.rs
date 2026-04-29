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
    #[serde(skip_serializing)]
    pub download_source: Option<EmulatorDownloadSource>,
    pub executable_name_candidates: Vec<&'static str>,
}

#[derive(Debug, Clone)]
pub enum EmulatorDownloadSource {
    GitHubLatest(GitHubReleaseSource),
    GenericLatestReleaseApi(GenericReleaseApiSource),
    Direct(DirectDownloadSource),
}

#[derive(Debug, Clone)]
pub struct GitHubReleaseSource {
    pub owner: &'static str,
    pub repo: &'static str,
    pub asset_filters: Vec<ReleaseAssetFilter>,
}

#[derive(Debug, Clone)]
pub struct GenericReleaseApiSource {
    pub api_url: &'static str,
    pub cache_key: &'static str,
    pub asset_filters: Vec<ReleaseAssetFilter>,
}

#[derive(Debug, Clone)]
pub struct DirectDownloadSource {
    pub url: &'static str,
}

#[derive(Debug, Clone)]
pub struct ReleaseAssetFilter {
    pub platform: ReleaseAssetPlatform,
    pub required_substrings: Vec<&'static str>,
    pub excluded_substrings: Vec<&'static str>,
    pub extensions: Vec<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseAssetPlatform {
    Windows,
    Macos,
    Linux,
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
            download_source: Some(EmulatorDownloadSource::Direct(DirectDownloadSource {
                url: "https://dl.dolphin-emu.org/releases/2603a/dolphin-2603a-x64.7z",
            })),
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
            download_source: Some(EmulatorDownloadSource::GitHubLatest(GitHubReleaseSource {
                owner: "melonDS-emu",
                repo: "melonDS",
                asset_filters: vec![ReleaseAssetFilter {
                    platform: ReleaseAssetPlatform::Windows,
                    required_substrings: vec!["windows", "x86_64"],
                    excluded_substrings: vec!["aarch64"],
                    extensions: vec![".zip"],
                }],
            })),
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
            download_source: Some(EmulatorDownloadSource::GitHubLatest(GitHubReleaseSource {
                owner: "azahar-emu",
                repo: "azahar",
                asset_filters: vec![ReleaseAssetFilter {
                    platform: ReleaseAssetPlatform::Windows,
                    required_substrings: vec!["windows", "msys2"],
                    excluded_substrings: vec!["installer", "libretro"],
                    extensions: vec![".zip"],
                }],
            })),
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
            download_source: Some(EmulatorDownloadSource::GenericLatestReleaseApi(
                GenericReleaseApiSource {
                    api_url: "https://git.eden-emu.dev/api/v1/repos/eden-emu/eden/releases/latest",
                    cache_key: "forgejo:git.eden-emu.dev/eden-emu/eden",
                    asset_filters: vec![ReleaseAssetFilter {
                        platform: ReleaseAssetPlatform::Windows,
                        required_substrings: vec!["windows", "amd64", "msvc", "standard"],
                        excluded_substrings: vec![],
                        extensions: vec![".zip"],
                    }],
                },
            )),
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
            download_source: Some(EmulatorDownloadSource::GitHubLatest(GitHubReleaseSource {
                owner: "PCSX2",
                repo: "pcsx2",
                asset_filters: vec![ReleaseAssetFilter {
                    platform: ReleaseAssetPlatform::Windows,
                    required_substrings: vec!["windows", "x64", "qt"],
                    excluded_substrings: vec!["installer", "symbols"],
                    extensions: vec![".7z"],
                }],
            })),
            executable_name_candidates: vec!["pcsx2-qt.exe"],
        },
    ]
}
