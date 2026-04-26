import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import LibraryPanel from "./components/LibraryPanel";
import RommConnectionCard from "./components/RommConnectionCard";
import ControllerMappingPanel from "./components/ControllerMappingPanel";
import NotificationOverlay, {
  type NotificationEntry,
  type NotificationKind
} from "./components/NotificationOverlay";
import {
  debugLog,
  recordDebugLogEntry,
  type DebugLogEntry
} from "./lib/debugLog";
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
  ControllerProfile,
  ControllerProfileSaveResult,
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
const DEBUG_WINDOW_LABEL = "debug-logs";
const LAUNCH_STARTUP_LOCK_MS = 1800;
const NOTIFICATION_TTL_MS = 5000;
const NOTIFICATION_EXIT_MS = 220;
const MAX_NOTIFICATION_HISTORY = 80;

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

interface EmulatorInstallProgressPayload {
  emulatorId: string;
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
  const [controllerProfiles, setControllerProfiles] = useState<ControllerProfile[]>([]);
  const [saveSyncStatuses, setSaveSyncStatuses] = useState<Record<string, SaveSyncStatus>>({});
  const [selectedEmulatorId, setSelectedEmulatorId] = useState<string | null>(null);
  const [showPicker, setShowPicker] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [libraryNotice, setLibraryNotice] = useState<{
    type: "success" | "error" | "info";
    message: string;
  } | null>(null);
  const [notifications, setNotifications] = useState<NotificationEntry[]>([]);
  const [notificationHistory, setNotificationHistory] = useState<NotificationEntry[]>([]);
  const [notificationHistoryOpen, setNotificationHistoryOpen] = useState(false);
  const notificationIdRef = useRef(0);
  const notificationTimersRef = useRef<number[]>([]);

  const [config, setConfig] = useState<AppConfig>({ installedEmulators: [] });
  const configRef = useRef(config);
  const [rommSession, setRommSession] = useState<RommSession | null>(null);
  const [installProgressById, setInstallProgressById] = useState<Record<string, number>>({});
  const installProgressRef = useRef(installProgressById);
  const [launchingId, setLaunchingId] = useState<string | null>(null);
  const launchingIdRef = useRef<string | null>(launchingId);
  const [runningRomPaths, setRunningRomPaths] = useState<Record<string, boolean>>({});
  const runningRomPathsRef = useRef(runningRomPaths);
  const [downloadProgressById, setDownloadProgressById] = useState<Record<string, number>>({});
  const romProgressLogStepRef = useRef<Record<string, number>>({});
  const installProgressLogStepRef = useRef<Record<string, number>>({});

  useEffect(() => {
    configRef.current = config;
  }, [config]);

  useEffect(() => {
    installProgressRef.current = installProgressById;
  }, [installProgressById]);

  useEffect(() => {
    runningRomPathsRef.current = runningRomPaths;
  }, [runningRomPaths]);

  useEffect(() => {
    launchingIdRef.current = launchingId;
  }, [launchingId]);

  useEffect(() => {
    return () => {
      for (const timerId of notificationTimersRef.current) {
        window.clearTimeout(timerId);
      }
    };
  }, []);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "F10") {
        event.preventDefault();
        void openDebugWindow();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

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
        void debugLog("info", "app", "Application bootstrap started");
        const portablePaths = await invoke<PortablePaths>("init_portable_layout");
        setPaths(portablePaths);
        void debugLog("debug", "paths", "Portable layout initialized", portablePaths);

        const savedConfig = await invoke<AppConfig>("load_app_config", {
          root: portablePaths.root
        });
        setConfig(savedConfig);
        configRef.current = savedConfig;

        await refreshLocalRoms(portablePaths.root);
        await refreshLocalSaves(portablePaths.root);
        const savedControllerProfiles = await invoke<ControllerProfile[]>(
          "load_controller_profiles_command",
          {
            root: portablePaths.root
          }
        );
        setControllerProfiles(savedControllerProfiles);

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
        void debugLog("success", "app", "Application bootstrap completed", {
          installedEmulators: mergedInstalledIds,
          selectedEmulatorId: firstInstalled?.id ?? null
        });
      } catch (reason) {
        const message = reason instanceof Error ? reason.message : String(reason);
        setError(message);
        void debugLog("error", "app", "Application bootstrap failed", message);
      } finally {
        setLoading(false);
      }
    };

    void bootstrap();
  }, []);

  useEffect(() => {
    let unlistenRomProgress: UnlistenFn | null = null;
    let unlistenRomComplete: UnlistenFn | null = null;
    let unlistenInstallProgress: UnlistenFn | null = null;
    let unlistenInstallComplete: UnlistenFn | null = null;
    let unlistenDebugEntry: UnlistenFn | null = null;

    const setupListeners = async () => {
      unlistenRomProgress = await listen<DownloadProgressPayload>("rom-download-progress", (event) => {
        const payload = event.payload;
        const percent = normalizeDownloadPercent(payload.percent);
        setDownloadProgressById((previous) => ({
          ...previous,
          [payload.downloadId]: percent
        }));
        logProgressMilestone(
          romProgressLogStepRef,
          payload.downloadId,
          percent,
          "rom-download",
          `Downloading ROM "${payload.fileName}"`,
          {
            downloadId: payload.downloadId,
            downloadedBytes: payload.downloadedBytes,
            totalBytes: payload.totalBytes
          }
        );
      });

      unlistenRomComplete = await listen<DownloadProgressPayload>("rom-download-complete", (event) => {
        const payload = event.payload;
        setDownloadProgressById((previous) => ({
          ...previous,
          [payload.downloadId]: 100
        }));
        void debugLog("success", "rom-download", `ROM download complete: ${payload.fileName}`, {
          downloadId: payload.downloadId,
          downloadedBytes: payload.downloadedBytes,
          totalBytes: payload.totalBytes
        });
      });

      unlistenInstallProgress = await listen<EmulatorInstallProgressPayload>(
        "emulator-install-progress",
        (event) => {
          const payload = event.payload;
          const percent = normalizeDownloadPercent(payload.percent);
          setInstallProgressById((previous) => {
            const next = {
              ...previous,
              [payload.emulatorId]: percent
            };
            installProgressRef.current = next;
            return next;
          });
          logProgressMilestone(
            installProgressLogStepRef,
            payload.emulatorId,
            percent,
            "emulator-install",
            `Downloading emulator archive "${payload.fileName}"`,
            {
              emulatorId: payload.emulatorId,
              downloadedBytes: payload.downloadedBytes,
              totalBytes: payload.totalBytes
            }
          );
        }
      );

      unlistenInstallComplete = await listen<EmulatorInstallProgressPayload>(
        "emulator-install-complete",
        (event) => {
          const payload = event.payload;
          setInstallProgressById((previous) => {
            const next = {
              ...previous,
              [payload.emulatorId]: 100
            };
            installProgressRef.current = next;
            return next;
          });
          void debugLog("success", "emulator-install", `Emulator archive downloaded: ${payload.emulatorId}`, {
            emulatorId: payload.emulatorId,
            fileName: payload.fileName,
            downloadedBytes: payload.downloadedBytes,
            totalBytes: payload.totalBytes
          });
        }
      );

      unlistenDebugEntry = await listen<DebugLogEntry>("debug-log-entry", (event) => {
        if (event.payload.source === "backend") {
          recordDebugLogEntry(event.payload);
        }
      });
    };

    void setupListeners();

    return () => {
      if (unlistenRomProgress) {
        unlistenRomProgress();
      }
      if (unlistenRomComplete) {
        unlistenRomComplete();
      }
      if (unlistenInstallProgress) {
        unlistenInstallProgress();
      }
      if (unlistenInstallComplete) {
        unlistenInstallComplete();
      }
      if (unlistenDebugEntry) {
        unlistenDebugEntry();
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
    configRef.current = nextConfig;
    setConfig(nextConfig);

    await invoke("save_app_config", {
      root: paths.root,
      config: nextConfig
    });
  };

  const saveControllerProfile = async (profile: ControllerProfile) => {
    const result = await invoke<ControllerProfileSaveResult>("save_controller_profile_command", {
      root: paths.root,
      profile
    });

    setControllerProfiles(result.profiles);
    return result;
  };

  const dismissNotification = (id: number) => {
    setNotifications((previous) =>
      previous.map((entry) => (entry.id === id ? { ...entry, exiting: true } : entry))
    );

    const removeTimerId = window.setTimeout(() => {
      setNotifications((previous) => previous.filter((entry) => entry.id !== id));
    }, NOTIFICATION_EXIT_MS);

    notificationTimersRef.current.push(removeTimerId);
  };

  const notify = (kind: NotificationKind, message: string) => {
    const entry: NotificationEntry = {
      id: notificationIdRef.current + 1,
      kind,
      message,
      createdAt: Date.now()
    };

    notificationIdRef.current = entry.id;
    setNotifications((previous) => [...previous.filter((item) => !item.exiting), entry].slice(-5));
    setNotificationHistory((previous) => [entry, ...previous].slice(0, MAX_NOTIFICATION_HISTORY));

    const dismissTimerId = window.setTimeout(() => {
      dismissNotification(entry.id);
    }, NOTIFICATION_TTL_MS - NOTIFICATION_EXIT_MS);

    notificationTimersRef.current.push(dismissTimerId);
  };

  const clearNotificationHistory = () => {
    setNotificationHistory([]);
  };

  const removeInstalledFlag = async (id: string) => {
    void debugLog("info", "emulator", `Removing installed flag for ${id}`);
    const nextInstalledIds = configRef.current.installedEmulators.filter((entry) => entry !== id);
    const nextConfig: AppConfig = {
      ...configRef.current,
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

    notify("info", `Removed ${id} from the installed list. Existing files were not deleted.`);
    void debugLog("success", "emulator", `Removed installed flag for ${id}`, {
      remainingInstalledEmulators: nextInstalledIds
    });
  };

  const clearLaunchingEmulator = (id: string, delayMs = 0) => {
    const clear = () => {
      if (launchingIdRef.current !== id) {
        return;
      }

      launchingIdRef.current = null;
      setLaunchingId((current) => (current === id ? null : current));
    };

    if (delayMs > 0) {
      window.setTimeout(clear, delayMs);
      return;
    }

    clear();
  };

  const clearRunningRomPath = (romPath: string, delayMs = 0) => {
    const clear = () => {
      setRunningRomPaths((previous) => {
        if (!previous[romPath]) {
          return previous;
        }

        const next = { ...previous };
        delete next[romPath];
        runningRomPathsRef.current = next;
        return next;
      });
    };

    if (delayMs > 0) {
      window.setTimeout(clear, delayMs);
      return;
    }

    clear();
  };

  const installSelectedEmulator = async (id: string) => {
    if (id in installProgressRef.current) {
      return;
    }

    try {
      const initialProgress = {
        ...installProgressRef.current,
        [id]: 0
      };
      installProgressRef.current = initialProgress;
      setInstallProgressById(initialProgress);
      notify("info", `Downloading and installing ${id}...`);
      void debugLog("info", "emulator-install", `Installing emulator ${id}`, {
        root: paths.root
      });

      const result = await invoke<InstallResult>("install_emulator_command", {
        root: paths.root,
        emulatorId: id
      });

      const nextInstalledIds = Array.from(new Set([...configRef.current.installedEmulators, id]));
      const nextConfig: AppConfig = {
        ...configRef.current,
        installedEmulators: nextInstalledIds
      };

      await persistConfig(nextConfig);

      setEmulators((previous) =>
        previous.map((emu) =>
          emu.id === id
            ? {
                ...emu,
                status: "installed" as const
              }
            : emu
        )
      );
      setSelectedEmulatorId(id);

      try {
        const configResult = await invoke<ConfigureResult>("configure_emulator_command", {
          root: paths.root,
          emulatorId: id
        });

        notify(
          "success",
          `${id} installed in ${result.installPath} and configured in ${configResult.userDirectory}`
        );
        void debugLog("success", "emulator-install", `Installed and configured ${id}`, {
          install: result,
          configuration: configResult
        });
      } catch {
        notify("warning", `${id} installed in ${result.installPath}, but configuration failed.`);
        void debugLog("warning", "emulator-install", `Installed ${id}, configuration step failed`, result);
      }

      await refreshInstalledVersions(paths.root);
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      notify("error", message);
      void debugLog("error", "emulator-install", `Installation failed for ${id}`, message);
    } finally {
      window.setTimeout(() => {
        setInstallProgressById((previous) => {
          if (!(id in previous)) {
            return previous;
          }

          const next = { ...previous };
          delete next[id];
          installProgressRef.current = next;
          delete installProgressLogStepRef.current[id];
          return next;
        });
      }, 500);
    }
  };

  const launchSelectedEmulator = async (id: string) => {
    if (launchingIdRef.current === id) {
      return;
    }

    let launched = false;

    try {
      launchingIdRef.current = id;
      setLaunchingId(id);
      void debugLog("info", "emulator-launch", `Launching emulator ${id}`);
      const result = await invoke<LaunchResult>("launch_emulator_command", {
        root: paths.root,
        emulatorId: id
      });
      notify("success", `Emulator launched from ${result.executablePath}`);
      void debugLog("success", "emulator-launch", `Emulator launched: ${id}`, result);
      launched = true;
      await refreshInstalledVersions(paths.root);
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      notify("error", message);
      void debugLog("error", "emulator-launch", `Emulator launch failed: ${id}`, message);
    } finally {
      clearLaunchingEmulator(id, launched ? LAUNCH_STARTUP_LOCK_MS : 0);
    }
  };

  const handleRommConnected = async (session: RommSession, username: string) => {
    setRommSession(session);
    void debugLog("success", "romm", `Connected to RomM as ${username}`, {
      baseUrl: session.baseUrl,
      username
    });

    const nextConfig: AppConfig = {
      ...configRef.current,
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
    notify("success", `Connected to RomM as ${username}.`);
  };

  const handleDownloadGame = async (game: RommGame) => {
    if (!rommSession) {
      setLibraryNotice({
        type: "error",
        message: "RomM connection required."
      });
      notify("error", "RomM connection required.");
      return;
    }

    const downloadUrl = resolveGameDownloadUrl(rommSession, game);

    if (!downloadUrl) {
      setLibraryNotice({
        type: "error",
        message: `Unable to resolve the download URL for "${game.name}".`
      });
      notify("error", `Unable to resolve the download URL for "${game.name}".`);
      return;
    }

    const targetFileName = resolveGameLocalFileName(game);
    const downloadId = String(game.id);
    const relativeSubdir = resolveGameRomSubdir(game);

    try {
      setDownloadProgressById((previous) => ({
        ...previous,
        [downloadId]: 0
      }));
      setLibraryNotice({
        type: "info",
        message: `Downloading "${game.name}" to Roms/${relativeSubdir}...`
      });
      notify("info", `Downloading "${game.name}" to Roms/${relativeSubdir}...`);
      void debugLog("info", "rom-download", `Starting ROM download: ${game.name}`, {
        downloadId,
        targetFileName,
        relativeSubdir,
        platform: game.platform_name ?? game.platform_display_name ?? null,
        expectedTotalBytes: game.fs_size_bytes ?? null
      });

      const result = await invoke<DownloadResult>("download_rom_command", {
        root: paths.root,
        request: {
          url: downloadUrl,
          fileName: targetFileName,
          bearerToken: rommSession.token,
          downloadId,
          expectedTotalBytes:
            typeof game.fs_size_bytes === "number" ? game.fs_size_bytes : undefined,
          relativeSubdir
        }
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
      notify("success", `ROM downloaded to ${result.filePath}`);
      void debugLog("success", "rom-download", `ROM registered locally: ${game.name}`, {
        downloadId,
        result
      });
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      setLibraryNotice({
        type: "error",
        message
      });
      notify("error", message);
      void debugLog("error", "rom-download", `ROM download failed: ${game.name}`, {
        downloadId,
        message
      });
    } finally {
      window.setTimeout(() => {
        setDownloadProgressById((previous) => {
          if (!(downloadId in previous)) {
            return previous;
          }

          const next = { ...previous };
          delete next[downloadId];
          delete romProgressLogStepRef.current[downloadId];
          return next;
        });
      }, 500);
    }
  };

  const launchSpecificRom = async (romPath: string) => {
    if (runningRomPathsRef.current[romPath]) {
      void debugLog("debug", "game-launch", "Ignored duplicate ROM launch while already running", {
        romPath
      });
      return;
    }

    const nextRunning = {
      ...runningRomPathsRef.current,
      [romPath]: true
    };
    runningRomPathsRef.current = nextRunning;
    setRunningRomPaths(nextRunning);

    let launched = false;

    try {
      void debugLog("info", "game-launch", "Launching ROM", { romPath });
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
      notify("success", `Session ${result.emulatorId} terminée et synchronisée pour ${result.romPath}`);
      void debugLog("success", "game-launch", `ROM session completed with ${result.emulatorId}`, result);
      launched = true;
      await refreshLocalRoms(paths.root);
      await refreshLocalSaves(paths.root);
      await refreshInstalledVersions(paths.root);
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      notify("error", message);
      void debugLog("error", "game-launch", "ROM launch failed", {
        romPath,
        message
      });
    } finally {
      clearRunningRomPath(romPath, launched ? LAUNCH_STARTUP_LOCK_MS : 0);
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
            .map((emu) => {
              const isSelected = selectedEmulatorId === emu.id;
              const isLaunching = launchingId === emu.id;
              const version = emu.version ?? emu.catalogVersion ?? "Unknown";

              return (
                <div
                  key={emu.id}
                  className={`emulator-menu-item ${isSelected ? "emulator-menu-item-active" : ""}`}
                >
                  <button
                    className="emulator-select-button"
                    type="button"
                    onClick={() => setSelectedEmulatorId(emu.id)}
                  >
                    <span className="emulator-menu-name">{emu.name}</span>
                    <span className="emulator-menu-platform">{emu.platformLabel}</span>
                    <span className="emulator-menu-version">Version {version}</span>
                  </button>
                  <button
                    className="emulator-menu-launch-button"
                    type="button"
                    disabled={isLaunching}
                    title={`Launch ${emu.name}`}
                    aria-label={`Launch ${emu.name}`}
                    onClick={() => void launchSelectedEmulator(emu.id)}
                  >
                    {isLaunching ? (
                      <span className="button-spinner" aria-hidden="true" />
                    ) : (
                      <span className="play-icon" aria-hidden="true" />
                    )}
                  </button>
                </div>
              );
            })}

          {installedCount === 0 ? (
            <div className="empty-state">
              <p>No emulator installed</p>
            </div>
          ) : null}
        </nav>
      </aside>

      <main className="content">
        <RommConnectionCard
          defaultBaseUrl={config.romm?.baseUrl}
          defaultUsername={config.romm?.username}
          onConnected={handleRommConnected}
        />

        <ControllerMappingPanel
          selectedEmulator={selectedEmulator}
          profiles={controllerProfiles}
          onSaveProfile={saveControllerProfile}
        />

        <LibraryPanel
          session={rommSession}
          localRoms={localRoms}
          saveSyncStatuses={saveSyncStatuses}
          onDownloadGame={handleDownloadGame}
          onLaunchLocalRom={launchSpecificRom}
          downloadProgressById={downloadProgressById}
          runningRomPaths={runningRomPaths}
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
                const installPercent = installProgressById[emu.id];
                const isInstalling = typeof installPercent === "number";
                const visibleInstallPercent = isInstalling
                  ? normalizeDownloadPercent(installPercent)
                  : 0;
                const isInstalled = emu.status === "installed";

                return (
                  <div key={emu.id} className="picker-item">
                    <div>
                      <strong>{emu.name}</strong>
                      <p>{emu.platformLabel}</p>
                    </div>
                    <button
                      className="primary-button picker-action-button"
                      disabled={isInstalling}
                      style={
                        isInstalling
                          ? {
                              background: `linear-gradient(
                                90deg,
                                #8fb1ff 0%,
                                #8fb1ff ${visibleInstallPercent}%,
                                #6d8cff ${visibleInstallPercent}%,
                                #6d8cff 100%
                              )`
                            }
                          : undefined
                      }
                      onClick={() =>
                        void (isInstalled ? removeInstalledFlag(emu.id) : installSelectedEmulator(emu.id))
                      }
                    >
                      {isInstalling
                        ? `Installing... ${Math.round(visibleInstallPercent)}%`
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

      <NotificationOverlay
        notifications={notifications}
        history={notificationHistory}
        historyOpen={notificationHistoryOpen}
        onToggleHistory={() => setNotificationHistoryOpen((open) => !open)}
        onDismiss={dismissNotification}
        onClearHistory={clearNotificationHistory}
      />
    </div>
  );
}

function normalizeDownloadPercent(value: number | undefined): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return 0;
  }

  return Math.min(100, Math.max(0, value));
}

async function openDebugWindow(): Promise<void> {
  try {
    const existing = await WebviewWindow.getByLabel(DEBUG_WINDOW_LABEL);
    if (existing) {
      await existing.unminimize();
      await existing.show();
      await existing.setFocus();
      void debugLog("debug", "debug", "Focused existing debug log window");
      return;
    }

    const debugWindow = new WebviewWindow(DEBUG_WINDOW_LABEL, {
      url: "/#/debug",
      title: "EmuManager Debug Logs",
      width: 1080,
      height: 720,
      minWidth: 760,
      minHeight: 440,
      resizable: true,
      focus: true
    });

    debugWindow.once("tauri://created", () => {
      void debugLog("info", "debug", "Debug log window opened");
    });
    debugWindow.once("tauri://error", (event) => {
      void debugLog("error", "debug", "Failed to open debug log window", event.payload);
    });
  } catch (reason) {
    void debugLog("error", "debug", "Failed to toggle debug log window", reason);
  }
}

function logProgressMilestone(
  ref: { current: Record<string, number> },
  id: string,
  percent: number,
  scope: string,
  label: string,
  details: Record<string, unknown>
) {
  const step = percent >= 100 ? 100 : Math.floor(percent / 10) * 10;
  const previous = ref.current[id] ?? -1;

  if (step <= previous || step === 0) {
    return;
  }

  ref.current[id] = step;
  void debugLog("debug", scope, `${label}: ${step}%`, {
    ...details,
    percent: step
  });
}
