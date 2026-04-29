import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import LibraryPanel from "./components/LibraryPanel";
import RommConnectionCard from "./components/RommConnectionCard";
import ControllerMappingPanel from "./components/ControllerMappingPanel";
import NotificationOverlay, {
  type NotificationEntry,
  type NotificationKind
} from "./components/NotificationOverlay";
import WindowTitlebar from "./components/WindowTitlebar";
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
  AppUpdateDownloadResult,
  AppUpdateStatus,
  ConfigureResult,
  ControllerProfile,
  ControllerProfileSaveResult,
  DeleteLocalRomResult,
  DownloadResult,
  EmulatorEntry,
  EmulatorResourceSummary,
  GameLaunchResult,
  InstallResult,
  LaunchResult,
  LocalRomEntry,
  LocalSaveEntry,
  ManualImportPlatform,
  ManualImportResult,
  PortablePaths,
  ResourceInstallResult,
  SaveConflictResolution,
  SaveConflictStatus,
  SaveSyncStatus,
  UninstallResult
} from "./types";

const fallbackPaths = buildPortablePaths(".");
const DEBUG_WINDOW_LABEL = "debug-logs";
const LAUNCH_STARTUP_LOCK_MS = 1800;
const NOTIFICATION_TTL_MS = 5000;
const NOTIFICATION_EXIT_MS = 220;
const MAX_NOTIFICATION_HISTORY = 80;
const DUPLICATE_IMPORT_PREFIX = "DUPLICATE_IMPORT:";
const SUPPORTED_MANUAL_IMPORT_EXTENSIONS = new Set([
  "zip",
  "rar",
  "iso",
  "rvz",
  "wbfs",
  "gcz",
  "ciso",
  "nds",
  "3ds",
  "cci",
  "cia",
  "3dsx",
  "xci",
  "nsp",
  "nro",
  "cue",
  "bin",
  "img",
  "chd"
]);

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

interface EmulatorResourceProgressPayload {
  emulatorId: string;
  resourceId: string;
  resourceLabel: string;
  stage: string;
  message: string;
  percent?: number | null;
}

interface AppUpdateProgressPayload {
  version: string;
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
  const [resourceSummaries, setResourceSummaries] = useState<Record<string, EmulatorResourceSummary>>({});
  const [resourceProgressByKey, setResourceProgressByKey] = useState<Record<string, EmulatorResourceProgressPayload>>({});
  const [localRoms, setLocalRoms] = useState<LocalRomEntry[]>([]);
  const [localSaves, setLocalSaves] = useState<LocalSaveEntry[]>([]);
  const [manualImportPlatforms, setManualImportPlatforms] = useState<ManualImportPlatform[]>([]);
  const [manualImportDragActive, setManualImportDragActive] = useState(false);
  const [pendingManualImport, setPendingManualImport] = useState<{
    sourcePath: string;
    fileName: string;
    conflictMessage?: string;
  } | null>(null);
  const [manualImportPlatformId, setManualImportPlatformId] = useState("");
  const [manualImportError, setManualImportError] = useState<string | null>(null);
  const [manualImporting, setManualImporting] = useState(false);
  const [pendingDeleteRom, setPendingDeleteRom] = useState<{
    romPath: string;
    title: string;
    fileName: string;
  } | null>(null);
  const [pendingSaveConflict, setPendingSaveConflict] = useState<{
    romPath: string;
    localOnly: boolean;
    conflict: SaveConflictStatus;
  } | null>(null);
  const [deletingRomPath, setDeletingRomPath] = useState<string | null>(null);
  const [pendingUninstallEmulator, setPendingUninstallEmulator] = useState<EmulatorEntry | null>(null);
  const [uninstallingId, setUninstallingId] = useState<string | null>(null);
  const [controllerProfiles, setControllerProfiles] = useState<ControllerProfile[]>([]);
  const [saveSyncStatuses, setSaveSyncStatuses] = useState<Record<string, SaveSyncStatus>>({});
  const [selectedEmulatorId, setSelectedEmulatorId] = useState<string | null>(null);
  const [showPicker, setShowPicker] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
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
  const [currentAppVersion, setCurrentAppVersion] = useState<string | null>(null);
  const [appUpdateStatus, setAppUpdateStatus] = useState<AppUpdateStatus | null>(null);
  const [checkingAppUpdate, setCheckingAppUpdate] = useState(false);
  const [appUpdatePromptOpen, setAppUpdatePromptOpen] = useState(false);
  const [appUpdateError, setAppUpdateError] = useState<string | null>(null);
  const [appUpdateProgress, setAppUpdateProgress] = useState<number | null>(null);
  const [appUpdateDownloading, setAppUpdateDownloading] = useState(false);
  const [appUpdateApplying, setAppUpdateApplying] = useState(false);
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

  const refreshEmulatorResourceStatuses = async (root: string) => {
    const summaries = await invoke<EmulatorResourceSummary[]>(
      "get_emulator_resource_statuses_command",
      { root }
    );
    const byId = Object.fromEntries(
      summaries.map((summary) => [summary.emulatorId, summary])
    );
    setResourceSummaries(byId);
    return byId;
  };

  const checkForAppUpdate = async (
    root = paths.root,
    configOverride: AppConfig = configRef.current,
    options: { manual?: boolean; silent?: boolean } = {}
  ) => {
    try {
      setCheckingAppUpdate(true);
      setAppUpdateError(null);
      if (!isAppUpdateBusy) {
        setAppUpdateProgress(null);
      }
      const status = await invoke<AppUpdateStatus>("check_app_update_command");
      setCurrentAppVersion(status.currentVersion);
      setAppUpdateStatus(status);

      if (status.updateAvailable) {
        const skipped = configOverride.skippedAppUpdateVersion === status.latestVersion;
        if (options.manual || !skipped) {
          setAppUpdatePromptOpen(true);
        }
        if (options.manual) {
          notify("info", `EmuManager ${status.latestVersion ?? "update"} is available.`);
        }
      } else if (options.manual) {
        notify("success", "EmuManager is up to date.");
      }

      void debugLog("info", "app-update", "Update check completed", {
        root,
        currentVersion: status.currentVersion,
        latestVersion: status.latestVersion,
        updateAvailable: status.updateAvailable,
        assetName: status.assetName
      });
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      setAppUpdateError(message);
      if (options.manual && !options.silent) {
        notify("error", message);
      }
      void debugLog("warning", "app-update", "Update check failed", message);
    } finally {
      setCheckingAppUpdate(false);
    }
  };

  useEffect(() => {
    const bootstrap = async () => {
      try {
        void debugLog("info", "app", "Application bootstrap started");
        const portablePaths = await invoke<PortablePaths>("init_portable_layout");
        setPaths(portablePaths);
        void debugLog("debug", "paths", "Portable layout initialized", portablePaths);

        const appVersion = await invoke<string>("get_app_version_command");
        setCurrentAppVersion(appVersion);

        const savedConfig = await invoke<AppConfig>("load_app_config", {
          root: portablePaths.root
        });
        setConfig(savedConfig);
        configRef.current = savedConfig;

        const importPlatforms = await invoke<ManualImportPlatform[]>(
          "get_manual_import_platforms_command"
        );
        setManualImportPlatforms(importPlatforms);
        setManualImportPlatformId(importPlatforms[0]?.id ?? "");

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
        await refreshEmulatorResourceStatuses(portablePaths.root);
        void checkForAppUpdate(portablePaths.root, nextConfig, { silent: true });

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
    let unlistenResourceProgress: UnlistenFn | null = null;
    let unlistenAppUpdateProgress: UnlistenFn | null = null;
    let unlistenAppUpdateComplete: UnlistenFn | null = null;
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

      unlistenResourceProgress = await listen<EmulatorResourceProgressPayload>(
        "emulator-resource-progress",
        (event) => {
          const payload = event.payload;
          const key = resourceProgressKey(payload.emulatorId, payload.resourceId);
          setResourceProgressByKey((previous) => ({
            ...previous,
            [key]: payload
          }));
          void debugLog("info", "emulator-resource", payload.message, {
            emulatorId: payload.emulatorId,
            resourceId: payload.resourceId,
            stage: payload.stage,
            percent: payload.percent ?? null
          });
        }
      );

      unlistenAppUpdateProgress = await listen<AppUpdateProgressPayload>(
        "app-update-progress",
        (event) => {
          const payload = event.payload;
          const percent = normalizeDownloadPercent(payload.percent);
          setAppUpdateProgress(percent);
        }
      );

      unlistenAppUpdateComplete = await listen<AppUpdateProgressPayload>(
        "app-update-complete",
        (event) => {
          const payload = event.payload;
          setAppUpdateProgress(100);
          void debugLog("success", "app-update", `Downloaded EmuManager ${payload.version}`, {
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
      if (unlistenResourceProgress) {
        unlistenResourceProgress();
      }
      if (unlistenAppUpdateProgress) {
        unlistenAppUpdateProgress();
      }
      if (unlistenAppUpdateComplete) {
        unlistenAppUpdateComplete();
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
  const isAppUpdateBusy = appUpdateDownloading || appUpdateApplying;
  const appUpdateCurrentVersion = appUpdateStatus?.currentVersion ?? currentAppVersion ?? "Unknown";
  const appUpdateLatestVersion = appUpdateStatus
    ? appUpdateStatus.latestVersion ?? "No release"
    : "Not checked";
  const appUpdateSkipped =
    Boolean(appUpdateStatus?.latestVersion) &&
    config.skippedAppUpdateVersion === appUpdateStatus?.latestVersion;

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

  const updatePinnedLibraryItems = async (pinnedItemIds: string[]) => {
    const nextConfig: AppConfig = {
      ...configRef.current,
      pinnedLibraryItems: pinnedItemIds
    };

    await persistConfig(nextConfig);
  };

  const closeAppUpdatePrompt = () => {
    if (appUpdateDownloading || appUpdateApplying) {
      return;
    }

    setAppUpdatePromptOpen(false);
    setAppUpdateError(null);
    setAppUpdateProgress(null);
  };

  const skipAppUpdateVersion = async () => {
    const latestVersion = appUpdateStatus?.latestVersion;
    if (!latestVersion) {
      return;
    }

    const nextConfig: AppConfig = {
      ...configRef.current,
      skippedAppUpdateVersion: latestVersion
    };

    await persistConfig(nextConfig);
    setAppUpdatePromptOpen(false);
    notify("info", `Skipped EmuManager ${latestVersion}.`);
    void debugLog("info", "app-update", "Skipped update version", {
      latestVersion
    });
  };

  const startAppUpdate = async () => {
    if (!appUpdateStatus?.updateAvailable || !appUpdateStatus.latestVersion) {
      return;
    }

    if (!appUpdateStatus.downloadUrl || !appUpdateStatus.assetName) {
      const message = "No compatible EmuManager executable was found in the latest release.";
      setAppUpdateError(message);
      notify("error", message);
      return;
    }

    try {
      setAppUpdateDownloading(true);
      setAppUpdateApplying(false);
      setAppUpdateProgress(0);
      setAppUpdateError(null);
      notify("info", `Downloading EmuManager ${appUpdateStatus.latestVersion}...`);
      void debugLog("info", "app-update", "Starting app update download", {
        latestVersion: appUpdateStatus.latestVersion,
        assetName: appUpdateStatus.assetName
      });

      const result = await invoke<AppUpdateDownloadResult>("download_app_update_command", {
        root: paths.root,
        request: {
          version: appUpdateStatus.latestVersion,
          assetName: appUpdateStatus.assetName,
          downloadUrl: appUpdateStatus.downloadUrl,
          assetSize: appUpdateStatus.assetSize ?? undefined
        }
      });

      setAppUpdateApplying(true);
      notify("info", "Applying update and relaunching...");
      void debugLog("info", "app-update", "Applying app update", result);

      await invoke("apply_app_update_command", {
        filePath: result.filePath
      });
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      setAppUpdateError(message);
      notify("error", message);
      void debugLog("error", "app-update", "App update failed", message);
    } finally {
      setAppUpdateDownloading(false);
      setAppUpdateApplying(false);
    }
  };

  const handleManualImportDrop = (droppedPaths: string[]) => {
    if (droppedPaths.length !== 1) {
      const message = "Drop one ROM or archive at a time.";
      setLibraryNotice({ type: "error", message });
      notify("error", message);
      void debugLog("warning", "manual-import", "Rejected multi-file drop", {
        droppedCount: droppedPaths.length
      });
      return;
    }

    const sourcePath = droppedPaths[0];
    const fileName = getPathFileName(sourcePath);

    if (!isSupportedManualImportFile(fileName)) {
      const message = "Unsupported file. Drop a ROM, .zip, or .rar archive.";
      setLibraryNotice({ type: "error", message });
      notify("error", message);
      void debugLog("warning", "manual-import", "Rejected unsupported file", {
        sourcePath
      });
      return;
    }

    const nextPlatformId = manualImportPlatformId || manualImportPlatforms[0]?.id || "";
    setManualImportPlatformId(nextPlatformId);
    setManualImportError(null);
    setPendingManualImport({
      sourcePath,
      fileName
    });
    void debugLog("info", "manual-import", "Manual import file dropped", {
      sourcePath,
      fileName
    });
  };

  const closeManualImportModal = () => {
    if (manualImporting) {
      return;
    }

    setPendingManualImport(null);
    setManualImportError(null);
  };

  const importPendingManualRom = async (overwrite = false) => {
    if (!pendingManualImport || !manualImportPlatformId) {
      return;
    }

    try {
      setManualImporting(true);
      setManualImportError(null);
      void debugLog("info", "manual-import", "Starting manual ROM import", {
        sourcePath: pendingManualImport.sourcePath,
        platformId: manualImportPlatformId,
        overwrite
      });

      const result = await invoke<ManualImportResult>("import_local_rom_command", {
        root: paths.root,
        request: {
          sourcePath: pendingManualImport.sourcePath,
          platformId: manualImportPlatformId,
          overwrite
        }
      });

      await refreshLocalRoms(paths.root);
      await refreshLocalSaves(paths.root);

      const importCount = result.importedRoms.length;
      const message =
        importCount === 1
          ? `Imported ${result.importedRoms[0].fileName} to Roms/${result.platformId}.`
          : `Imported ${importCount} ROMs to Roms/${result.platformId}.`;

      setLibraryNotice({
        type: "success",
        message
      });
      notify("success", message);
      setPendingManualImport(null);
      void debugLog("success", "manual-import", "Manual ROM import completed", result);
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);

      if (message.startsWith(DUPLICATE_IMPORT_PREFIX)) {
        const conflictMessage = formatDuplicateImportMessage(message);
        setPendingManualImport((current) =>
          current
            ? {
                ...current,
                conflictMessage
              }
            : current
        );
        void debugLog("warning", "manual-import", "Manual import duplicate detected", {
          sourcePath: pendingManualImport.sourcePath,
          conflictMessage
        });
        return;
      }

      setManualImportError(message);
      setLibraryNotice({
        type: "error",
        message
      });
      notify("error", message);
      void debugLog("error", "manual-import", "Manual ROM import failed", {
        sourcePath: pendingManualImport.sourcePath,
        message
      });
    } finally {
      setManualImporting(false);
    }
  };

  useEffect(() => {
    let disposed = false;
    let unlistenDragDrop: UnlistenFn | null = null;

    const setupDragDrop = async () => {
      try {
        const unlisten = await getCurrentWebview().onDragDropEvent((event) => {
          if (event.payload.type === "enter" || event.payload.type === "over") {
            setManualImportDragActive(true);
            return;
          }

          if (event.payload.type === "leave") {
            setManualImportDragActive(false);
            return;
          }

          if (event.payload.type === "drop") {
            setManualImportDragActive(false);
            handleManualImportDrop(event.payload.paths);
          }
        });

        if (disposed) {
          unlisten();
          return;
        }

        unlistenDragDrop = unlisten;
      } catch (reason) {
        void debugLog("warning", "manual-import", "File drag-drop listener unavailable", reason);
      }
    };

    void setupDragDrop();

    return () => {
      disposed = true;
      if (unlistenDragDrop) {
        unlistenDragDrop();
      }
    };
  }, [manualImportPlatforms, manualImportPlatformId]);

  const requestDeleteLocalRom = (romPath: string, title: string) => {
    setPendingDeleteRom({
      romPath,
      title,
      fileName: getPathFileName(romPath)
    });
  };

  const closeDeleteRomModal = () => {
    if (deletingRomPath) {
      return;
    }

    setPendingDeleteRom(null);
  };

  const closeSaveConflictModal = () => {
    setPendingSaveConflict(null);
  };

  const confirmSaveConflictResolution = (resolution: SaveConflictResolution) => {
    if (!pendingSaveConflict) {
      return;
    }

    const { romPath, localOnly } = pendingSaveConflict;
    setPendingSaveConflict(null);
    void launchSpecificRom(romPath, localOnly, resolution);
  };

  const confirmDeleteLocalRom = async () => {
    if (!pendingDeleteRom) {
      return;
    }

    try {
      setDeletingRomPath(pendingDeleteRom.romPath);
      void debugLog("info", "library", "Deleting local ROM", {
        romPath: pendingDeleteRom.romPath
      });

      const result = await invoke<DeleteLocalRomResult>("delete_local_rom_command", {
        root: paths.root,
        romPath: pendingDeleteRom.romPath
      });

      await refreshLocalRoms(paths.root);
      await refreshLocalSaves(paths.root);
      setLibraryNotice({
        type: "success",
        message: `Deleted ${result.fileName}.`
      });
      notify("success", `Deleted ${result.fileName}.`);
      setPendingDeleteRom(null);
      void debugLog("success", "library", "Local ROM deleted", result);
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      setLibraryNotice({ type: "error", message });
      notify("error", message);
      void debugLog("error", "library", "Local ROM delete failed", {
        romPath: pendingDeleteRom.romPath,
        message
      });
    } finally {
      setDeletingRomPath(null);
    }
  };

  const requestUninstallEmulator = (emulator: EmulatorEntry) => {
    setPendingUninstallEmulator(emulator);
  };

  const closeUninstallEmulatorModal = () => {
    if (uninstallingId) {
      return;
    }

    setPendingUninstallEmulator(null);
  };

  const confirmUninstallEmulator = async () => {
    if (!pendingUninstallEmulator) {
      return;
    }

    const { id, name } = pendingUninstallEmulator;

    try {
      setUninstallingId(id);
      void debugLog("info", "emulator-uninstall", `Uninstalling emulator ${id}`, {
        root: paths.root
      });

      const result = await invoke<UninstallResult>("uninstall_emulator_command", {
        root: paths.root,
        emulatorId: id
      });

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

      await refreshInstalledVersions(paths.root);
      await refreshEmulatorResourceStatuses(paths.root);
      setPendingUninstallEmulator(null);
      notify(
        "success",
        result.removed ? `Uninstalled ${name}.` : `${name} was already absent from disk.`
      );
      void debugLog("success", "emulator-uninstall", `Uninstalled emulator ${id}`, result);
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      notify("error", message);
      void debugLog("error", "emulator-uninstall", `Uninstall failed for ${id}`, message);
    } finally {
      setUninstallingId(null);
    }
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
      await refreshEmulatorResourceStatuses(paths.root);
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

  const installResourcesForEmulator = async (
    id: string,
    sessionOverride: RommSession | null = rommSession,
    knownSummary?: EmulatorResourceSummary
  ) => {
    const summary = knownSummary ?? resourceSummaries[id];
    if (!summary || summary.requirements.length === 0 || summary.ready) {
      return true;
    }

    const displayName = emulators.find((emu) => emu.id === id)?.name ?? id;

    if (!sessionOverride) {
      const message = formatResourceBlockMessage(summary);
      notify("error", `${message} Connect to RomM to install missing resources automatically.`);
      return false;
    }

    try {
      notify("info", `Installing required resources for ${displayName}...`);
      void debugLog("info", "emulator-resource", `Installing resources for ${id}`, {
        missing: summary.statuses
          .filter((status) => status.required && status.state !== "valid")
          .map((status) => status.id)
      });

      const result = await invoke<ResourceInstallResult>("install_emulator_resources_command", {
        root: paths.root,
        emulatorId: id,
        rommSession: {
          baseUrl: sessionOverride.baseUrl,
          token: sessionOverride.token
        }
      });

      setResourceSummaries((previous) => ({
        ...previous,
        [result.summary.emulatorId]: result.summary
      }));
      await refreshEmulatorResourceStatuses(paths.root);

      notify(
        result.summary.ready ? "success" : "warning",
        result.summary.ready
          ? `${displayName} resources are ready.`
          : formatResourceBlockMessage(result.summary)
      );
      void debugLog("success", "emulator-resource", `Resource installation completed for ${id}`, result);

      return result.summary.ready;
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      notify("error", message);
      void debugLog("error", "emulator-resource", `Resource installation failed for ${id}`, message);
      await refreshEmulatorResourceStatuses(paths.root);
      return false;
    } finally {
      window.setTimeout(() => {
        setResourceProgressByKey((previous) =>
          Object.fromEntries(
            Object.entries(previous).filter(
              ([key]) => !key.startsWith(`${id}:`)
            )
          )
        );
      }, 700);
    }
  };

  const ensureResourcesForEmulator = async (
    id: string,
    sessionOverride: RommSession | null = rommSession
  ) => {
    const summaries =
      Object.keys(resourceSummaries).length > 0
        ? resourceSummaries
        : await refreshEmulatorResourceStatuses(paths.root);
    const summary = summaries[id];

    if (!summary || summary.requirements.length === 0 || summary.ready) {
      return true;
    }

    return installResourcesForEmulator(id, sessionOverride, summary);
  };

  const installMissingResourcesForInstalledEmulators = async (session: RommSession) => {
    const summaries = await refreshEmulatorResourceStatuses(paths.root);

    for (const emu of emulators.filter((entry) => entry.status === "installed")) {
      const summary = summaries[emu.id];
      if (summary && summary.requirements.length > 0 && !summary.ready) {
        await installResourcesForEmulator(emu.id, session, summary);
      }
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
    await installMissingResourcesForInstalledEmulators(session);
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

  const launchSpecificRom = async (
    romPath: string,
    localOnly = false,
    saveConflictResolution?: SaveConflictResolution
  ) => {
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
      const emulatorId = await invoke<string>("resolve_emulator_id_for_rom_command", {
        root: paths.root,
        romPath
      });
      const resourcesReady = await ensureResourcesForEmulator(
        emulatorId,
        rommSession
      );
      if (!resourcesReady) {
        return;
      }

      if (!localOnly && rommSession && !saveConflictResolution) {
        const conflict = await invoke<SaveConflictStatus | null>(
          "get_save_conflict_status_command",
          {
            root: paths.root,
            romPath,
            rommSession: {
              baseUrl: rommSession.baseUrl,
              token: rommSession.token
            } satisfies RommLaunchSession
          }
        );

        if (conflict) {
          setPendingSaveConflict({
            romPath,
            localOnly,
            conflict
          });
          return;
        }
      }

      void debugLog("info", "game-launch", "Launching ROM", { romPath, localOnly });
      const result = await invoke<GameLaunchResult>("launch_game_auto_command", {
        root: paths.root,
        romPath,
        rommSession: !localOnly && rommSession
          ? ({
              baseUrl: rommSession.baseUrl,
              token: rommSession.token,
              saveConflictResolution
            } satisfies RommLaunchSession)
          : null
      });
      notify(
        "success",
        localOnly
          ? `Session ${result.emulatorId} terminee en local pour ${result.romPath}`
          : `Session ${result.emulatorId} terminee et synchronisee pour ${result.romPath}`
      );
      void debugLog("success", "game-launch", `ROM session completed with ${result.emulatorId}`, result);
      launched = true;
      await refreshLocalRoms(paths.root);
      await refreshLocalSaves(paths.root);
      await refreshInstalledVersions(paths.root);
    } catch (reason) {
      const conflict = parseSaveConflictError(reason);
      if (conflict) {
        setPendingSaveConflict({
          romPath,
          localOnly,
          conflict
        });
        return;
      }

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
      <div className="window-shell">
        <WindowTitlebar />
        <div className="window-content">
          <div className="center-screen">
            <div className="panel loading-panel">
              <h2 className="panel-title">Initialization</h2>
              <p className="panel-subtitle">Preparing portable environment</p>
            </div>
          </div>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="window-shell">
        <WindowTitlebar />
        <div className="window-content">
          <div className="center-screen">
            <div className="panel loading-panel">
              <h2 className="panel-title">Error</h2>
              <p className="panel-subtitle">Failed to initialize EmuManager</p>
              <p>{error}</p>
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="window-shell">
      <WindowTitlebar />
      <div className="window-content">
        <div className="app-shell">
          <aside className="sidebar">
            <div>
              <h2 className="sidebar-title">EmuManager</h2>
              <p className="muted">{installedCount} installed</p>
            </div>

            <button className="primary-button" onClick={() => setShowPicker(true)}>
              Emulators
            </button>
            <button className="ghost-button sidebar-settings-button" onClick={() => setShowSettings(true)}>
              Settings
            </button>

            <nav className="emulator-list">
              {emulators
                .filter((emu) => emu.status === "installed")
                .map((emu) => {
                  const isSelected = selectedEmulatorId === emu.id;
                  const isLaunching = launchingId === emu.id;
                  const version = emu.version ?? emu.catalogVersion ?? "Unknown";
                  const resourceSummary = resourceSummaries[emu.id];

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
                        <span className="emulator-menu-name-row">
                          <span className="emulator-menu-name">{emu.name}</span>
                          <ResourceIndicators summary={resourceSummary} />
                        </span>
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
              root={paths.root}
              session={rommSession}
              localRoms={localRoms}
              saveSyncStatuses={saveSyncStatuses}
              onDownloadGame={handleDownloadGame}
              onLaunchLocalRom={launchSpecificRom}
              onRequestDeleteLocalRom={requestDeleteLocalRom}
              pinnedItemIds={config.pinnedLibraryItems ?? []}
              onPinnedItemIdsChange={updatePinnedLibraryItems}
              downloadProgressById={downloadProgressById}
              runningRomPaths={runningRomPaths}
              notice={libraryNotice}
              manualImportDragActive={manualImportDragActive}
              pendingManualImportFileName={pendingManualImport?.fileName ?? null}
            />
          </main>
        </div>
      </div>

      {pendingSaveConflict ? (
        <div className="modal-backdrop" onClick={closeSaveConflictModal}>
          <div className="modal confirmation-modal" onClick={(event) => event.stopPropagation()}>
            <div className="modal-header">
              <div>
                <h2 className="panel-title">Save conflict</h2>
                <p className="panel-subtitle">{getPathFileName(pendingSaveConflict.romPath)}</p>
              </div>
              <button className="ghost-button" onClick={closeSaveConflictModal}>
                Close
              </button>
            </div>

            <p className="confirmation-copy">
              Local and RomM saves both changed since the last sync. Choose which save should be
              used for this launch.
            </p>

            <div className="save-conflict-compare">
              <div className="save-conflict-option">
                <strong>Local save</strong>
                <span>{formatConflictTimestamp(pendingSaveConflict.conflict.localSaveUpdatedAtMs)}</span>
                <small>Kept on this device; uploaded back to RomM after play.</small>
              </div>
              <div className="save-conflict-option">
                <strong>RomM save</strong>
                <span>
                  {formatConflictIsoTimestamp(pendingSaveConflict.conflict.remoteSaveUpdatedAt)}
                </span>
                <small>{pendingSaveConflict.conflict.remoteSaveFileName ?? "Remote save archive"}</small>
              </div>
            </div>

            <div className="confirmation-actions save-conflict-actions">
              <button className="ghost-button" onClick={closeSaveConflictModal}>
                Cancel
              </button>
              <button
                className="primary-button"
                onClick={() => confirmSaveConflictResolution("useLocal")}
              >
                Use local save
              </button>
              <button
                className="primary-button"
                onClick={() => confirmSaveConflictResolution("useRomm")}
              >
                Use RomM save
              </button>
            </div>
          </div>
        </div>
      ) : null}

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
                const resourceSummary = resourceSummaries[emu.id];
                const resourceProgress = getActiveResourceProgress(
                  resourceProgressByKey,
                  emu.id
                );

                return (
                  <div key={emu.id} className="picker-item">
                    <div>
                      <div className="picker-title-row">
                        <strong>{emu.name}</strong>
                        <ResourceIndicators summary={resourceSummary} />
                      </div>
                      <p>{emu.platformLabel}</p>
                      {resourceSummary?.requirements.length ? (
                        <small className="resource-summary-line">
                          {formatResourceSummaryLine(resourceSummary)}
                        </small>
                      ) : null}
                      {resourceProgress ? (
                        <small className="resource-progress-line">
                          {resourceProgress.message}
                          {typeof resourceProgress.percent === "number"
                            ? ` ${Math.round(normalizeDownloadPercent(resourceProgress.percent))}%`
                            : ""}
                        </small>
                      ) : null}
                    </div>
                    <button
                      className="primary-button picker-action-button"
                      disabled={isInstalling || uninstallingId === emu.id}
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
                        void (isInstalled ? requestUninstallEmulator(emu) : installSelectedEmulator(emu.id))
                      }
                    >
                      {uninstallingId === emu.id
                        ? "Uninstalling..."
                        : isInstalling
                          ? `Installing... ${Math.round(visibleInstallPercent)}%`
                          : isInstalled
                            ? "Uninstall"
                            : "Install"}
                    </button>
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      ) : null}

      {showSettings ? (
        <div className="modal-backdrop" onClick={() => !isAppUpdateBusy && setShowSettings(false)}>
          <div className="modal settings-modal" onClick={(event) => event.stopPropagation()}>
            <div className="modal-header">
              <div>
                <h2 className="panel-title">Settings</h2>
                <p className="panel-subtitle">EmuManager</p>
              </div>
              <button
                className="ghost-button"
                disabled={isAppUpdateBusy}
                onClick={() => setShowSettings(false)}
              >
                Close
              </button>
            </div>

            <div className="version-grid">
              <div className="version-row">
                <span>Current version</span>
                <strong>{appUpdateCurrentVersion}</strong>
              </div>
              <div className="version-row">
                <span>Latest version</span>
                <strong>{appUpdateLatestVersion}</strong>
              </div>
              <div className="version-row">
                <span>Status</span>
                <strong>
                  {checkingAppUpdate
                    ? "Checking..."
                    : appUpdateStatus?.updateAvailable
                      ? appUpdateSkipped
                        ? "Skipped"
                        : "Update available"
                      : appUpdateStatus
                        ? "Up to date"
                        : "Not checked"}
                </strong>
              </div>
              {appUpdateStatus?.assetName ? (
                <div className="version-row">
                  <span>Update asset</span>
                  <strong>{appUpdateStatus.assetName}</strong>
                </div>
              ) : null}
            </div>

            {appUpdateError ? (
              <div className="inline-notice inline-notice-error">{appUpdateError}</div>
            ) : null}

            {typeof appUpdateProgress === "number" ? (
              <div className="update-progress">
                <div className="mapping-progress-track">
                  <span style={{ width: `${normalizeDownloadPercent(appUpdateProgress)}%` }} />
                </div>
                <span>
                  {appUpdateApplying
                    ? "Applying update..."
                    : `Downloading... ${Math.round(normalizeDownloadPercent(appUpdateProgress))}%`}
                </span>
              </div>
            ) : null}

            <div className="settings-actions">
              <button
                className="ghost-button"
                disabled={checkingAppUpdate || isAppUpdateBusy}
                onClick={() => void checkForAppUpdate(paths.root, configRef.current, { manual: true })}
              >
                {checkingAppUpdate ? "Checking..." : "Check for updates"}
              </button>
              <button
                className="ghost-button"
                disabled={!appUpdateStatus?.updateAvailable || appUpdateSkipped || isAppUpdateBusy}
                onClick={() => void skipAppUpdateVersion()}
              >
                Skip version
              </button>
              <button
                className="primary-button"
                disabled={!appUpdateStatus?.updateAvailable || !appUpdateStatus.downloadUrl || isAppUpdateBusy}
                onClick={() => setAppUpdatePromptOpen(true)}
              >
                Update now
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {appUpdatePromptOpen && appUpdateStatus?.updateAvailable ? (
        <div className="modal-backdrop" onClick={closeAppUpdatePrompt}>
          <div className="modal confirmation-modal" onClick={(event) => event.stopPropagation()}>
            <div className="modal-header">
              <div>
                <h2 className="panel-title">Update EmuManager?</h2>
                <p className="panel-subtitle">
                  {appUpdateCurrentVersion} to {appUpdateStatus.latestVersion}
                </p>
              </div>
              <button className="ghost-button" disabled={isAppUpdateBusy} onClick={closeAppUpdatePrompt}>
                Close
              </button>
            </div>

            <p className="confirmation-copy">
              A new EmuManager build is available. The app will download it, replace the current
              executable, and relaunch.
            </p>

            {appUpdateStatus.assetName ? (
              <div className="version-row update-asset-row">
                <span>Asset</span>
                <strong>{appUpdateStatus.assetName}</strong>
              </div>
            ) : (
              <div className="inline-notice inline-notice-warning">
                No compatible Windows executable was found in this release.
              </div>
            )}

            {appUpdateError ? (
              <div className="inline-notice inline-notice-error">{appUpdateError}</div>
            ) : null}

            {typeof appUpdateProgress === "number" ? (
              <div className="update-progress">
                <div className="mapping-progress-track">
                  <span style={{ width: `${normalizeDownloadPercent(appUpdateProgress)}%` }} />
                </div>
                <span>
                  {appUpdateApplying
                    ? "Applying update..."
                    : `Downloading... ${Math.round(normalizeDownloadPercent(appUpdateProgress))}%`}
                </span>
              </div>
            ) : null}

            <div className="confirmation-actions save-conflict-actions">
              <button className="ghost-button" disabled={isAppUpdateBusy} onClick={closeAppUpdatePrompt}>
                Remind me later
              </button>
              <button
                className="ghost-button"
                disabled={isAppUpdateBusy}
                onClick={() => void skipAppUpdateVersion()}
              >
                Skip version
              </button>
              <button
                className="primary-button"
                disabled={!appUpdateStatus.downloadUrl || isAppUpdateBusy}
                onClick={() => void startAppUpdate()}
              >
                {isAppUpdateBusy ? "Updating..." : "Update and restart"}
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {pendingDeleteRom ? (
        <div className="modal-backdrop" onClick={closeDeleteRomModal}>
          <div className="modal confirmation-modal" onClick={(event) => event.stopPropagation()}>
            <div className="modal-header">
              <div>
                <h2 className="panel-title">Delete ROM?</h2>
                <p className="panel-subtitle">{pendingDeleteRom.title}</p>
              </div>
              <button className="ghost-button" disabled={Boolean(deletingRomPath)} onClick={closeDeleteRomModal}>
                Close
              </button>
            </div>
            <p className="confirmation-copy">
              This will permanently delete {pendingDeleteRom.fileName} from the local ROM folder.
            </p>
            <div className="confirmation-actions">
              <button className="ghost-button" disabled={Boolean(deletingRomPath)} onClick={closeDeleteRomModal}>
                Cancel
              </button>
              <button
                className="danger-button"
                disabled={Boolean(deletingRomPath)}
                onClick={() => void confirmDeleteLocalRom()}
              >
                {deletingRomPath ? "Deleting..." : "Delete ROM"}
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {pendingUninstallEmulator ? (
        <div className="modal-backdrop" onClick={closeUninstallEmulatorModal}>
          <div className="modal confirmation-modal" onClick={(event) => event.stopPropagation()}>
            <div className="modal-header">
              <div>
                <h2 className="panel-title">Uninstall emulator?</h2>
                <p className="panel-subtitle">
                  {pendingUninstallEmulator.name} - {pendingUninstallEmulator.platformLabel}
                </p>
              </div>
              <button className="ghost-button" disabled={Boolean(uninstallingId)} onClick={closeUninstallEmulatorModal}>
                Close
              </button>
            </div>
            <p className="confirmation-copy">
              This will permanently delete the emulator installation from disk.
            </p>
            <div className="confirmation-actions">
              <button className="ghost-button" disabled={Boolean(uninstallingId)} onClick={closeUninstallEmulatorModal}>
                Cancel
              </button>
              <button
                className="danger-button"
                disabled={Boolean(uninstallingId)}
                onClick={() => void confirmUninstallEmulator()}
              >
                {uninstallingId ? "Uninstalling..." : "Uninstall"}
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {pendingManualImport ? (
        <div className="modal-backdrop" onClick={closeManualImportModal}>
          <div className="modal manual-import-modal" onClick={(event) => event.stopPropagation()}>
            <div className="modal-header">
              <div>
                <h2 className="panel-title">Which platform is this ROM for?</h2>
                <p className="panel-subtitle">{pendingManualImport.fileName}</p>
              </div>
              <button className="ghost-button" disabled={manualImporting} onClick={closeManualImportModal}>
                Close
              </button>
            </div>

            <div className="manual-import-platform-grid" role="listbox" aria-label="ROM platform">
              {manualImportPlatforms.map((platform) => {
                const isSelected = platform.id === manualImportPlatformId;

                return (
                  <button
                    key={platform.id}
                    className={`manual-import-platform-option ${
                      isSelected ? "manual-import-platform-option-active" : ""
                    }`}
                    type="button"
                    role="option"
                    aria-selected={isSelected}
                    disabled={manualImporting}
                    onClick={() => {
                      setManualImportPlatformId(platform.id);
                      setManualImportError(null);
                      setPendingManualImport((current) =>
                        current
                          ? {
                              ...current,
                              conflictMessage: undefined
                            }
                          : current
                      );
                    }}
                  >
                    {platform.label}
                  </button>
                );
              })}
            </div>

            {pendingManualImport.conflictMessage ? (
              <div className="inline-notice inline-notice-warning manual-import-conflict">
                {pendingManualImport.conflictMessage}
              </div>
            ) : null}

            {manualImportError ? (
              <p className="form-message error-message manual-import-error">{manualImportError}</p>
            ) : null}

            <div className="manual-import-modal-actions">
              <button className="ghost-button" disabled={manualImporting} onClick={closeManualImportModal}>
                Cancel
              </button>
              <button
                className="primary-button"
                disabled={manualImporting || !manualImportPlatformId}
                onClick={() => void importPendingManualRom(Boolean(pendingManualImport.conflictMessage))}
              >
                {manualImporting
                  ? "Importing..."
                  : pendingManualImport.conflictMessage
                    ? "Overwrite"
                    : "Import"}
              </button>
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

function getPathFileName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

function isSupportedManualImportFile(fileName: string): boolean {
  const extension = fileName.split(".").pop()?.toLowerCase();
  return Boolean(extension && SUPPORTED_MANUAL_IMPORT_EXTENSIONS.has(extension));
}

function formatDuplicateImportMessage(rawMessage: string): string {
  const conflicts = rawMessage
    .slice(DUPLICATE_IMPORT_PREFIX.length)
    .split(/\r?\n/)
    .map((entry) => entry.trim())
    .filter(Boolean);

  if (!conflicts.length) {
    return "A ROM with this file name already exists. Overwrite it?";
  }

  if (conflicts.length === 1) {
    return `A ROM already exists at ${conflicts[0]}. Overwrite it?`;
  }

  return `These ROMs already exist:\n${conflicts.join("\n")}\nOverwrite them?`;
}

function parseSaveConflictError(reason: unknown): SaveConflictStatus | null {
  const message = reason instanceof Error ? reason.message : String(reason);
  const prefix = "SAVE_CONFLICT:";
  const start = message.indexOf(prefix);

  if (start === -1) {
    return null;
  }

  try {
    return JSON.parse(message.slice(start + prefix.length)) as SaveConflictStatus;
  } catch {
    return null;
  }
}

function formatConflictTimestamp(value: number): string {
  return new Date(value).toLocaleString("fr-FR", {
    dateStyle: "short",
    timeStyle: "short"
  });
}

function formatConflictIsoTimestamp(value?: string | null): string {
  if (!value) {
    return "Unknown";
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return date.toLocaleString("fr-FR", {
    dateStyle: "short",
    timeStyle: "short"
  });
}

function ResourceIndicators({ summary }: { summary?: EmulatorResourceSummary }) {
  if (!summary?.requirements.length) {
    return null;
  }

  return (
    <span className="resource-indicators" aria-label={formatResourceSummaryLine(summary)}>
      {summary.statuses.map((status) => (
        <span
          key={status.id}
          className={`resource-dot resource-dot-${status.state}`}
          title={`${status.label}: ${status.message}`}
          aria-label={`${status.label}: ${status.state}`}
        >
          {resourceDotLabel(status.kind)}
        </span>
      ))}
    </span>
  );
}

function resourceDotLabel(kind: string): string {
  switch (kind) {
    case "bios":
      return "B";
    case "firmware":
      return "F";
    case "keys":
      return "K";
    default:
      return "?";
  }
}

function formatResourceSummaryLine(summary: EmulatorResourceSummary): string {
  if (!summary.requirements.length) {
    return "No required resources";
  }

  const problemStatuses = summary.statuses.filter(
    (status) => status.required && status.state !== "valid"
  );

  if (!problemStatuses.length) {
    return `Resources ready: ${summary.statuses.map((status) => status.label).join(", ")}`;
  }

  return `Resources missing: ${problemStatuses
    .map((status) => status.label)
    .join(", ")}`;
}

function formatResourceBlockMessage(summary: EmulatorResourceSummary): string {
  const problemStatuses = summary.statuses.filter(
    (status) => status.required && status.state !== "valid"
  );

  if (!problemStatuses.length) {
    return `${summary.emulatorName} resources are ready.`;
  }

  return `${summary.emulatorName} cannot launch yet: ${problemStatuses
    .map((status) => `${status.label} ${status.state}`)
    .join(", ")}.`;
}

function resourceProgressKey(emulatorId: string, resourceId: string): string {
  return `${emulatorId}:${resourceId}`;
}

function getActiveResourceProgress(
  progressByKey: Record<string, EmulatorResourceProgressPayload>,
  emulatorId: string
): EmulatorResourceProgressPayload | null {
  return (
    Object.entries(progressByKey)
      .filter(([key]) => key.startsWith(`${emulatorId}:`))
      .map(([, value]) => value)
      .find((value) => value.stage !== "complete") ?? null
  );
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
      decorations: false,
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
