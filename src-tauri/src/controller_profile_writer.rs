use crate::controller_profiles::{load_controller_profiles, ControllerProfile};
use crate::portable_paths::PortablePaths;
use serde::Serialize;
use std::path::Path;

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
        "azahar" => crate::azahar_controller_writer::apply_azahar_profile(paths, profile),
        "dolphin" => crate::dolphin_controller_writer::apply_dolphin_profile(paths, profile),
        "eden" => crate::eden_controller_writer::apply_eden_profile(paths, profile),
        "melonds" => crate::melonds_controller_writer::apply_melonds_profile(paths, profile),
        "pcsx2" => crate::pcsx2_controller_writer::apply_pcsx2_profile(paths, profile),
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
    apply_saved_controller_profile_for_rom(paths, emulator_id, None)
}

pub fn apply_saved_controller_profile_for_rom(
    paths: &PortablePaths,
    emulator_id: &str,
    rom_path: Option<&Path>,
) -> Result<Option<ControllerWriteResult>, String> {
    let profiles = load_controller_profiles(paths)?;
    let profile = find_saved_profile(paths, &profiles, emulator_id, rom_path);

    match profile {
        Some(profile) => apply_controller_profile(paths, profile).map(Some),
        None => Ok(None),
    }
}

pub fn apply_saved_controller_profile_for_rom_to_user_dir(
    paths: &PortablePaths,
    emulator_id: &str,
    user_dir: &Path,
    rom_path: Option<&Path>,
) -> Result<Option<ControllerWriteResult>, String> {
    let profiles = load_controller_profiles(paths)?;
    let profile = find_saved_profile(paths, &profiles, emulator_id, rom_path);

    match profile {
        Some(profile) => match emulator_id {
            "azahar" => crate::azahar_controller_writer::apply_azahar_profile_to_user_dir(
                paths, profile, user_dir,
            )
            .map(Some),
            "dolphin" => crate::dolphin_controller_writer::apply_dolphin_profile_to_user_dir(
                profile, user_dir,
            )
            .map(Some),
            "eden" => crate::eden_controller_writer::apply_eden_profile_to_user_dir(
                paths, profile, user_dir,
            )
            .map(Some),
            "pcsx2" => crate::pcsx2_controller_writer::apply_pcsx2_profile_to_install_dir(
                paths, profile, user_dir,
            )
            .map(Some),
            _ => Err(format!(
                "Ecriture de profil non implementee pour {}",
                emulator_id
            )),
        },
        None => Ok(None),
    }
}

fn find_saved_profile<'a>(
    paths: &PortablePaths,
    profiles: &'a [ControllerProfile],
    emulator_id: &str,
    rom_path: Option<&Path>,
) -> Option<&'a ControllerProfile> {
    let candidates: Vec<&ControllerProfile> = profiles
        .iter()
        .rev()
        .filter(|entry| entry.emulator_id == emulator_id)
        .collect();

    if candidates.is_empty() {
        return None;
    }

    if emulator_id == "dolphin" {
        if let Some(preference) = preferred_dolphin_profile(paths, rom_path) {
            if let Some(profile) = candidates
                .iter()
                .copied()
                .find(|profile| dolphin_profile_matches(profile, preference))
            {
                return Some(profile);
            }
        }
    }

    candidates.first().copied()
}

#[derive(Debug, Clone, Copy)]
enum DolphinProfilePreference {
    GameCube,
    Wii,
}

fn preferred_dolphin_profile(
    paths: &PortablePaths,
    rom_path: Option<&Path>,
) -> Option<DolphinProfilePreference> {
    let rom_path = rom_path?;
    let roms_root = Path::new(&paths.roms);
    let relative = rom_path.strip_prefix(roms_root).ok()?;
    let folder = relative
        .iter()
        .next()?
        .to_string_lossy()
        .to_ascii_lowercase();

    match folder.as_str() {
        "gamecube" | "gc" => Some(DolphinProfilePreference::GameCube),
        "wii" | "gamecube-wii" | "wii-gamecube" => Some(DolphinProfilePreference::Wii),
        _ => None,
    }
}

fn dolphin_profile_matches(
    profile: &ControllerProfile,
    preference: DolphinProfilePreference,
) -> bool {
    let controller_id = profile
        .emulated_controller_id
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let label = profile.emulated_device_label.to_ascii_lowercase();
    let is_gamecube = controller_id == "gamecube" || label.contains("gamecube");

    match preference {
        DolphinProfilePreference::GameCube => is_gamecube,
        DolphinProfilePreference::Wii => !is_gamecube,
    }
}
