import { FormEvent, useState } from "react";
import { createRommSession, type RommSession } from "../lib/romm";

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
      const session = await createRommSession({
        baseUrl,
        username,
        password
      });

      await onConnected(session, username);
      setPassword("");
      setSuccess("Connexion RomM réussie.");
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Connexion RomM impossible.");
    } finally {
      setLoading(false);
    }
  };

  return (
    <section className="panel">
      <p className="eyebrow">RomM</p>
      <h2>Connexion au serveur</h2>
      <form className="form-grid" onSubmit={handleSubmit}>
        <label className="field">
          <span>URL du serveur</span>
          <input
            value={baseUrl}
            onChange={(event) => setBaseUrl(event.target.value)}
            placeholder="https://romm.example.com"
            required
          />
        </label>

        <label className="field">
          <span>Utilisateur</span>
          <input
            value={username}
            onChange={(event) => setUsername(event.target.value)}
            placeholder="tilio"
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
    </section>
  );
}