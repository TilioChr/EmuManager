export type EmulatorStatus = "installed" | "not_installed" | "updating";

export interface EmulatorEntry {
  id: string;
  name: string;
  platformLabel: string;
  catalogVersion?: string;
  status: EmulatorStatus;
  version?: string;
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
}

export interface InstallResult {
  emulatorId: string;
  installPath: string;
  executablePath: string;
  archivePath: string;
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

export interface GameLaunchResult {
  emulatorId: string;
  executablePath: string;
  romPath: string;
  launched: boolean;
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
  physicalDeviceLabel: string;
  emulatedDeviceLabel: string;
  bindings: ControllerBinding[];
}

export interface ControllerWriteResult {
  emulatorId: string;
  profileId: string;
  profilePath: string;
  gameIniPath: string;
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