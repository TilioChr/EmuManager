use crate::portable_paths::PortablePaths;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use unrar::Archive;
use zip::ZipArchive;

const DUPLICATE_IMPORT_ERROR_PREFIX: &str = "DUPLICATE_IMPORT:";

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualImportPlatform {
    pub id: &'static str,
    pub label: &'static str,
    pub folder: &'static str,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualImportRequest {
    pub source_path: String,
    pub platform_id: String,
    pub overwrite: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualImportedRom {
    pub file_name: String,
    pub file_path: String,
    pub file_size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualImportResult {
    pub platform_id: String,
    pub platform_label: String,
    pub target_directory: String,
    pub source_kind: String,
    pub imported_roms: Vec<ManualImportedRom>,
}

#[derive(Debug, Clone, Copy)]
enum ImportSourceKind {
    Rom,
    Zip,
    Rar,
}

impl ImportSourceKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Rom => "rom",
            Self::Zip => "zip",
            Self::Rar => "rar",
        }
    }
}

const MANUAL_IMPORT_PLATFORMS: &[ManualImportPlatform] = &[
    ManualImportPlatform {
        id: "gamecube-wii",
        label: "Wii / GameCube",
        folder: "gamecube-wii",
    },
    ManualImportPlatform {
        id: "nds",
        label: "Nintendo DS",
        folder: "nds",
    },
    ManualImportPlatform {
        id: "3ds",
        label: "Nintendo 3DS",
        folder: "3ds",
    },
    ManualImportPlatform {
        id: "switch",
        label: "Nintendo Switch",
        folder: "switch",
    },
    ManualImportPlatform {
        id: "ps2",
        label: "PS2",
        folder: "ps2",
    },
];

pub fn manual_import_platforms() -> Vec<ManualImportPlatform> {
    MANUAL_IMPORT_PLATFORMS.to_vec()
}

pub fn import_local_rom(
    paths: &PortablePaths,
    request: &ManualImportRequest,
) -> Result<ManualImportResult, String> {
    let platform = find_platform(&request.platform_id)
        .ok_or_else(|| format!("Unsupported platform: {}", request.platform_id))?;
    let source_path = PathBuf::from(&request.source_path);
    let source_kind = classify_source(&source_path)?;

    let metadata = fs::metadata(&source_path).map_err(|error| {
        format!(
            "Unable to read dropped file {}: {}",
            source_path.to_string_lossy(),
            error
        )
    })?;

    if !metadata.is_file() {
        return Err("Only files can be imported.".to_string());
    }

    let target_directory = PathBuf::from(&paths.roms).join(platform.folder);
    fs::create_dir_all(&target_directory).map_err(|error| {
        format!(
            "Unable to create target ROM folder {}: {}",
            target_directory.to_string_lossy(),
            error
        )
    })?;

    let imported_roms = match source_kind {
        ImportSourceKind::Rom => {
            import_direct_rom(&source_path, &target_directory, request.overwrite)?
        }
        ImportSourceKind::Zip | ImportSourceKind::Rar => import_archive_roms(
            paths,
            &source_path,
            &target_directory,
            source_kind,
            request.overwrite,
        )?,
    };

    Ok(ManualImportResult {
        platform_id: platform.id.to_string(),
        platform_label: platform.label.to_string(),
        target_directory: target_directory.to_string_lossy().to_string(),
        source_kind: source_kind.as_str().to_string(),
        imported_roms,
    })
}

fn find_platform(platform_id: &str) -> Option<ManualImportPlatform> {
    MANUAL_IMPORT_PLATFORMS
        .iter()
        .copied()
        .find(|platform| platform.id == platform_id)
}

fn classify_source(source_path: &Path) -> Result<ImportSourceKind, String> {
    let extension = source_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match extension.as_str() {
        "zip" => Ok(ImportSourceKind::Zip),
        "rar" => Ok(ImportSourceKind::Rar),
        _ if is_supported_manual_import_rom_file(source_path) => Ok(ImportSourceKind::Rom),
        _ => Err(format!(
            "Unsupported file type{}.",
            if extension.is_empty() {
                "".to_string()
            } else {
                format!(" .{}", extension)
            }
        )),
    }
}

fn import_direct_rom(
    source_path: &Path,
    target_directory: &Path,
    overwrite: bool,
) -> Result<Vec<ManualImportedRom>, String> {
    let file_name = file_name_for_target(source_path)?;
    assert_targets_available(target_directory, &[file_name.clone()], overwrite)?;

    let destination = target_directory.join(&file_name);
    let file_size_bytes = move_file_to_destination(source_path, &destination, overwrite)?;

    Ok(vec![ManualImportedRom {
        file_name,
        file_path: destination.to_string_lossy().to_string(),
        file_size_bytes,
    }])
}

fn import_archive_roms(
    paths: &PortablePaths,
    source_path: &Path,
    target_directory: &Path,
    source_kind: ImportSourceKind,
    overwrite: bool,
) -> Result<Vec<ManualImportedRom>, String> {
    let candidates = match source_kind {
        ImportSourceKind::Zip => list_zip_rom_candidates(source_path)?,
        ImportSourceKind::Rar => list_rar_rom_candidates(source_path)?,
        ImportSourceKind::Rom => Vec::new(),
    };

    if candidates.is_empty() {
        return Err("Archive does not contain a supported ROM file.".to_string());
    }

    assert_targets_available(target_directory, &candidates, overwrite)?;

    let temp_directory = create_import_temp_directory(paths)?;
    let result = (|| {
        match source_kind {
            ImportSourceKind::Zip => extract_zip_to_dir(source_path, &temp_directory)?,
            ImportSourceKind::Rar => extract_rar_to_dir(source_path, &temp_directory)?,
            ImportSourceKind::Rom => {}
        }

        let extracted_roms = collect_rom_files(&temp_directory)?;
        if extracted_roms.is_empty() {
            return Err("Archive extraction finished, but no supported ROM was found.".to_string());
        }

        move_extracted_roms(extracted_roms, target_directory, overwrite)
    })();

    let _ = fs::remove_dir_all(&temp_directory);
    result
}

fn list_zip_rom_candidates(archive_path: &Path) -> Result<Vec<String>, String> {
    let file = fs::File::open(archive_path)
        .map_err(|error| format!("Unable to open zip archive: {}", error))?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| format!("Invalid zip archive: {}", error))?;
    let mut candidates = Vec::new();

    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|error| format!("Unable to read zip entry: {}", error))?;

        if entry.is_dir() {
            continue;
        }

        let enclosed = entry
            .enclosed_name()
            .map(|path| path.to_path_buf())
            .ok_or_else(|| "Zip archive contains an unsafe path.".to_string())?;

        if is_supported_manual_import_rom_file(&enclosed) {
            candidates.push(file_name_for_target(&enclosed)?);
        }
    }

    Ok(candidates)
}

fn list_rar_rom_candidates(archive_path: &Path) -> Result<Vec<String>, String> {
    let archive = Archive::new(archive_path)
        .open_for_listing()
        .map_err(|error| format!("Unable to open rar archive: {}", error))?;
    let mut candidates = Vec::new();

    for entry_result in archive {
        let entry = entry_result.map_err(|error| format!("Unable to read rar entry: {}", error))?;
        if entry.is_file() && is_supported_manual_import_rom_file(&entry.filename) {
            candidates.push(file_name_for_target(&entry.filename)?);
        }
    }

    Ok(candidates)
}

fn extract_zip_to_dir(archive_path: &Path, destination: &Path) -> Result<(), String> {
    let file = fs::File::open(archive_path)
        .map_err(|error| format!("Unable to open zip archive: {}", error))?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| format!("Invalid zip archive: {}", error))?;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("Unable to read zip entry: {}", error))?;

        let enclosed = entry
            .enclosed_name()
            .map(|path| path.to_path_buf())
            .ok_or_else(|| "Zip archive contains an unsafe path.".to_string())?;
        let out_path = destination.join(enclosed);

        if entry.is_dir() {
            fs::create_dir_all(&out_path)
                .map_err(|error| format!("Unable to create extracted folder: {}", error))?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Unable to create extracted parent folder: {}", error))?;
        }

        let mut output = fs::File::create(&out_path)
            .map_err(|error| format!("Unable to create extracted file: {}", error))?;
        io::copy(&mut entry, &mut output)
            .map_err(|error| format!("Unable to extract zip file: {}", error))?;
    }

    Ok(())
}

fn extract_rar_to_dir(archive_path: &Path, destination: &Path) -> Result<(), String> {
    let mut archive = Archive::new(archive_path)
        .open_for_processing()
        .map_err(|error| format!("Unable to open rar archive: {}", error))?;

    while let Some(header) = archive
        .read_header()
        .map_err(|error| format!("Unable to read rar entry: {}", error))?
    {
        archive = if header.entry().is_file()
            && is_supported_manual_import_rom_file(&header.entry().filename)
        {
            let file_name = file_name_for_target(&header.entry().filename)?;
            let output_path = destination.join(file_name);
            header
                .extract_to(&output_path)
                .map_err(|error| format!("Unable to extract rar file: {}", error))?
        } else {
            header
                .skip()
                .map_err(|error| format!("Unable to skip rar directory: {}", error))?
        };
    }

    Ok(())
}

fn move_extracted_roms(
    extracted_roms: Vec<PathBuf>,
    target_directory: &Path,
    overwrite: bool,
) -> Result<Vec<ManualImportedRom>, String> {
    let mut imported_roms = Vec::new();
    let mut seen = HashSet::new();

    for rom_path in extracted_roms {
        let file_name = file_name_for_target(&rom_path)?;
        let dedupe_key = file_name.to_ascii_lowercase();
        if !seen.insert(dedupe_key) {
            return Err(format!(
                "Archive contains multiple ROMs named {}. Rename one before importing.",
                file_name
            ));
        }

        let destination = target_directory.join(&file_name);
        let file_size_bytes = move_file_to_destination(&rom_path, &destination, overwrite)?;

        imported_roms.push(ManualImportedRom {
            file_name,
            file_path: destination.to_string_lossy().to_string(),
            file_size_bytes,
        });
    }

    Ok(imported_roms)
}

fn assert_targets_available(
    target_directory: &Path,
    file_names: &[String],
    overwrite: bool,
) -> Result<(), String> {
    let mut seen = HashSet::new();
    for file_name in file_names {
        let dedupe_key = file_name.to_ascii_lowercase();
        if !seen.insert(dedupe_key) {
            return Err(format!(
                "Archive contains multiple ROMs named {}. Rename one before importing.",
                file_name
            ));
        }
    }

    let conflicts = file_names
        .iter()
        .map(|file_name| target_directory.join(file_name))
        .filter(|path| path.exists())
        .collect::<Vec<_>>();

    if conflicts.is_empty() || overwrite {
        return Ok(());
    }

    Err(format!(
        "{}{}",
        DUPLICATE_IMPORT_ERROR_PREFIX,
        conflicts
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("\n")
    ))
}

fn create_import_temp_directory(paths: &PortablePaths) -> Result<PathBuf, String> {
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("System clock error: {}", error))?
        .as_millis();
    let temp_directory = PathBuf::from(&paths.data)
        .join("manual-import")
        .join(timestamp_ms.to_string());

    fs::create_dir_all(&temp_directory)
        .map_err(|error| format!("Unable to create temporary import folder: {}", error))?;

    Ok(temp_directory)
}

fn collect_rom_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut roms = Vec::new();
    collect_rom_files_inner(root, &mut roms)?;
    roms.sort_by(|left, right| {
        left.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase()
            .cmp(
                &right
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_ascii_lowercase(),
            )
    });
    Ok(roms)
}

fn collect_rom_files_inner(dir: &Path, output: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry_result in fs::read_dir(dir)
        .map_err(|error| format!("Unable to read extracted archive folder: {}", error))?
    {
        let entry =
            entry_result.map_err(|error| format!("Unable to read extracted entry: {}", error))?;
        let path = entry.path();

        if path.is_dir() {
            collect_rom_files_inner(&path, output)?;
            continue;
        }

        if is_supported_manual_import_rom_file(&path) {
            output.push(path);
        }
    }

    Ok(())
}

fn is_supported_manual_import_rom_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .as_deref(),
        Some("iso")
            | Some("rvz")
            | Some("wbfs")
            | Some("gcz")
            | Some("ciso")
            | Some("nds")
            | Some("3ds")
            | Some("cci")
            | Some("cia")
            | Some("3dsx")
            | Some("xci")
            | Some("nsp")
            | Some("nro")
            | Some("cue")
            | Some("bin")
            | Some("img")
            | Some("chd")
    )
}

fn move_file_to_destination(
    source: &Path,
    destination: &Path,
    overwrite: bool,
) -> Result<u64, String> {
    if destination.exists() {
        if !overwrite {
            return Err(format!(
                "{}{}",
                DUPLICATE_IMPORT_ERROR_PREFIX,
                destination.to_string_lossy()
            ));
        }

        if same_file(source, destination) {
            return fs::metadata(destination)
                .map(|metadata| metadata.len())
                .map_err(|error| format!("Unable to read existing ROM metadata: {}", error));
        }

        fs::remove_file(destination)
            .map_err(|error| format!("Unable to replace existing ROM: {}", error))?;
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Unable to create ROM target folder: {}", error))?;
    }

    match fs::rename(source, destination) {
        Ok(()) => {}
        Err(rename_error) => {
            fs::copy(source, destination).map_err(|copy_error| {
                format!(
                    "Unable to copy ROM to target folder: {} (rename failed: {})",
                    copy_error, rename_error
                )
            })?;
            fs::remove_file(source).map_err(|remove_error| {
                format!(
                    "ROM was copied, but the original could not be removed: {}",
                    remove_error
                )
            })?;
        }
    }

    fs::metadata(destination)
        .map(|metadata| metadata.len())
        .map_err(|error| format!("Unable to read imported ROM metadata: {}", error))
}

fn same_file(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn file_name_for_target(path: &Path) -> Result<String, String> {
    let raw = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "Invalid file name.".to_string())?;
    let sanitized = sanitize_file_name(raw);

    if sanitized.is_empty() {
        return Err("Invalid empty file name.".to_string());
    }

    Ok(sanitized)
}

fn sanitize_file_name(input: &str) -> String {
    input
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => character,
        })
        .collect::<String>()
        .trim_matches('.')
        .trim()
        .to_string()
}
