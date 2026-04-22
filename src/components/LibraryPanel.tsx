import { useMemo, useState } from "react";
import { getRommGames, type RommGame, type RommSession } from "../lib/romm";

interface LibraryPanelProps {
  session: RommSession | null;
  onDownloadGame: (game: RommGame) => Promise<void>;
  downloadingGameId?: string | null;
}

export default function LibraryPanel({
  session,
  onDownloadGame,
  downloadingGameId = null
}: LibraryPanelProps) {
  const [games, setGames] = useState<RommGame[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");

  const filteredGames = useMemo(() => {
    const needle = search.trim().toLowerCase();
    if (!needle) {
      return games;
    }

    return games.filter((game) => {
      return [game.name, game.platform_name, game.file_name]
        .filter(Boolean)
        .some((value) => String(value).toLowerCase().includes(needle));
    });
  }, [games, search]);

  const loadGames = async () => {
    if (!session) {
      setError("Connecte-toi à RomM pour charger la bibliothèque.");
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
    <section className="panel">
      <div className="panel-header-row">
        <div>
          <p className="eyebrow">Bibliothèque</p>
          <h2>Jeux RomM</h2>
        </div>
        <button className="primary-button compact-button" onClick={() => void loadGames()} disabled={loading}>
          {loading ? "Chargement..." : "Charger la bibliothèque"}
        </button>
      </div>

      <div className="library-toolbar">
        <input
          value={search}
          onChange={(event) => setSearch(event.target.value)}
          placeholder="Rechercher un jeu..."
          disabled={!games.length}
        />
      </div>

      {error && <p className="form-message error-message">{error}</p>}

      <div className="library-list">
        {filteredGames.map((game) => {
          const gameId = String(game.id);
          return (
            <div key={gameId} className="library-item">
              <div>
                <strong>{game.name}</strong>
                <p>{game.platform_name ?? "Plateforme inconnue"}</p>
                <small>{game.file_name ?? "Nom de fichier inconnu"}</small>
              </div>
              <button
                className="primary-button compact-button"
                disabled={downloadingGameId === gameId}
                onClick={() => void onDownloadGame(game)}
              >
                {downloadingGameId === gameId ? "Téléchargement..." : "Télécharger"}
              </button>
            </div>
          );
        })}

        {!loading && !filteredGames.length && (
          <div className="empty-state">
            <p>Aucun jeu affiché</p>
            <small>Charge la bibliothèque puis filtre si besoin.</small>
          </div>
        )}
      </div>
    </section>
  );
}