import type { LibraryPaths } from "../types";

export interface PortableConfig {
  paths: LibraryPaths;
  romm?: {
    baseUrl: string;
    username: string;
  };
}

export function buildPortablePaths(root: string): LibraryPaths {
  const cleanRoot = root.replace(/[\\/]+$/, "");

  return {
    root: cleanRoot,
    emu: `${cleanRoot}\\Emu`,
    roms: `${cleanRoot}\\Roms`,
    saves: `${cleanRoot}\\Saves`,
    firmware: `${cleanRoot}\\Firmware`
  };
}

export const defaultPortableConfig: PortableConfig = {
  paths: buildPortablePaths("C:\\Users\\Tilio\\Documents\\EmuManager")
};