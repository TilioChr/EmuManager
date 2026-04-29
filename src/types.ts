export type EmulatorStatus = "installed" | "not_installed" | "updating";

export interface EmulatorEntry {
  id: string;
  name: string;
  platformLabel: string;
  catalogVersion?: string;
  status: EmulatorStatus;
  version?: string;
}

export interface EmulatorResourceRequirement {
  id: string;
  label: string;
  kind: string;
  required: boolean;
  installHint: string;
}

export interface EmulatorResourceStatus {
  id: string;
  label: string;
  kind: string;
  required: boolean;
  state: "valid" | "missing" | "invalid";
  installedPath?: string | null;
  message: string;
  fileCount: number;
}

export interface EmulatorResourceSummary {
  emulatorId: string;
  emulatorName: string;
  requirements: EmulatorResourceRequirement[];
  statuses: EmulatorResourceStatus[];
  ready: boolean;
}

export interface ResourceInstallResult {
  emulatorId: string;
  installed: Array<{
    resourceId: string;
    resourceLabel: string;
    sourceFileName: string;
    destinationPath: string;
    verifiedByRomm: boolean;
  }>;
  summary: EmulatorResourceSummary;
}

export interface LibraryPaths {
  root: string;
  emu: string;
  roms: string;
  saves: string;
  firmware: string;
}

export interface PortablePaths extends LibraryPaths {
  config: string;
  data: string;
}

export interface AppConfig {
  romm?: {
    baseUrl: string;
    username: string;
  };
  installedEmulators: string[];
  skippedAppUpdateVersion?: string | null;
  pinnedLibraryItems?: string[];
}

export interface InstallResult {
  emulatorId: string;
  installPath: string;
  executablePath: string;
  archivePath: string;
}

export interface UninstallResult {
  emulatorId: string;
  installPath: string;
  removed: boolean;
}

export interface ConfigureResult {
  emulatorId: string;
  portableFilePath: string;
  userDirectory: string;
  configDirectory: string;
  gcSavesDirectory: string;
  wiiSavesDirectory: string;
}

export interface LaunchResult {
  emulatorId: string;
  executablePath: string;
  workingDirectory: string;
  launched: boolean;
}

export interface DownloadResult {
  filePath: string;
  fileName: string;
  bytesWritten: number;
}

export interface AppUpdateStatus {
  currentVersion: string;
  latestVersion?: string | null;
  updateAvailable: boolean;
  releaseName?: string | null;
  releaseUrl?: string | null;
  publishedAt?: string | null;
  assetName?: string | null;
  assetSize?: number | null;
  downloadUrl?: string | null;
}

export interface AppUpdateDownloadResult {
  version: string;
  assetName: string;
  filePath: string;
  bytesWritten: number;
}

export interface DeleteLocalRomResult {
  fileName: string;
  filePath: string;
}

export interface ManualImportPlatform {
  id: string;
  label: string;
  folder: string;
}

export interface ManualImportedRom {
  fileName: string;
  filePath: string;
  fileSizeBytes: number;
}

export interface ManualImportResult {
  platformId: string;
  platformLabel: string;
  targetDirectory: string;
  sourceKind: "rom" | "zip" | "rar";
  importedRoms: ManualImportedRom[];
}

export interface GameLaunchResult {
  emulatorId: string;
  executablePath: string;
  romPath: string;
  launched: boolean;
}

export interface SaveSyncStatus {
  romPath: string;
  rommId?: string | null;
  emulatorId: string;
  hasLocalSave: boolean;
  localSaveUpdatedAtMs?: number | null;
  lastKnownRemoteSaveAt?: string | null;
}

export type SaveConflictResolution = "useLocal" | "useRomm";

export interface SaveConflictStatus {
  romPath: string;
  rommId: string;
  emulatorId: string;
  slotName: string;
  localSaveUpdatedAtMs: number;
  lastSyncedLocalSaveAtMs?: number | null;
  remoteSaveUpdatedAt?: string | null;
  lastKnownRemoteSaveAt?: string | null;
  remoteSaveFileName?: string | null;
}

export interface ControllerBinding {
  physicalInput: string;
  emulatedInput: string;
}

export interface ControllerProfile {
  id: string;
  name: string;
  emulatorId: string;
  platformLabel: string;
  physicalDeviceId?: string | null;
  physicalDeviceLabel: string;
  emulatedControllerId?: string | null;
  emulatedDeviceLabel: string;
  dolphinSettings?: ControllerDolphinSettings | null;
  bindings: ControllerBinding[];
}

export interface ControllerDolphinSettings {
  irAutoHide: boolean;
  irRelativeInput: boolean;
}

export interface ControllerWriteResult {
  emulatorId: string;
  profileId: string;
  profilePath: string;
  gameIniPath: string;
}

export interface ControllerProfileSaveResult {
  profiles: ControllerProfile[];
  writeResult?: ControllerWriteResult | null;
  warning?: string | null;
}

export interface LocalRomEntry {
  fileName: string;
  filePath: string;
  fileSizeBytes: number;
  platformGuess: string;
}

export interface LocalSaveEntry {
  fileName: string;
  filePath: string;
  fileSizeBytes: number;
  platformGuess: string;
}
