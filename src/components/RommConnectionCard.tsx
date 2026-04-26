import { FormEvent, useState } from "react";
import { createRommSession, type RommSession } from "../lib/romm";
import { debugLog } from "../lib/debugLog";
import CollapsiblePanel from "./CollapsiblePanel";

interface RommConnectionCardProps {
  defaultBaseUrl?: string;
  defaultUsername?: string;
  onConnected: (session: RommSession, username: string) => Promise<void> | void;
}

export default function RommConnectionCard({
  defaultBaseUrl = "",
  defaultUsername = "",
  onConnected
}: RommConnectionCardProps) {
  const [baseUrl, setBaseUrl] = useState(defaultBaseUrl);
  const [username, setUsername] = useState(defaultUsername);
  const [password, setPassword] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setLoading(true);
    setError(null);
    setSuccess(null);

    try {
      void debugLog("info", "romm", "RomM connection attempt", {
        baseUrl,
        username
      });
      const session = await createRommSession({
        baseUrl,
        username,
        password
      });

      await onConnected(session, username);
      setPassword("");
      setSuccess("Connexion RomM réussie.");
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : "Connexion RomM impossible.";
      setError(message);
      void debugLog("error", "romm", "RomM connection failed", {
        baseUrl,
        username,
        message
      });
    } finally {
      setLoading(false);
    }
  };

  return (
    <CollapsiblePanel eyebrow="RomM" title="Connexion au serveur">
      <form className="form-grid" onSubmit={handleSubmit}>
        <label className="field">
          <span>URL du serveur</span>
          <input
            value={baseUrl}
            onChange={(event) => setBaseUrl(event.target.value)}
            placeholder="192.168.1.47:8085"
            required
          />
        </label>

        <label className="field">
          <span>Utilisateur</span>
          <input
            value={username}
            onChange={(event) => setUsername(event.target.value)}
            placeholder="John Doe"
            required
          />
        </label>

        <label className="field field-full">
          <span>Mot de passe</span>
          <input
            type="password"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
            placeholder="••••••••"
            required
          />
        </label>

        <div className="field-full form-actions">
          <button className="primary-button" type="submit" disabled={loading}>
            {loading ? "Connexion..." : "Se connecter à RomM"}
          </button>
        </div>

        {error && <p className="form-message error-message">{error}</p>}
        {success && <p className="form-message success-message">{success}</p>}
      </form>
    </CollapsiblePanel>
  );
}
