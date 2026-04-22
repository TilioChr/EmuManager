export type EmulatorStatus = "installed" | "not_installed" | "updating";

export interface EmulatorEntry {
  id: string;
  name: string;
  platformLabel: string;
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