export interface RommConnectionInput {
  baseUrl: string;
  username: string;
  password: string;
}

export interface RommSession {
  baseUrl: string;
  token: string;
}

export interface RommPlatform {
  id: number | string;
  name: string;
  slug?: string;
}

export interface RommGame {
  id: number | string;
  name: string;
  platform_name?: string;
  file_name?: string;
  download_url?: string;
  url?: string;
}

export class RommError extends Error {
  constructor(message: string, public status?: number) {
    super(message);
    this.name = "RommError";
  }
}

function normalizeBaseUrl(value: string): string {
  return value.trim().replace(/\/+$/, "");
}

async function parseJsonSafe(response: Response): Promise<unknown> {
  const text = await response.text();
  if (!text) {
    return null;
  }

  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

export async function createRommSession(
  input: RommConnectionInput
): Promise<RommSession> {
  const baseUrl = normalizeBaseUrl(input.baseUrl);

  const response = await fetch(`${baseUrl}/api/token`, {
    method: "POST",
    headers: {
      "Content-Type": "application/x-www-form-urlencoded"
    },
    body: new URLSearchParams({
      username: input.username,
      password: input.password
    })
  });

  const payload = (await parseJsonSafe(response)) as
    | { access_token?: string; token?: string; detail?: string }
    | string
    | null;

  if (!response.ok) {
    const message =
      typeof payload === "object" && payload && "detail" in payload
        ? payload.detail || "Échec d'authentification RomM"
        : "Échec d'authentification RomM";

    throw new RommError(message, response.status);
  }

  const token =
    typeof payload === "object" && payload
      ? payload.access_token || payload.token
      : undefined;

  if (!token) {
    throw new RommError("Réponse RomM invalide : token manquant");
  }

  return {
    baseUrl,
    token
  };
}

async function rommFetch<T>(
  session: RommSession,
  path: string,
  init?: RequestInit
): Promise<T> {
  const response = await fetch(`${session.baseUrl}${path}`, {
    ...init,
    headers: {
      Authorization: `Bearer ${session.token}`,
      Accept: "application/json",
      ...(init?.headers || {})
    }
  });

  const payload = (await parseJsonSafe(response)) as T | { detail?: string } | string | null;

  if (!response.ok) {
    const message =
      typeof payload === "object" && payload && "detail" in payload
        ? payload.detail || "Erreur API RomM"
        : "Erreur API RomM";

    throw new RommError(message, response.status);
  }

  return payload as T;
}

export async function getRommPlatforms(
  session: RommSession
): Promise<RommPlatform[]> {
  const payload = await rommFetch<
    RommPlatform[] | { items?: RommPlatform[]; results?: RommPlatform[] }
  >(session, "/api/platforms");

  if (Array.isArray(payload)) {
    return payload;
  }

  return payload.items || payload.results || [];
}

export async function getRommGames(session: RommSession): Promise<RommGame[]> {
  const payload = await rommFetch<
    RommGame[] | { items?: RommGame[]; results?: RommGame[] }
  >(session, "/api/roms");

  if (Array.isArray(payload)) {
    return payload;
  }

  return payload.items || payload.results || [];
}

export function resolveGameDownloadUrl(session: RommSession, game: RommGame): string | null {
  const raw = game.download_url || game.url;
  if (!raw) {
    return null;
  }

  if (/^https?:\/\//i.test(raw)) {
    return raw;
  }

  return `${session.baseUrl}${raw.startsWith("/") ? raw : `/${raw}`}`;
}