import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  getLatestRommSave,
  getRommGameDetails,
  getRommGameScreenshots,
  getRommGames,
  resolveGameLocalFileName,
  type RommGame,
  type RommSaveEntry,
  type RommSession
} from "../lib/romm";
import { debugLog } from "../lib/debugLog";
import type { LocalRomEntry, SaveSyncStatus } from "../types";
import CollapsiblePanel from "./CollapsiblePanel";

type LibraryViewMode = "list" | "grid";

interface LibraryPanelProps {
  root: string;
  session: RommSession | null;
  localRoms: LocalRomEntry[];
  saveSyncStatuses: Record<string, SaveSyncStatus>;
  pinnedItemIds: string[];
  onPinnedItemIdsChange: (pinnedItemIds: string[]) => Promise<void>;
  onDownloadGame: (game: RommGame) => Promise<void>;
  onLaunchLocalRom: (romPath: string, localOnly?: boolean) => Promise<void>;
  onRequestDeleteLocalRom: (romPath: string, title: string) => void;
  downloadProgressById?: Record<string, number>;
  runningRomPaths?: Record<string, boolean>;
  notice?: {
    type: "success" | "error" | "info";
    message: string;
  } | null;
  manualImportDragActive?: boolean;
  pendingManualImportFileName?: string | null;
}

interface LibraryItem {
  id: string;
  title: string;
  platform: string;
  fileName: string;
  fileSizeBytes?: number;
  downloaded: boolean;
  localPath?: string;
  rommId?: string;
  localSaveStatus?: SaveSyncStatus;
  remoteGame?: RommGame;
  localOnly: boolean;
}

interface RommMediaCacheResult {
  mediaId: string;
  mediaKind: string;
  filePath: string;
  mimeType: string;
  dataUrl: string;
}

interface DetailRow {
  label: string;
  value: string;
}

export default function LibraryPanel({
  root,
  session,
  localRoms,
  saveSyncStatuses,
  pinnedItemIds,
  onPinnedItemIdsChange,
  onDownloadGame,
  onLaunchLocalRom,
  onRequestDeleteLocalRom,
  downloadProgressById = {},
  runningRomPaths = {},
  notice = null,
  manualImportDragActive = false,
  pendingManualImportFileName = null
}: LibraryPanelProps) {
  const [games, setGames] = useState<RommGame[]>([]);
  const [cachedGamesByRommId, setCachedGamesByRommId] = useState<Record<string, RommGame>>({});
  const [remoteSaveStatuses, setRemoteSaveStatuses] = useState<Record<string, RommSaveEntry | null>>({});
  const [coverDataById, setCoverDataById] = useState<Record<string, string>>({});
  const coverDataRef = useRef(coverDataById);
  const [screenshotDataByUrl, setScreenshotDataByUrl] = useState<Record<string, string>>({});
  const screenshotDataRef = useRef(screenshotDataByUrl);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [platformFilter, setPlatformFilter] = useState("All");
  const [viewMode, setViewMode] = useState<LibraryViewMode>("grid");
  const [selectedDetailItem, setSelectedDetailItem] = useState<LibraryItem | null>(null);
  const [detailGame, setDetailGame] = useState<RommGame | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);

  useEffect(() => {
    coverDataRef.current = coverDataById;
  }, [coverDataById]);

  useEffect(() => {
    screenshotDataRef.current = screenshotDataByUrl;
  }, [screenshotDataByUrl]);

  useEffect(() => {
    if (!session) {
      setGames([]);
      setRemoteSaveStatuses({});
      setSelectedDetailItem(null);
      setDetailGame(null);
    }
  }, [session]);

  const localRommIds = useMemo(
    () =>
      unique(
        localRoms
          .map((rom) => saveSyncStatuses[rom.filePath]?.rommId)
          .filter((rommId): rommId is string => typeof rommId === "string" && rommId.length > 0)
      ),
    [localRoms, saveSyncStatuses]
  );

  useEffect(() => {
    if (!localRommIds.length) {
      return;
    }

    let cancelled = false;

    const loadCachedMetadata = async () => {
      try {
        const cachedGames = await loadCachedRommGameMetadata(root, localRommIds);
        if (!cancelled) {
          setCachedGamesByRommId((previous) => ({
            ...previous,
            ...mapGamesByRommId(cachedGames)
          }));
        }
      } catch (reason) {
        void debugLog("warning", "library", "Cached RomM metadata load failed", reason);
      }
    };

    void loadCachedMetadata();

    return () => {
      cancelled = true;
    };
  }, [localRommIds, root]);

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
        fileSizeBytes: localMatch?.fileSizeBytes ?? game.fs_size_bytes,
        downloaded: Boolean(localMatch),
        localPath: localMatch?.filePath,
        rommId,
        localSaveStatus: localMatch ? saveSyncStatuses[localMatch.filePath] : undefined,
        remoteGame: game,
        localOnly: false
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
        rom,
        rommId: saveSyncStatuses[rom.filePath]?.rommId ?? undefined
      }))
      .map(({ rom, rommId }) => {
        const cachedGame = rommId ? cachedGamesByRommId[rommId] : undefined;

        return {
          id: `local-${rom.filePath}`,
          title: cachedGame?.name ?? stripExtension(rom.fileName),
          platform: cachedGame ? getRemotePlatform(cachedGame) : rom.platformGuess,
          fileName: rom.fileName,
          fileSizeBytes: rom.fileSizeBytes,
          downloaded: true,
          localPath: rom.filePath,
          rommId,
          localSaveStatus: saveSyncStatuses[rom.filePath],
          remoteGame: cachedGame,
          localOnly: !rommId
        };
      });

    return [...remoteItems, ...localOnlyItems].sort((left, right) => {
      const leftPinned = pinRank(pinnedItemIds, left.id);
      const rightPinned = pinRank(pinnedItemIds, right.id);
      if (leftPinned !== rightPinned) {
        return leftPinned - rightPinned;
      }

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
    cachedGamesByRommId,
    games,
    localByRommId,
    localRoms,
    pinnedItemIds,
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
      try {
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
      } catch (reason) {
        void debugLog("warning", "library", "Remote save status refresh failed", reason);
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

  useEffect(() => {
    let cancelled = false;

    const loadCachedCovers = async () => {
      for (const item of filteredItems.slice(0, 96)) {
        if (cancelled) {
          return;
        }

        if (!item.rommId || coverDataRef.current[item.id]) {
          continue;
        }

        try {
          const result = await readCachedRommMedia(root, {
            mediaId: `${item.rommId}-cover`,
            mediaKind: "cover"
          });

          if (!cancelled && result) {
            setCoverDataById((previous) => ({
              ...previous,
              [item.id]: result.dataUrl
            }));
          }
        } catch (reason) {
          void debugLog("warning", "library", "Cached cover load failed", {
            itemId: item.id,
            reason: reason instanceof Error ? reason.message : String(reason)
          });
        }
      }
    };

    void loadCachedCovers();

    return () => {
      cancelled = true;
    };
  }, [filteredItems, root]);

  useEffect(() => {
    if (!session) {
      return;
    }

    let cancelled = false;

    const cacheVisibleCovers = async () => {
      for (const item of filteredItems.slice(0, 96)) {
        if (cancelled) {
          return;
        }

        if (!item.remoteGame || !item.rommId || coverDataRef.current[item.id]) {
          continue;
        }

        const coverUrls = resolveGameCoverUrls(session, item.remoteGame);
        if (!coverUrls.length) {
          continue;
        }

        let lastError: unknown = null;

        for (const coverUrl of coverUrls) {
          try {
            const result = await cacheRommMedia(root, session, {
              mediaId: `${item.rommId}-cover`,
              mediaKind: "cover",
              url: coverUrl
            });

            if (!cancelled) {
              setCoverDataById((previous) => ({
                ...previous,
                [item.id]: result.dataUrl
              }));
            }
            lastError = null;
            break;
          } catch (reason) {
            lastError = reason;
          }
        }

        if (lastError) {
          void debugLog("warning", "library", "Cover cache failed", {
            itemId: item.id,
            reason: lastError instanceof Error ? lastError.message : String(lastError)
          });
        }
      }
    };

    void cacheVisibleCovers();

    return () => {
      cancelled = true;
    };
  }, [filteredItems, root, session]);

  const detailSource = detailGame ?? selectedDetailItem?.remoteGame ?? null;
  const screenshotUrls = useMemo(
    () => {
      const liveUrls = session && detailSource ? resolveGameScreenshotUrls(session, detailSource) : [];
      if (liveUrls.length) {
        return liveUrls;
      }

      return selectedDetailItem?.rommId ? cachedScreenshotKeys(selectedDetailItem.rommId) : [];
    },
    [detailSource, selectedDetailItem?.rommId, session]
  );

  useEffect(() => {
    if (!selectedDetailItem?.rommId) {
      return;
    }

    let cancelled = false;

    const loadCachedScreenshots = async () => {
      for (let index = 1; index <= 8; index += 1) {
        if (cancelled) {
          return;
        }

        const key = cachedScreenshotKey(selectedDetailItem.rommId!, index);
        if (screenshotDataRef.current[key]) {
          continue;
        }

        try {
          const result = await readCachedRommMedia(root, {
            mediaId: `${selectedDetailItem.rommId}-screenshot-${index}`,
            mediaKind: "screenshot"
          });

          if (!cancelled && result) {
            setScreenshotDataByUrl((previous) => ({
              ...previous,
              [key]: result.dataUrl
            }));
          }
        } catch (reason) {
          void debugLog("warning", "library", "Cached screenshot load failed", {
            itemId: selectedDetailItem.id,
            index,
            reason: reason instanceof Error ? reason.message : String(reason)
          });
        }
      }
    };

    void loadCachedScreenshots();

    return () => {
      cancelled = true;
    };
  }, [root, selectedDetailItem]);

  useEffect(() => {
    if (!session || !selectedDetailItem?.rommId || !screenshotUrls.length) {
      return;
    }

    let cancelled = false;

    const cacheScreenshots = async () => {
      for (const [index, url] of screenshotUrls.entries()) {
        if (cancelled) {
          return;
        }

        if (screenshotDataRef.current[url]) {
          continue;
        }

        try {
          const result = await cacheRommMedia(root, session, {
            mediaId: `${selectedDetailItem.rommId}-screenshot-${index + 1}`,
            mediaKind: "screenshot",
            url
          });

          if (!cancelled) {
            setScreenshotDataByUrl((previous) => ({
              ...previous,
              [url]: result.dataUrl
            }));
          }
        } catch (reason) {
          void debugLog("warning", "library", "Screenshot cache failed", {
            itemId: selectedDetailItem.id,
            url,
            reason: reason instanceof Error ? reason.message : String(reason)
          });
        }
      }
    };

    void cacheScreenshots();

    return () => {
      cancelled = true;
    };
  }, [root, screenshotUrls, selectedDetailItem, session]);

  const loadGames = async () => {
    if (!session) {
      setError("Offline mode: only already downloaded ROMs are available.");
      void debugLog("warning", "library", "RomM library load skipped in offline mode");
      return;
    }

    try {
      setLoading(true);
      setError(null);
      void debugLog("info", "library", "Loading RomM library", {
        baseUrl: session.baseUrl
      });
      const roms = await getRommGames(session);
      setGames(roms);
      setCachedGamesByRommId((previous) => ({
        ...previous,
        ...mapGamesByRommId(roms)
      }));
      void cacheRommGameMetadataBatch(root, roms);
      void debugLog("success", "library", "RomM library loaded", {
        count: roms.length
      });
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : "Failed to load RomM library.";
      setError(message);
      void debugLog("error", "library", "Failed to load RomM library", message);
    } finally {
      setLoading(false);
    }
  };

  const togglePinned = async (itemId: string) => {
    const next = pinnedItemIds.includes(itemId)
      ? pinnedItemIds.filter((id) => id !== itemId)
      : [itemId, ...pinnedItemIds];

    try {
      await onPinnedItemIdsChange(next);
    } catch (reason) {
      void debugLog("error", "library", "Could not persist pinned games", reason);
    }
  };

  const openDetails = (item: LibraryItem) => {
    setSelectedDetailItem(item);
    setDetailGame(item.remoteGame ?? null);
    setDetailError(null);

    if (!session || !item.rommId) {
      setDetailLoading(false);
      return;
    }

    setDetailLoading(true);
    void loadRommGameDetailsWithScreenshots(session, item.rommId)
      .then((game) => {
        setDetailGame(game);
        setCachedGamesByRommId((previous) => ({
          ...previous,
          [String(game.id)]: game
        }));
        void cacheRommGameMetadata(root, game);
      })
      .catch((reason) => {
        setDetailGame(item.remoteGame ?? null);
        setDetailError(reason instanceof Error ? reason.message : String(reason));
      })
      .finally(() => setDetailLoading(false));
  };

  const closeDetails = () => {
    setSelectedDetailItem(null);
    setDetailGame(null);
    setDetailError(null);
    setDetailLoading(false);
  };

  return (
    <>
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
        <div
          className={`manual-import-zone ${
            manualImportDragActive ? "manual-import-zone-active" : ""
          }`}
        >
          <span className="manual-import-zone-icon" aria-hidden="true" />
          <div>
            <strong>{manualImportDragActive ? "Release to import" : "Drop local ROM"}</strong>
            <p>{pendingManualImportFileName ?? "ROM, .zip, or .rar"}</p>
          </div>
        </div>

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

          <div className="library-view-toggle" role="group" aria-label="Library view">
            <button
              className={`icon-button ${viewMode === "list" ? "icon-button-active" : ""}`}
              type="button"
              title="List view"
              aria-label="List view"
              onClick={() => setViewMode("list")}
            >
              <span className="list-view-icon" aria-hidden="true" />
            </button>
            <button
              className={`icon-button ${viewMode === "grid" ? "icon-button-active" : ""}`}
              type="button"
              title="Grid view"
              aria-label="Grid view"
              onClick={() => setViewMode("grid")}
            >
              <span className="grid-view-icon" aria-hidden="true" />
            </button>
          </div>
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

        <div className={`library-list library-list-${viewMode}`}>
          {filteredItems.map((item) => {
            const downloadId = item.remoteGame ? String(item.remoteGame.id) : null;
            const downloadPercent = downloadId ? downloadProgressById[downloadId] : undefined;
            const isDownloading = typeof downloadPercent === "number";
            const visibleDownloadPercent = isDownloading
              ? Math.min(100, Math.max(0, downloadPercent))
              : 0;
            const isRunning = item.localPath ? Boolean(runningRomPaths[item.localPath]) : false;
            const isPinned = pinnedItemIds.includes(item.id);

            return (
              <article
                key={item.id}
                className={`library-item library-item-${viewMode} ${
                  isPinned ? "library-item-pinned" : ""
                }`}
              >
                <GameCover
                  src={coverDataById[item.id]}
                  title={item.title}
                  platform={item.platform}
                />

                <div className="library-item-main">
                  <div className="library-heading-row">
                    <div className="library-title-block">
                      <div className="library-title-row">
                        <strong>{item.title}</strong>
                        {isPinned ? <span className="pinned-badge">Pinned</span> : null}
                        {item.localOnly ? (
                          <span className="local-only-badge" title="Local-only ROM">
                            <span aria-hidden="true" />
                            Local
                          </span>
                        ) : null}
                      </div>
                      <p>
                        {item.platform}
                        {item.downloaded ? " - downloaded" : ""}
                      </p>
                    </div>

                    <div className="library-icon-actions">
                      <button
                        className={`icon-button ${isPinned ? "icon-button-active" : ""}`}
                        type="button"
                        title={isPinned ? "Unpin game" : "Pin game"}
                        aria-label={isPinned ? `Unpin ${item.title}` : `Pin ${item.title}`}
                        onClick={() => void togglePinned(item.id)}
                      >
                        <span className="pin-icon" aria-hidden="true" />
                      </button>
                      <button
                        className="icon-button"
                        type="button"
                        title="Game details"
                        aria-label={`Show details for ${item.title}`}
                        onClick={() => openDetails(item)}
                      >
                        <span className="info-icon" aria-hidden="true">
                          i
                        </span>
                      </button>
                    </div>
                  </div>

                  <small className="library-file-name">{item.fileName}</small>
                  <div className="library-save-meta">
                    <small>
                      Local save:{" "}
                      {item.localSaveStatus?.hasLocalSave && item.localSaveStatus.localSaveUpdatedAtMs
                        ? formatLocalTimestamp(item.localSaveStatus.localSaveUpdatedAtMs)
                        : "none"}
                    </small>
                    <small>
                      RomM sync:{" "}
                      {item.localOnly
                        ? "blocked (local only)"
                        : formatRemoteSaveValue(
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
                      className="primary-button compact-button launch-action-button"
                      disabled={isRunning}
                      aria-busy={isRunning}
                      onClick={() => void onLaunchLocalRom(item.localPath!, item.localOnly)}
                    >
                      <span className="button-content">
                        {isRunning ? (
                          <>
                            <span className="button-spinner" aria-hidden="true" />
                            Running...
                          </>
                        ) : (
                          "Launch"
                        )}
                      </span>
                    </button>
                  ) : null}
                  {item.downloaded && item.localPath ? (
                    <button
                      className="danger-button compact-button delete-rom-button"
                      disabled={isRunning}
                      onClick={() => onRequestDeleteLocalRom(item.localPath!, item.title)}
                    >
                      Delete
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
              </article>
            );
          })}

          {!loading && !filteredItems.length ? (
            <div className="empty-state">
              <p>No games displayed</p>
            </div>
          ) : null}
        </div>
      </CollapsiblePanel>

      {selectedDetailItem ? (
        <GameDetailModal
          item={selectedDetailItem}
          game={detailSource}
          coverSrc={coverDataById[selectedDetailItem.id]}
          screenshotUrls={screenshotUrls}
          screenshotDataByUrl={screenshotDataByUrl}
          loading={detailLoading}
          error={detailError}
          session={session}
          onClose={closeDetails}
        />
      ) : null}
    </>
  );
}

function GameCover({
  src,
  title,
  platform
}: {
  src?: string;
  title: string;
  platform: string;
}) {
  return (
    <div className="game-cover">
      {src ? (
        <img src={src} alt={`${title} cover`} loading="lazy" />
      ) : (
        <div className="game-cover-fallback" aria-hidden="true">
          <span>{coverInitials(title)}</span>
          <small>{platform}</small>
        </div>
      )}
    </div>
  );
}

function GameDetailModal({
  item,
  game,
  coverSrc,
  screenshotUrls,
  screenshotDataByUrl,
  loading,
  error,
  session,
  onClose
}: {
  item: LibraryItem;
  game: RommGame | null;
  coverSrc?: string;
  screenshotUrls: string[];
  screenshotDataByUrl: Record<string, string>;
  loading: boolean;
  error: string | null;
  session: RommSession | null;
  onClose: () => void;
}) {
  const description = game ? resolveGameDescription(game) : null;
  const metadataRows = game ? buildEssentialRows(game, item) : buildLocalEssentialRows(item);
  const visibleScreenshots = screenshotUrls.filter(
    (url) => !isCachedScreenshotKey(url) || Boolean(screenshotDataByUrl[url])
  );

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal game-detail-modal" onClick={(event) => event.stopPropagation()}>
        <div className="modal-header">
          <div>
            <h2 className="panel-title">{item.title}</h2>
            <p className="panel-subtitle">{item.platform}</p>
          </div>
          <button className="ghost-button" onClick={onClose}>
            Close
          </button>
        </div>

        <div className="game-detail-hero">
          <GameCover src={coverSrc} title={item.title} platform={item.platform} />
          <div>
            {loading ? <div className="inline-notice inline-notice-info">Loading RomM details...</div> : null}
            {error ? (
              <div className="inline-notice inline-notice-warning">
                RomM details could not be refreshed. Showing cached library data.
              </div>
            ) : null}
            <p className="game-description">
              {description ?? (item.localOnly ? "Local-only game with no RomM metadata yet." : "No description in RomM.")}
            </p>
            <div className="detail-badge-row">
              <span>{item.downloaded ? "Downloaded" : "Remote"}</span>
              <span>{session ? "RomM live" : "Cached"}</span>
            </div>
          </div>
        </div>

        <section className="game-detail-section">
          <h3>Screenshots</h3>
          {visibleScreenshots.length ? (
            <div className="screenshot-strip">
              {visibleScreenshots.map((url) => (
                <img
                  key={url}
                  src={screenshotDataByUrl[url] ?? url}
                  alt={`${item.title} screenshot`}
                  loading="lazy"
                />
              ))}
            </div>
          ) : (
            <p className="muted">No screenshots from RomM.</p>
          )}
        </section>

        <div className="game-detail-grid">
          <DetailSection title="Info" rows={metadataRows} empty="No metadata from RomM." />
        </div>
      </div>
    </div>
  );
}

function DetailSection({
  title,
  rows,
  empty
}: {
  title: string;
  rows: DetailRow[];
  empty: string;
}) {
  return (
    <section className="game-detail-section">
      <h3>{title}</h3>
      {rows.length ? (
        <dl className="detail-list">
          {rows.map((row) => (
            <div key={row.label}>
              <dt>{row.label}</dt>
              <dd>{row.value}</dd>
            </div>
          ))}
        </dl>
      ) : (
        <p className="muted">{empty}</p>
      )}
    </section>
  );
}

async function cacheRommMedia(
  root: string,
  session: RommSession,
  request: { mediaId: string; mediaKind: string; url: string }
): Promise<RommMediaCacheResult> {
  const bearerToken = shouldAuthenticateMediaRequest(session, request.url)
    ? session.token
    : undefined;

  return invoke<RommMediaCacheResult>("cache_romm_media_command", {
    root,
    request: {
      mediaId: request.mediaId,
      mediaKind: request.mediaKind,
      url: request.url,
      bearerToken
    }
  });
}

async function readCachedRommMedia(
  root: string,
  request: { mediaId: string; mediaKind: string }
): Promise<RommMediaCacheResult | null> {
  return invoke<RommMediaCacheResult | null>("read_romm_cached_media_command", {
    root,
    request
  });
}

async function cacheRommGameMetadata(root: string, game: RommGame): Promise<void> {
  await invoke("cache_romm_game_metadata_command", {
    root,
    game
  });
}

async function cacheRommGameMetadataBatch(root: string, games: RommGame[]): Promise<void> {
  for (const game of games) {
    try {
      await cacheRommGameMetadata(root, game);
    } catch (reason) {
      void debugLog("warning", "library", "RomM metadata cache write failed", {
        id: game.id,
        reason: reason instanceof Error ? reason.message : String(reason)
      });
    }
  }
}

async function loadCachedRommGameMetadata(root: string, rommIds: string[]): Promise<RommGame[]> {
  return invoke<RommGame[]>("load_romm_game_metadata_command", {
    root,
    rommIds
  });
}

async function loadRommGameDetailsWithScreenshots(
  session: RommSession,
  rommId: string
): Promise<RommGame> {
  const [game, screenshots] = await Promise.all([
    getRommGameDetails(session, rommId),
    getRommGameScreenshots(session, rommId).catch((reason) => {
      void debugLog("warning", "library", "RomM screenshot assets load failed", {
        rommId,
        reason: reason instanceof Error ? reason.message : String(reason)
      });
      return [];
    })
  ]);

  return screenshots.length ? { ...game, screenshots } : game;
}

function mapGamesByRommId(games: RommGame[]): Record<string, RommGame> {
  return Object.fromEntries(games.map((game) => [String(game.id), game]));
}

function shouldAuthenticateMediaRequest(session: RommSession, url: string): boolean {
  try {
    return new URL(url).origin === new URL(session.baseUrl).origin;
  } catch {
    return false;
  }
}

function pinRank(pinnedItemIds: string[], itemId: string): number {
  const index = pinnedItemIds.indexOf(itemId);
  return index >= 0 ? index : Number.MAX_SAFE_INTEGER;
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

function coverInitials(title: string): string {
  const words = title
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2);
  const initials = words.map((word) => word[0]?.toUpperCase() ?? "").join("");
  return initials || "GM";
}

function cachedScreenshotKey(rommId: string, index: number): string {
  return `cached-screenshot:${rommId}:${index}`;
}

function cachedScreenshotKeys(rommId: string): string[] {
  return Array.from({ length: 8 }, (_, index) => cachedScreenshotKey(rommId, index + 1));
}

function isCachedScreenshotKey(value: string): boolean {
  return value.startsWith("cached-screenshot:");
}

function resolveGameCoverUrls(session: RommSession, game: RommGame): string[] {
  return resolveMediaUrls(session, game, [
    "cover_url",
    "coverUrl",
    "cover",
    "cover_path",
    "coverPath",
    "boxart",
    "box_art",
    "thumbnail",
    "thumbnail_url",
    "thumbnailUrl",
    "image_url",
    "imageUrl",
    "url_cover",
    "path_cover"
  ]);
}

function resolveGameScreenshotUrls(session: RommSession, game: RommGame): string[] {
  return resolveMediaUrls(session, game, [
    "screenshot",
    "screenshots",
    "screen_shots",
    "screenshot_url",
    "screenshotUrl",
    "screenshot_urls",
    "screenshotUrls",
    "screenshots_urls",
    "screenshotsUrls",
    "url_screenshot",
    "urlScreenshot",
    "url_screenshots",
    "urlScreenshots",
    "path_screenshot",
    "pathScreenshot",
    "path_screenshots",
    "pathScreenshots",
    "merged_screenshots",
    "mergedScreenshots",
    "igdb_screenshots",
    "igdbScreenshots",
    "images",
    "image_urls",
    "imageUrls",
    "media",
    "assets"
  ]).slice(0, 8);
}

function resolveMediaUrls(session: RommSession, game: RommGame, keys: string[]): string[] {
  const records = gameRecords(game);
  const found: string[] = [];

  for (const record of records) {
    for (const key of keys) {
      if (key in record) {
        collectMediaStrings(record[key], found);
      }
    }
  }

  return unique(
    found
      .map((value) => absolutizeMediaUrl(session.baseUrl, value))
      .filter((value): value is string => Boolean(value))
  );
}

function collectMediaStrings(value: unknown, output: string[]) {
  if (typeof value === "string") {
    if (isLikelyImageValue(value)) {
      output.push(value);
    }
    return;
  }

  if (Array.isArray(value)) {
    for (const entry of value) {
      collectMediaStrings(entry, output);
    }
    return;
  }

  if (isRecord(value)) {
    const imageId = value.image_id ?? value.imageId;
    if (typeof imageId === "string" && imageId.trim()) {
      output.push(`https://images.igdb.com/igdb/image/upload/t_screenshot_big/${imageId.trim()}.jpg`);
    }

    for (const [key, entry] of Object.entries(value)) {
      const normalizedKey = key.toLowerCase();
      if (
        [
          "url",
          "path",
          "src",
          "image",
          "imageurl",
          "image_url",
          "thumbnail",
          "cover",
          "download_url",
          "downloadurl",
          "url_download",
          "asset_path",
          "assetpath",
          "file",
          "file_name",
          "filename"
        ].includes(normalizedKey) ||
        normalizedKey.includes("screenshot")
      ) {
        collectMediaStrings(entry, output);
      }
    }
  }
}

function isLikelyImageValue(value: string): boolean {
  return (
    /^https?:\/\//i.test(value) ||
    value.startsWith("//") ||
    value.startsWith("/") ||
    /\.(png|jpe?g|webp|gif)(\?|#|$)/i.test(value)
  );
}

function absolutizeMediaUrl(baseUrl: string, value: string): string | null {
  const trimmed = value.trim();
  if (!trimmed || trimmed.startsWith("data:")) {
    return null;
  }

  try {
    return new URL(trimmed, baseUrl.endsWith("/") ? baseUrl : `${baseUrl}/`).href;
  } catch {
    return null;
  }
}

function resolveGameDescription(game: RommGame): string | null {
  return readFirstString(game, [
    "summary",
    "description",
    "overview",
    "plot",
    "storyline",
    "description_raw"
  ]);
}

function buildEssentialRows(game: RommGame, item: LibraryItem): DetailRow[] {
  const rows: Array<[string, unknown]> = [
    ["Console", getRemotePlatform(game) || item.platform],
    ["Release date", formatReleaseDate(readFirstValue(game, ["release_date", "released", "first_release_date", "year"]))],
    ["Region", readFirstValue(game, ["region", "regions"])],
    ["Size", formatSize(readFirstValue(game, ["fs_size_bytes", "size"]) ?? item.fileSizeBytes)],
    ["Genre", readFirstValue(game, ["genres", "genre"])]
  ];

  return rowsToDetails(rows);
}

function buildLocalEssentialRows(item: LibraryItem): DetailRow[] {
  return rowsToDetails([
    ["Console", item.platform],
    ["Size", formatSize(item.fileSizeBytes)]
  ]);
}

function rowsToDetails(rows: Array<[string, unknown]>): DetailRow[] {
  return rows
    .map(([label, value]) => [label, formatDetailValue(value)] as const)
    .filter((row): row is readonly [string, string] => Boolean(row[1]))
    .map(([label, value]) => ({ label, value }));
}

function readFirstString(game: RommGame, keys: string[]): string | null {
  const value = readFirstValue(game, keys);
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function readFirstValue(game: RommGame, keys: string[]): unknown {
  for (const record of gameRecords(game)) {
    for (const key of keys) {
      if (key in record && record[key] !== undefined && record[key] !== null && record[key] !== "") {
        return record[key];
      }
    }
  }

  return null;
}

function gameRecords(game: RommGame): Record<string, unknown>[] {
  const records: Record<string, unknown>[] = [game as Record<string, unknown>];
  for (const key of [
    "metadata",
    "metadatum",
    "manual_metadata",
    "manualMetadata",
    "merged_metadata",
    "mergedMetadata",
    "igdb",
    "igdb_metadata",
    "igdbMetadata",
    "moby",
    "moby_metadata",
    "mobyMetadata",
    "ss",
    "sgdb",
    "ra_metadata",
    "raMetadata",
    "extra"
  ]) {
    const value = (game as Record<string, unknown>)[key];
    if (isRecord(value)) {
      records.push(value);
    }
  }
  return records;
}

function formatDetailValue(value: unknown): string | null {
  if (value === null || value === undefined || value === "") {
    return null;
  }

  if (typeof value === "number") {
    return String(value);
  }

  if (typeof value === "boolean") {
    return value ? "Yes" : "No";
  }

  if (typeof value === "string") {
    return formatMaybeDate(value);
  }

  if (Array.isArray(value)) {
    const formatted = value
      .map((entry) => formatDetailValue(entry))
      .filter((entry): entry is string => Boolean(entry));
    return formatted.length ? unique(formatted).join(", ") : null;
  }

  if (isRecord(value)) {
    const named = ["name", "title", "value", "slug"]
      .map((key) => value[key])
      .find((entry) => typeof entry === "string" && entry.trim());

    if (typeof named === "string") {
      return named;
    }

    return Object.entries(value)
      .slice(0, 4)
      .map(([key, entry]) => `${prettifyKey(key)}: ${formatDetailValue(entry) ?? "n/a"}`)
      .join(", ");
  }

  return String(value);
}

function formatMaybeDate(value: string): string {
  if (/^\d{4}-\d{2}-\d{2}/.test(value)) {
    return formatIsoTimestamp(value);
  }
  return value;
}

function formatReleaseDate(value: unknown): string | null {
  if (value === null || value === undefined || value === "") {
    return null;
  }

  if (typeof value === "number") {
    if (value >= 1900 && value <= 2100 && Number.isInteger(value)) {
      return String(value);
    }

    const timestampMs = value > 100000000000 ? value : value * 1000;
    const date = new Date(timestampMs);
    return Number.isNaN(date.getTime()) ? String(value) : formatDisplayDate(date);
  }

  if (typeof value === "string") {
    const trimmed = value.trim();
    if (!trimmed) {
      return null;
    }

    if (/^\d{4}$/.test(trimmed)) {
      return trimmed;
    }

    if (/^\d+$/.test(trimmed)) {
      return formatReleaseDate(Number(trimmed));
    }

    const date = new Date(trimmed);
    return Number.isNaN(date.getTime()) ? trimmed : formatDisplayDate(date);
  }

  return formatDetailValue(value);
}

function formatDisplayDate(date: Date): string {
  return date.toLocaleDateString("fr-FR", {
    year: "numeric",
    month: "short",
    day: "2-digit"
  });
}

function formatSize(value: unknown): string | null {
  if (typeof value === "number") {
    return formatBytes(value);
  }

  if (typeof value === "string" && value.trim()) {
    const numberValue = Number(value);
    if (Number.isFinite(numberValue)) {
      return formatBytes(numberValue);
    }
    return value.trim();
  }

  return null;
}

function formatBytes(value: number): string {
  if (!Number.isFinite(value) || value <= 0) {
    return "0 B";
  }

  const units = ["B", "KB", "MB", "GB", "TB"];
  const index = Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1);
  const amount = value / 1024 ** index;
  return `${amount.toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
}

function compactGameDetails(game: RommGame): Record<string, unknown> {
  const omitted = new Set(["cover", "screenshots", "files"]);
  return Object.fromEntries(
    Object.entries(game as Record<string, unknown>)
      .filter(([key, value]) => !omitted.has(key) && value !== null && value !== undefined && value !== "")
      .slice(0, 80)
  );
}

function prettifyKey(value: string): string {
  return value
    .replace(/[_-]+/g, " ")
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .replace(/\b\w/g, (letter) => letter.toUpperCase());
}

function unique(values: string[]): string[] {
  return Array.from(new Set(values));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
