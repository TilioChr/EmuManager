import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import LibraryPanel from "./components/LibraryPanel";
import RommConnectionCard from "./components/RommConnectionCard";
import CollapsiblePanel from "./components/CollapsiblePanel";
import {
  resolveGameDownloadUrl,
  resolveGameLocalFileName,
  resolveGameRomSubdir,
  type RommLaunchSession,
  type RommGame,
  type RommSession
} from "./lib/romm";
import { buildPortablePaths } from "./lib/portableConfig";
import type {
  AppConfig,
  ConfigureResult,
  DownloadResult,
  EmulatorEntry,
  GameLaunchResult,
  InstallResult,
  LaunchResult,
  LocalRomEntry,
  LocalSaveEntry,
  PortablePaths,
  SaveSyncStatus
} from "./types";

const fallbackPaths = buildPortablePaths(".");

const initialPaths: PortablePaths = {
  ...fallbackPaths,
  config: `${fallbackPaths.root}\\Config`,
  data: `${fallbackPaths.root}\\Data`
};

interface DownloadProgressPayload {
  downloadId: string;
  fileName: string;
  downloadedBytes: number;
  totalBytes?: number;
  percent: number;
}

interface InstalledVersionMap {
  versions: Record<string, string>;
}

export default function App() {
  const [paths, setPaths] = useState<PortablePaths>(initialPaths);
  const [emulators, setEmulators] = useState<EmulatorEntry[]>([]);
  const [localRoms, setLocalRoms] = useState<LocalRomEntry[]>([]);
  const [localSaves, setLocalSaves] = useState<LocalSaveEntry[]>([]);
  const [saveSyncStatuses, setSaveSyncStatuses] = useState<Record<string, SaveSyncStatus>>({});
  const [selectedEmulatorId, setSelectedEmulatorId] = useState<string | null>(null);
  const [showPicker, setShowPicker] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [generalMessage, setGeneralMessage] = useState<string | null>(null);
  const [libraryNotice, setLibraryNotice] = useState<{
    type: "success" | "error" | "info";
    message: string;
  } | null>(null);

  const [config, setConfig] = useState<AppConfig>({ installedEmulators: [] });
  const [rommSession, setRommSession] = useState<RommSession | null>(null);
  const [installingId, setInstallingId] = useState<string | null>(null);
  const [launchingId, setLaunchingId] = useState<string | null>(null);
  const [configuringId, setConfiguringId] = useState<string | null>(null);
  const [downloadingGameId, setDownloadingGameId] = useState<string | null>(null);
  const [downloadPercent, setDownloadPercent] = useState(0);

  const refreshSaveSyncStatuses = async (root: string, roms: LocalRomEntry[]) => {
    const statuses = await invoke<SaveSyncStatus[]>("get_save_sync_statuses_command", {
      root,
      romPaths: roms.map((entry) => entry.filePath)
    });

    setSaveSyncStatuses(
      Object.fromEntries(statuses.map((entry) => [entry.romPath, entry]))
    );
  };

  const refreshLocalRoms = async (root: string) => {
    const roms = await invoke<LocalRomEntry[]>("list_local_roms_command", { root });
    setLocalRoms(roms);
    await refreshSaveSyncStatuses(root, roms);
    return roms;
  };

  const refreshLocalSaves = async (root: string) => {
    const saves = await invoke<LocalSaveEntry[]>("list_local_saves_command", { root });
    setLocalSaves(saves);
  };

  const refreshInstalledVersions = async (
    root: string,
    currentEmulators?: Array<Omit<EmulatorEntry, "status">>,
    installedIds?: string[]
  ) => {
    const versionMap = await invoke<InstalledVersionMap>("get_installed_emulator_versions", {
      root
    });

    setEmulators((previous) => {
      const source =
        currentEmulators?.length
          ? currentEmulators.map((emu) => ({
              ...emu,
              status: installedIds?.includes(emu.id) ? ("installed" as const) : ("not_installed" as const)
            }))
          : previous;

      return source.map((emu) => ({
        ...emu,
        version: versionMap.versions[emu.id] ?? emu.catalogVersion ?? emu.version
      }));
    });
  };

  useEffect(() => {
    const bootstrap = async () => {
      try {
        const portablePaths = await invoke<PortablePaths>("init_portable_layout");
        setPaths(portablePaths);

        const savedConfig = await invoke<AppConfig>("load_app_config", {
          root: portablePaths.root
        });
        setConfig(savedConfig);

        await refreshLocalRoms(portablePaths.root);
        await refreshLocalSaves(portablePaths.root);

        const builtin = await invoke<Array<Omit<EmulatorEntry, "status">>>(
          "get_builtin_emulators"
        );

        const installedIds: string[] = [];
        for (const emu of builtin) {
          const isInstalled = await invoke<boolean>("check_emulator_installed", {
            root: portablePaths.root,
            emulatorId: emu.id
          });

          if (isInstalled) {
            installedIds.push(emu.id);
          }
        }

        const mergedInstalledIds = Array.from(
          new Set([...savedConfig.installedEmulators, ...installedIds])
        );

        const nextConfig: AppConfig = {
          ...savedConfig,
          installedEmulators: mergedInstalledIds
        };

        if (mergedInstalledIds.length !== savedConfig.installedEmulators.length) {
          await invoke("save_app_config", {
            root: portablePaths.root,
            config: nextConfig
          });
        }

        setConfig(nextConfig);

        const nextEmulators = builtin.map((emu) => ({
          ...emu,
          status: mergedInstalledIds.includes(emu.id) ? ("installed" as const) : ("not_installed" as const),
          version: emu.catalogVersion
        }));

        setEmulators(nextEmulators);

        await refreshInstalledVersions(portablePaths.root, builtin, mergedInstalledIds);

        const firstInstalled = nextEmulators.find((entry) => entry.status === "installed");
        setSelectedEmulatorId(firstInstalled?.id ?? null);
      } catch (reason) {
        setError(reason instanceof Error ? reason.message : String(reason));
      } finally {
        setLoading(false);
      }
    };

    void bootstrap();
  }, []);

  useEffect(() => {
    let unlistenProgress: UnlistenFn | null = null;
    let unlistenComplete: UnlistenFn | null = null;

    const setupListeners = async () => {
      unlistenProgress = await listen<DownloadProgressPayload>("rom-download-progress", (event) => {
        const payload = event.payload;
        setDownloadPercent(payload.percent ?? 0);
      });

      unlistenComplete = await listen<DownloadProgressPayload>("rom-download-complete", () => {
        setDownloadPercent(100);
      });
    };

    void setupListeners();

    return () => {
      if (unlistenProgress) {
        unlistenProgress();
      }
      if (unlistenComplete) {
        unlistenComplete();
      }
    };
  }, []);

  const installedCount = useMemo(
    () => emulators.filter((emu) => emu.status === "installed").length,
    [emulators]
  );

  const selectedEmulator =
    emulators.find((entry) => entry.id === selectedEmulatorId) ??
    emulators.find((entry) => entry.status === "installed") ??
    null;

  const persistConfig = async (nextConfig: AppConfig) => {
    await invoke("save_app_config", {
      root: paths.root,
      config: nextConfig
    });
    setConfig(nextConfig);
  };

  const removeInstalledFlag = async (id: string) => {
    const nextInstalledIds = config.installedEmulators.filter((entry) => entry !== id);
    const nextConfig: AppConfig = {
      ...config,
      installedEmulators: nextInstalledIds
    };

    await persistConfig(nextConfig);

    const nextEmulators = emulators.map((emu) =>
      emu.id === id
        ? {
            ...emu,
            status: "not_installed" as const
          }
        : emu
    );

    setEmulators(nextEmulators);

    if (selectedEmulatorId === id) {
      const replacement = nextEmulators.find((entry) => entry.status === "installed");
      setSelectedEmulatorId(replacement?.id ?? null);
    }

    setGeneralMessage(`Removed ${id} from the installed list. Existing files were not deleted.`);
  };

  const configureSelectedEmulator = async (id: string) => {
    try {
      setConfiguringId(id);
      const result = await invoke<ConfigureResult>("configure_emulator_command", {
        root: paths.root,
        emulatorId: id
      });
      setGeneralMessage(`Configuration reapplied: ${result.userDirectory}`);
      await refreshInstalledVersions(paths.root);
    } catch (reason) {
      setGeneralMessage(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setConfiguringId(null);
    }
  };

  const installSelectedEmulator = async (id: string) => {
    try {
      setInstallingId(id);
      setGeneralMessage(`Downloading and installing ${id}...`);

      const result = await invoke<InstallResult>("install_emulator_command", {
        root: paths.root,
        emulatorId: id
      });

      const nextInstalledIds = Array.from(new Set([...config.installedEmulators, id]));
      const nextConfig: AppConfig = {
        ...config,
        installedEmulators: nextInstalledIds
      };

      await persistConfig(nextConfig);

      const nextEmulators = emulators.map((emu) =>
        emu.id === id
          ? {
              ...emu,
              status: "installed" as const
            }
          : emu
      );

      setEmulators(nextEmulators);
      setSelectedEmulatorId(id);

      try {
        const configResult = await invoke<ConfigureResult>("configure_emulator_command", {
          root: paths.root,
          emulatorId: id
        });

        setGeneralMessage(
          `${id} installed in ${result.installPath} and configured in ${configResult.userDirectory}`
        );
      } catch {
        setGeneralMessage(`${id} installed in ${result.installPath}`);
      }

      await refreshInstalledVersions(paths.root);
    } catch (reason) {
      setGeneralMessage(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setInstallingId(null);
    }
  };

  const launchSelectedEmulator = async (id: string) => {
    try {
      setLaunchingId(id);
      const result = await invoke<LaunchResult>("launch_emulator_command", {
        root: paths.root,
        emulatorId: id
      });
      setGeneralMessage(`Emulator launched from ${result.executablePath}`);
      await refreshInstalledVersions(paths.root);
    } catch (reason) {
      setGeneralMessage(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setLaunchingId(null);
    }
  };

  const handleRommConnected = async (session: RommSession, username: string) => {
    setRommSession(session);

    const nextConfig: AppConfig = {
      ...config,
      romm: {
        baseUrl: session.baseUrl,
        username
      }
    };

    await persistConfig(nextConfig);
    setLibraryNotice({
      type: "success",
      message: `Connected to RomM as ${username}.`
    });
  };

  const handleDownloadGame = async (game: RommGame) => {
    if (!rommSession) {
      setLibraryNotice({
        type: "error",
        message: "RomM connection required."
      });
      return;
    }

    const downloadUrl = resolveGameDownloadUrl(rommSession, game);

    if (!downloadUrl) {
      setLibraryNotice({
        type: "error",
        message: `Unable to resolve the download URL for "${game.name}".`
      });
      return;
    }

    const targetFileName = resolveGameLocalFileName(game);
    const downloadId = String(game.id);
    const relativeSubdir = resolveGameRomSubdir(game);

    try {
      setDownloadingGameId(downloadId);
      setDownloadPercent(0);
      setLibraryNotice({
        type: "info",
        message: `Downloading "${game.name}" to Roms/${relativeSubdir}...`
      });

      const result = await invoke<DownloadResult>("download_rom_command", {
        root: paths.root,
        url: downloadUrl,
        fileName: targetFileName,
        bearerToken: rommSession.token,
        downloadId,
        expectedTotalBytes:
          typeof game.fs_size_bytes === "number" ? game.fs_size_bytes : undefined,
        relativeSubdir
      });

      await invoke("register_romm_rom_command", {
        root: paths.root,
        romPath: result.filePath,
        rommId: String(game.id),
        platformName: game.platform_name ?? game.platform_display_name ?? null,
        fileName: targetFileName
      });

      await refreshLocalRoms(paths.root);
      await refreshLocalSaves(paths.root);

      setLibraryNotice({
        type: "success",
        message: `ROM downloaded to ${result.filePath}`
      });
    } catch (reason) {
      setLibraryNotice({
        type: "error",
        message: reason instanceof Error ? reason.message : String(reason)
      });
    } finally {
      setDownloadingGameId(null);
      setTimeout(() => setDownloadPercent(0), 500);
    }
  };

  const launchSpecificRom = async (romPath: string) => {
    try {
      const result = await invoke<GameLaunchResult>("launch_game_auto_command", {
        root: paths.root,
        romPath,
        rommSession: rommSession
          ? ({
              baseUrl: rommSession.baseUrl,
              token: rommSession.token
            } satisfies RommLaunchSession)
          : null
      });
      setGeneralMessage(`Session ${result.emulatorId} terminée et synchronisée pour ${result.romPath}`);
      await refreshLocalRoms(paths.root);
      await refreshLocalSaves(paths.root);
      await refreshInstalledVersions(paths.root);
    } catch (reason) {
      setGeneralMessage(reason instanceof Error ? reason.message : String(reason));
    }
  };

  if (loading) {
    return (
      <div className="center-screen">
        <div className="panel loading-panel">
          <h2 className="panel-title">Initialization</h2>
          <p className="panel-subtitle">Preparing portable environment</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="center-screen">
        <div className="panel loading-panel">
          <h2 className="panel-title">Error</h2>
          <p className="panel-subtitle">Failed to initialize EmuManager</p>
          <p>{error}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div>
          <h2 className="sidebar-title">EmuManager</h2>
          <p className="muted">{installedCount} installed</p>
        </div>

        <button className="primary-button" onClick={() => setShowPicker(true)}>
          Emulators
        </button>

        <nav className="emulator-list">
          {emulators
            .filter((emu) => emu.status === "installed")
            .map((emu) => (
              <button
                key={emu.id}
                className={`emulator-item ${selectedEmulatorId === emu.id ? "emulator-item-active" : ""}`}
                onClick={() => setSelectedEmulatorId(emu.id)}
              >
                <span>{emu.name}</span>
                <small>{emu.platformLabel}</small>
              </button>
            ))}

          {installedCount === 0 ? (
            <div className="empty-state">
              <p>No emulator installed</p>
            </div>
          ) : null}
        </nav>
      </aside>

      <main className="content">
        <CollapsiblePanel eyebrow="Selected Emulator" title={selectedEmulator?.name}>
          {selectedEmulator ? (
            <div className="selected-emulator-grid">
              <StatusCard label="Platform" value={selectedEmulator.platformLabel} />
              <StatusCard
                label="Version"
                value={selectedEmulator.version ?? selectedEmulator.catalogVersion ?? "Unknown"}
              />
            </div>
          ) : null}

          <div className="selected-actions">
            <button
              className="primary-button"
              disabled={!selectedEmulator || launchingId === selectedEmulator.id}
              onClick={() => selectedEmulator && void launchSelectedEmulator(selectedEmulator.id)}
            >
              {selectedEmulator && launchingId === selectedEmulator.id
                ? "Launching..."
                : "Open emulator"}
            </button>
          </div>

          {generalMessage ? (
            <div className="inline-notice inline-notice-info top-gap">{generalMessage}</div>
          ) : null}
        </CollapsiblePanel>

        <RommConnectionCard
          defaultBaseUrl={config.romm?.baseUrl}
          defaultUsername={config.romm?.username}
          onConnected={handleRommConnected}
        />

        <LibraryPanel
          session={rommSession}
          localRoms={localRoms}
          saveSyncStatuses={saveSyncStatuses}
          onDownloadGame={handleDownloadGame}
          onLaunchLocalRom={launchSpecificRom}
          downloadingGameId={downloadingGameId}
          downloadPercent={downloadPercent}
          notice={libraryNotice}
        />
      </main>

      {showPicker ? (
        <div className="modal-backdrop" onClick={() => setShowPicker(false)}>
          <div className="modal" onClick={(event) => event.stopPropagation()}>
            <div className="modal-header">
              <div>
                <h2 className="panel-title">Choose emulators</h2>
                <p className="panel-subtitle">Installation</p>
              </div>
              <button className="ghost-button" onClick={() => setShowPicker(false)}>
                Close
              </button>
            </div>

            <div className="picker-list">
              {emulators.map((emu) => {
                const isInstalling = installingId === emu.id;
                const isInstalled = emu.status === "installed";

                return (
                  <div key={emu.id} className="picker-item">
                    <div>
                      <strong>{emu.name}</strong>
                      <p>{emu.platformLabel}</p>
                    </div>
                    <button
                      className="primary-button"
                      disabled={isInstalling}
                      onClick={() =>
                        void (isInstalled ? removeInstalledFlag(emu.id) : installSelectedEmulator(emu.id))
                      }
                    >
                      {isInstalling
                        ? "Installing..."
                        : isInstalled
                          ? "Remove"
                          : "Install"}
                    </button>
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}

interface StatusCardProps {
  label: string;
  value: string;
}

function StatusCard({ label, value }: StatusCardProps) {
  return (
    <div className="path-card">
      <small>{label}</small>
      <strong>{value}</strong>
    </div>
  );
}
