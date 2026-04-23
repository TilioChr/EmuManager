import { useEffect, useMemo, useState } from "react";
import {
  getLatestRommSave,
  getRommGames,
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
  downloadingGameId?: string | null;
  downloadPercent?: number;
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
  downloadingGameId = null,
  downloadPercent = 0,
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

  const localByFileName = useMemo(() => {
    const map = new Map<string, LocalRomEntry>();
    for (const rom of localRoms) {
      map.set(normalizeKey(rom.fileName), rom);
    }
    return map;
  }, [localRoms]);

  const mergedItems = useMemo<LibraryItem[]>(() => {
    const remoteItems = games.map((game) => {
      const fileName = getRemoteFileName(game) || `${game.name}.rom`;
      const localMatch = localByFileName.get(normalizeKey(fileName));

      return {
        id: `remote-${game.id}`,
        title: game.name,
        platform: game.platform_name ?? game.platform_display_name ?? "Unknown platform",
        fileName,
        downloaded: Boolean(localMatch),
        localPath: localMatch?.filePath,
        rommId: String(game.id),
        localSaveStatus: localMatch ? saveSyncStatuses[localMatch.filePath] : undefined,
        remoteGame: game
      };
    });

    const remoteFileNames = new Set(remoteItems.map((item) => normalizeKey(item.fileName)));

    const localOnlyItems = localRoms
      .filter((rom) => !remoteFileNames.has(normalizeKey(rom.fileName)))
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
  }, [games, localByFileName, localRoms, saveSyncStatuses]);

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
          const isDownloading = item.remoteGame && downloadingGameId === String(item.remoteGame.id);

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
                              #8fb1ff ${downloadPercent}%,
                              #5f7df0 ${downloadPercent}%,
                              #5f7df0 100%
                            )`
                          }
                        : undefined
                    }
                  >
                    <span className="download-button-label">
                      {isDownloading
                        ? `Downloading... ${Math.round(downloadPercent)}%`
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

function normalizeKey(value: string): string {
  return value.trim().toLowerCase();
}

function stripExtension(fileName: string): string {
  return fileName.replace(/\.[^.]+$/, "");
}

function getRemoteFileName(game: RommGame): string | null {
  return (
    game.file_name ??
    game.fs_name ??
    (Array.isArray(game.files)
      ? (game.files[0]?.file_name as string | undefined) ??
        (game.files[0]?.fs_name as string | undefined) ??
        (game.files[0]?.name as string | undefined)
      : undefined) ??
    null
  );
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
    default:
      return null;
  }
}
