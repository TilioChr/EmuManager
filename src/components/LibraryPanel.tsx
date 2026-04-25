import { useEffect, useMemo, useState } from "react";
import {
  getLatestRommSave,
  getRommGames,
  resolveGameLocalFileName,
  type RommGame,
  type RommSaveEntry,
  type RommSession
} from "../lib/romm";
import type { LocalRomEntry, SaveSyncStatus } from "../types";
import CollapsiblePanel from "./CollapsiblePanel";

interface LibraryPanelProps {
  session: RommSession | null;
  localRoms: LocalRomEntry[];
  saveSyncStatuses: Record<string, SaveSyncStatus>;
  onDownloadGame: (game: RommGame) => Promise<void>;
  onLaunchLocalRom: (romPath: string) => Promise<void>;
  downloadProgressById?: Record<string, number>;
  notice?: {
    type: "success" | "error" | "info";
    message: string;
  } | null;
}

interface LibraryItem {
  id: string;
  title: string;
  platform: string;
  fileName: string;
  downloaded: boolean;
  localPath?: string;
  rommId?: string;
  localSaveStatus?: SaveSyncStatus;
  remoteGame?: RommGame;
}

export default function LibraryPanel({
  session,
  localRoms,
  saveSyncStatuses,
  onDownloadGame,
  onLaunchLocalRom,
  downloadProgressById = {},
  notice = null
}: LibraryPanelProps) {
  const [games, setGames] = useState<RommGame[]>([]);
  const [remoteSaveStatuses, setRemoteSaveStatuses] = useState<Record<string, RommSaveEntry | null>>({});
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [platformFilter, setPlatformFilter] = useState("All");

  useEffect(() => {
    if (!session) {
      setGames([]);
      setRemoteSaveStatuses({});
    }
  }, [session]);

  const localByRommId = useMemo(() => {
    const map = new Map<string, LocalRomEntry>();
    for (const rom of localRoms) {
      const rommId = saveSyncStatuses[rom.filePath]?.rommId;

      if (rommId && !map.has(rommId)) {
        map.set(rommId, rom);
      }
    }
    return map;
  }, [localRoms, saveSyncStatuses]);

  const mergedItems = useMemo<LibraryItem[]>(() => {
    const remoteItems = games.map((game) => {
      const fileName = resolveGameLocalFileName(game);
      const platform = getRemotePlatform(game);
      const rommId = String(game.id);
      const localMatch = localByRommId.get(rommId);

      return {
        id: `remote-${game.id}`,
        title: game.name,
        platform,
        fileName,
        downloaded: Boolean(localMatch),
        localPath: localMatch?.filePath,
        rommId,
        localSaveStatus: localMatch ? saveSyncStatuses[localMatch.filePath] : undefined,
        remoteGame: game
      };
    });

    const matchedLocalPaths = new Set(
      remoteItems
        .map((item) => item.localPath)
        .filter((localPath): localPath is string => typeof localPath === "string")
    );

    const activeDownloadItems = remoteItems.filter((item) =>
      item.rommId ? hasDownloadProgress(downloadProgressById, item.rommId) : false
    );

    const localOnlyItems = localRoms
      .filter(
        (rom) =>
          !matchedLocalPaths.has(rom.filePath) &&
          !activeDownloadItems.some((item) => isLocalRomForRemoteItem(rom, item))
      )
      .map((rom) => ({
        id: `local-${rom.filePath}`,
        title: stripExtension(rom.fileName),
        platform: rom.platformGuess,
        fileName: rom.fileName,
        downloaded: true,
        localPath: rom.filePath,
        rommId: saveSyncStatuses[rom.filePath]?.rommId ?? undefined,
        localSaveStatus: saveSyncStatuses[rom.filePath]
      }));

    return [...remoteItems, ...localOnlyItems].sort((left, right) => {
      if (left.downloaded !== right.downloaded) {
        return left.downloaded ? -1 : 1;
      }

      const platformCompare = left.platform.localeCompare(right.platform, "en", {
        sensitivity: "base"
      });
      if (platformCompare !== 0) {
        return platformCompare;
      }

      return left.title.localeCompare(right.title, "en", { sensitivity: "base" });
    });
  }, [
    downloadProgressById,
    games,
    localByRommId,
    localRoms,
    saveSyncStatuses
  ]);

  useEffect(() => {
    if (!session) {
      return;
    }

    const itemsWithRemoteSave = mergedItems.filter(
      (item): item is LibraryItem & { rommId: string; localSaveStatus: SaveSyncStatus } =>
        typeof item.rommId === "string" &&
        item.rommId.length > 0 &&
        item.localSaveStatus !== undefined &&
        Boolean(resolveSlotName(item.localSaveStatus.emulatorId))
    );

    if (!itemsWithRemoteSave.length) {
      setRemoteSaveStatuses({});
      return;
    }

    let cancelled = false;

    const loadRemoteStatuses = async () => {
      const entries = await Promise.all(
        itemsWithRemoteSave.map(async (item) => [
            item.rommId,
            await getLatestRommSave(
              session,
              item.rommId,
              item.localSaveStatus.emulatorId,
              resolveSlotName(item.localSaveStatus.emulatorId)!
            )
          ] as const)
      );

      if (!cancelled) {
        setRemoteSaveStatuses(Object.fromEntries(entries));
      }
    };

    void loadRemoteStatuses();

    return () => {
      cancelled = true;
    };
  }, [mergedItems, session]);

  const platformOptions = useMemo(() => {
    const unique = Array.from(new Set(mergedItems.map((item) => item.platform))).sort((a, b) =>
      a.localeCompare(b, "en", { sensitivity: "base" })
    );
    return ["All", ...unique];
  }, [mergedItems]);

  const filteredItems = useMemo(() => {
    const needle = search.trim().toLowerCase();

    return mergedItems.filter((item) => {
      const matchesSearch =
        !needle ||
        [item.title, item.platform, item.fileName].some((value) =>
          value.toLowerCase().includes(needle)
        );

      const matchesPlatform = platformFilter === "All" || item.platform === platformFilter;

      return matchesSearch && matchesPlatform;
    });
  }, [mergedItems, platformFilter, search]);

  const loadGames = async () => {
    if (!session) {
      setError("Offline mode: only already downloaded ROMs are available.");
      return;
    }

    try {
      setLoading(true);
      setError(null);
      const roms = await getRommGames(session);
      setGames(roms);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Failed to load RomM library.");
    } finally {
      setLoading(false);
    }
  };

  return (
    <CollapsiblePanel
      eyebrow="Library"
      actions={
        <button
          className="primary-button compact-button"
          onClick={() => void loadGames()}
          disabled={loading || !session}
        >
          {loading ? "Loading..." : session ? "Load RomM" : "Offline"}
        </button>
      }
    >
      <div className="library-toolbar library-toolbar-grid">
        <input
          value={search}
          onChange={(event) => setSearch(event.target.value)}
          placeholder="Search a game..."
          disabled={!mergedItems.length}
        />

        <select
          value={platformFilter}
          onChange={(event) => setPlatformFilter(event.target.value)}
          disabled={!mergedItems.length}
        >
          {platformOptions.map((option) => (
            <option key={option} value={option}>
              {option}
            </option>
          ))}
        </select>
      </div>

      {notice ? (
        <div
          className={`inline-notice ${
            notice.type === "error"
              ? "inline-notice-error"
              : notice.type === "success"
                ? "inline-notice-success"
                : "inline-notice-info"
          }`}
        >
          {notice.message}
        </div>
      ) : null}

      {error ? <p className="form-message error-message">{error}</p> : null}

      <div className="library-list">
        {filteredItems.map((item) => {
          const downloadId = item.remoteGame ? String(item.remoteGame.id) : null;
          const downloadPercent = downloadId ? downloadProgressById[downloadId] : undefined;
          const isDownloading = typeof downloadPercent === "number";
          const visibleDownloadPercent = isDownloading
            ? Math.min(100, Math.max(0, downloadPercent))
            : 0;

          return (
            <div key={item.id} className="library-item">
              <div>
                <strong>{item.title}</strong>
                <p>
                  {item.platform}
                  {item.downloaded ? " • downloaded" : ""}
                </p>
                <small>{item.fileName}</small>
                <div className="library-save-meta">
                  <small>
                    Local save:{" "}
                    {item.localSaveStatus?.hasLocalSave && item.localSaveStatus.localSaveUpdatedAtMs
                      ? formatLocalTimestamp(item.localSaveStatus.localSaveUpdatedAtMs)
                      : "none"}
                  </small>
                  <small>
                    RomM save:{" "}
                    {formatRemoteSaveValue(
                      session,
                      item.localSaveStatus?.lastKnownRemoteSaveAt ?? null,
                      item.rommId ? remoteSaveStatuses[item.rommId] ?? null : null
                    )}
                  </small>
                </div>
              </div>

              <div className="library-actions">
                {item.downloaded && item.localPath ? (
                  <button
                    className="primary-button compact-button"
                    onClick={() => void onLaunchLocalRom(item.localPath!)}
                  >
                    Launch
                  </button>
                ) : null}

                {!item.downloaded && item.remoteGame ? (
                  <button
                    className="download-button"
                    disabled={isDownloading}
                    onClick={() => void onDownloadGame(item.remoteGame!)}
                    style={
                      isDownloading
                        ? {
                            background: `linear-gradient(
                              90deg,
                              #8fb1ff 0%,
                              #8fb1ff ${visibleDownloadPercent}%,
                              #5f7df0 ${visibleDownloadPercent}%,
                              #5f7df0 100%
                            )`
                          }
                        : undefined
                    }
                  >
                    <span className="download-button-label">
                      {isDownloading
                        ? `Downloading... ${Math.round(visibleDownloadPercent)}%`
                        : "Download"}
                    </span>
                  </button>
                ) : null}
              </div>
            </div>
          );
        })}

        {!loading && !filteredItems.length ? (
          <div className="empty-state">
            <p>No games displayed</p>
          </div>
        ) : null}
      </div>
    </CollapsiblePanel>
  );
}

function normalizeFileNameKey(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[<>:"/\\|?*\x00-\x1f]/g, "_");
}

function stripExtension(fileName: string): string {
  return fileName.replace(/\.[^.]+$/, "");
}

function getRemotePlatform(game: RommGame): string {
  return game.platform_name ?? game.platform_display_name ?? "Unknown platform";
}

function isLocalRomForRemoteItem(rom: LocalRomEntry, item: LibraryItem): boolean {
  return (
    normalizeFileNameKey(rom.fileName) === normalizeFileNameKey(item.fileName) &&
    arePlatformsCompatible(item.platform, rom.platformGuess)
  );
}

function hasDownloadProgress(
  downloadProgressById: Record<string, number>,
  downloadId: string
): boolean {
  return Object.prototype.hasOwnProperty.call(downloadProgressById, downloadId);
}

function arePlatformsCompatible(remotePlatform: string, localPlatform: string): boolean {
  const remoteTokens = platformTokens(remotePlatform);
  const localTokens = platformTokens(localPlatform);

  if (!remoteTokens.size || !localTokens.size) {
    return normalizePlatformName(remotePlatform) === normalizePlatformName(localPlatform);
  }

  for (const token of remoteTokens) {
    if (localTokens.has(token)) {
      return true;
    }
  }

  return false;
}

function platformTokens(value: string): Set<string> {
  const normalized = normalizePlatformName(value);
  const tokens = new Set<string>();

  if (/\b3ds\b/.test(normalized)) {
    tokens.add("3ds");
  }
  if (/\bnds\b|\bnintendo ds\b|\bds\b/.test(normalized)) {
    tokens.add("ds");
  }
  if (/\bwii\b/.test(normalized)) {
    tokens.add("wii");
  }
  if (/\bgamecube\b|\bgame cube\b|\bgc\b/.test(normalized)) {
    tokens.add("gamecube");
  }
  if (/\bswitch\b|\bnsw\b/.test(normalized)) {
    tokens.add("switch");
  }
  if (/\bps2\b|\bplaystation 2\b/.test(normalized)) {
    tokens.add("ps2");
  }
  if (/\bpsp\b/.test(normalized)) {
    tokens.add("psp");
  }
  if (/\bps1\b|\bpsx\b|\bplaystation 1\b/.test(normalized) || normalized === "playstation") {
    tokens.add("ps1");
  }
  if (/\bgba\b|\bgame boy advance\b/.test(normalized)) {
    tokens.add("gba");
  }
  if (/\bgbc\b|\bgame boy color\b/.test(normalized)) {
    tokens.add("gbc");
  }
  if (
    /\bgb\b|\bgame boy\b/.test(normalized) &&
    !/\bgame boy advance\b|\bgame boy color\b/.test(normalized)
  ) {
    tokens.add("gb");
  }
  if (/\bnes\b/.test(normalized)) {
    tokens.add("nes");
  }
  if (/\bsnes\b|\bsfc\b|\bsuper nintendo\b/.test(normalized)) {
    tokens.add("snes");
  }
  if (/\bn64\b|\bnintendo 64\b/.test(normalized)) {
    tokens.add("n64");
  }

  return tokens;
}

function normalizePlatformName(value: string): string {
  return value
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, " ")
    .trim();
}

function formatLocalTimestamp(value: number): string {
  return new Date(value).toLocaleString("fr-FR", {
    dateStyle: "short",
    timeStyle: "short"
  });
}

function formatRemoteSaveValue(
  session: RommSession | null,
  fallbackRemoteDate: string | null,
  remoteSave: RommSaveEntry | null
): string {
  if (remoteSave?.updated_at) {
    return formatIsoTimestamp(remoteSave.updated_at);
  }

  if (fallbackRemoteDate) {
    return `${formatIsoTimestamp(fallbackRemoteDate)}${session ? "" : " (cached)"}`;
  }

  return session ? "none" : "offline";
}

function formatIsoTimestamp(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return date.toLocaleString("fr-FR", {
    dateStyle: "short",
    timeStyle: "short"
  });
}

function resolveSlotName(emulatorId: string): string | null {
  switch (emulatorId) {
    case "dolphin":
      return "EmuManager Dolphin";
    case "melonds":
      return "EmuManager melonDS";
    case "azahar":
      return "EmuManager Azahar";
    case "eden":
      return "EmuManager Eden";
    case "pcsx2":
      return "EmuManager PCSX2";
    default:
      return null;
  }
}
