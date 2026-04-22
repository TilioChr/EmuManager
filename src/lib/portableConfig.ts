export interface PortablePathsShape {
  root: string;
  emu: string;
  roms: string;
  saves: string;
  firmware: string;
}

export interface PortableConfig {
  paths: PortablePathsShape;
}

export function buildPortablePaths(root: string): PortablePathsShape {
  const normalizedRoot = root.replace(/[\\/]+$/, "");

  return {
    root: normalizedRoot,
    emu: `${normalizedRoot}\\Emu`,
    roms: `${normalizedRoot}\\Roms`,
    saves: `${normalizedRoot}\\Saves`,
    firmware: `${normalizedRoot}\\Firmware`
  };
}