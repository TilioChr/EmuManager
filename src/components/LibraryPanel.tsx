import { useEffect, useMemo, useState } from "react";
import { getRommGames, type RommGame, type RommSession } from "../lib/romm";
import type { LocalRomEntry } from "../types";
import CollapsiblePanel from "./CollapsiblePanel";

interface LibraryPanelProps {
  session: RommSession | null;
  localRoms: LocalRomEntry[];
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
  remoteGame?: RommGame;
}

export default function LibraryPanel({
  session,
  localRoms,
  onDownloadGame,
  onLaunchLocalRom,
  downloadingGameId = null,
  downloadPercent = 0,
  notice = null
}: LibraryPanelProps) {
  const [games, setGames] = useState<RommGame[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [platformFilter, setPlatformFilter] = useState("Toutes");

  useEffect(() => {
    if (!session) {
      setGames([]);
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
        platform: game.platform_name ?? game.platform_display_name ?? "Plateforme inconnue",
        fileName,
        downloaded: Boolean(localMatch),
        localPath: localMatch?.filePath,
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
        localPath: rom.filePath
      }));

    return [...remoteItems, ...localOnlyItems].sort((left, right) => {
      if (left.downloaded !== right.downloaded) {
        return left.downloaded ? -1 : 1;
      }

      const platformCompare = left.platform.localeCompare(right.platform, "fr", {
        sensitivity: "base"
      });
      if (platformCompare !== 0) {
        return platformCompare;
      }

      return left.title.localeCompare(right.title, "fr", { sensitivity: "base" });
    });
  }, [games, localByFileName, localRoms]);

  const platformOptions = useMemo(() => {
    const unique = Array.from(new Set(mergedItems.map((item) => item.platform))).sort((a, b) =>
      a.localeCompare(b, "fr", { sensitivity: "base" })
    );
    return ["Toutes", ...unique];
  }, [mergedItems]);

  const filteredItems = useMemo(() => {
    const needle = search.trim().toLowerCase();

    return mergedItems.filter((item) => {
      const matchesSearch =
        !needle ||
        [item.title, item.platform, item.fileName].some((value) =>
          value.toLowerCase().includes(needle)
        );

      const matchesPlatform = platformFilter === "Toutes" || item.platform === platformFilter;

      return matchesSearch && matchesPlatform;
    });
  }, [mergedItems, platformFilter, search]);

  const loadGames = async () => {
    if (!session) {
      setError("Mode hors ligne : seules les ROMs déjà téléchargées sont affichées.");
      return;
    }

    try {
      setLoading(true);
      setError(null);
      const roms = await getRommGames(session);
      setGames(roms);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Chargement RomM impossible.");
    } finally {
      setLoading(false);
    }
  };

  return (
    <CollapsiblePanel
      eyebrow="Bibliothèque"
      title="Jeux"
      actions={
        <button
          className="primary-button compact-button"
          onClick={() => void loadGames()}
          disabled={loading || !session}
        >
          {loading ? "Chargement..." : session ? "Charger RomM" : "Hors ligne"}
        </button>
      }
    >
      <div className="library-toolbar library-toolbar-grid">
        <input
          value={search}
          onChange={(event) => setSearch(event.target.value)}
          placeholder="Rechercher un jeu..."
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

      {notice && (
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
      )}

      {error && <p className="form-message error-message">{error}</p>}

      <div className="library-list">
        {filteredItems.map((item) => {
          const isDownloading = item.remoteGame && downloadingGameId === String(item.remoteGame.id);

          return (
            <div key={item.id} className="library-item">
              <div>
                <strong>{item.title}</strong>
                <p>
                  {item.platform}
                  {item.downloaded ? " • téléchargé" : ""}
                </p>
                <small>{item.fileName}</small>
              </div>

              <div className="library-actions">
                {item.downloaded && item.localPath && (
                  <button
                    className="primary-button compact-button"
                    onClick={() => void onLaunchLocalRom(item.localPath)}
                  >
                    Lancer
                  </button>
                )}

                {!item.downloaded && item.remoteGame && (
                  <button
                    className={`download-button ${isDownloading ? "download-button-loading" : ""}`}
                    disabled={isDownloading}
                    onClick={() => void onDownloadGame(item.remoteGame)}
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
                        ? `Téléchargement... ${Math.round(downloadPercent)}%`
                        : "Télécharger"}
                    </span>
                  </button>
                )}
              </div>
            </div>
          );
        })}

        {!loading && !filteredItems.length && (
          <div className="empty-state">
            <p>Aucun jeu affiché</p>
            <small>Les ROMs locales restent visibles même hors ligne.</small>
          </div>
        )}
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