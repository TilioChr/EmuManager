export interface RommConnectionInput {
  baseUrl: string;
  username: string;
  password: string;
}

export interface RommSession {
  baseUrl: string;
  token: string;
}

export interface RommLaunchSession {
  baseUrl: string;
  token: string;
  saveConflictResolution?: "useLocal" | "useRomm";
}

export interface RommPlatform {
  id: number | string;
  name: string;
  slug?: string;
}

export interface RommGameFile {
  id?: number | string;
  file_name?: string;
  filename?: string;
  fs_name?: string;
  name?: string;
  [key: string]: unknown;
}

export interface RommGame {
  id: number | string;
  name: string;
  platform_name?: string;
  platform_display_name?: string;
  platform_slug?: string;
  platform_fs_slug?: string;

  file_name?: string;
  filename?: string;
  fs_name?: string;
  fsName?: string;

  fs_size_bytes?: number;

  download_url?: string;
  downloadUrl?: string;
  url?: string;
  file_url?: string;
  fileUrl?: string;

  summary?: string;
  description?: string;
  overview?: string;
  cover_url?: string;
  coverUrl?: string;
  thumbnail_url?: string;
  thumbnailUrl?: string;
  image_url?: string;
  imageUrl?: string;
  screenshots?: unknown;

  files?: RommGameFile[];

  [key: string]: unknown;
}

export interface RommSaveEntry {
  id: number | string;
  file_name?: string;
  updated_at?: string;
  slot?: string;
  emulator?: string;
  [key: string]: unknown;
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

const DEFAULT_SCOPES = [
  "roms.read",
  "platforms.read",
  "assets.read",
  "assets.write",
  "firmware.read",
  "collections.read",
  "me.read"
].join(" ");

export async function createRommSession(
  input: RommConnectionInput
): Promise<RommSession> {
  const baseUrl = normalizeBaseUrl(input.baseUrl);

  const body = new URLSearchParams({
    username: input.username,
    password: input.password,
    grant_type: "password",
    scope: DEFAULT_SCOPES
  });

  const response = await fetch(`${baseUrl}/api/token`, {
    method: "POST",
    headers: {
      "Content-Type": "application/x-www-form-urlencoded"
    },
    body
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
        : `Erreur API RomM (${response.status})`;

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

export async function getRommGameDetails(
  session: RommSession,
  romId: string | number
): Promise<RommGame> {
  const payload = await rommFetch<
    RommGame | { item?: RommGame; result?: RommGame; data?: RommGame }
  >(session, `/api/roms/${encodeURIComponent(String(romId))}`);

  if ("name" in payload && "id" in payload) {
    return payload;
  }

  return payload.item || payload.result || payload.data || (payload as RommGame);
}

export async function getRommGameScreenshots(
  session: RommSession,
  romId: string | number
): Promise<unknown[]> {
  const encodedId = encodeURIComponent(String(romId));
  const paths = [
    `/api/screenshots?rom_id=${encodedId}`,
    `/api/roms/${encodedId}/screenshots`,
    `/api/roms/${encodedId}/assets/screenshots`
  ];

  for (const path of paths) {
    try {
      const payload = await rommFetch<unknown>(session, path);
      const screenshots = unwrapRommList(payload);
      if (screenshots.length > 0) {
        return screenshots;
      }
    } catch (reason) {
      if (
        !(reason instanceof RommError) ||
        ![400, 404, 405, 422].includes(reason.status ?? 0)
      ) {
        throw reason;
      }
    }
  }

  return [];
}

export async function getLatestRommSave(
  session: RommSession,
  romId: string | number,
  emulatorId: string,
  slotName: string
): Promise<RommSaveEntry | null> {
  const payload = await rommFetch<
    RommSaveEntry[] | { items?: RommSaveEntry[]; results?: RommSaveEntry[] }
  >(
    session,
    `/api/saves?rom_id=${encodeURIComponent(String(romId))}&emulator=${encodeURIComponent(
      emulatorId
    )}&slot=${encodeURIComponent(
      slotName
    )}`
  );

  const saves = Array.isArray(payload) ? payload : payload.items || payload.results || [];

  return saves
    .filter((entry) => typeof entry.file_name === "string" && entry.file_name.endsWith(".zip"))
    .sort((left, right) => {
      const leftDate = left.updated_at || "";
      const rightDate = right.updated_at || "";
      return rightDate.localeCompare(leftDate);
    })[0] ?? null;
}

function absolutize(baseUrl: string, raw: string): string {
  if (/^https?:\/\//i.test(raw)) {
    return raw;
  }

  return `${baseUrl}${raw.startsWith("/") ? raw : `/${raw}`}`;
}

function resolveGameFileName(game: RommGame): string | null {
  const direct = [game.file_name, game.filename, game.fs_name, game.fsName].find(
    (value): value is string => typeof value === "string" && value.length > 0
  );

  if (direct) {
    return direct;
  }

  if (Array.isArray(game.files) && game.files.length > 0) {
    const first = game.files[0];
    const nested = [first.file_name, first.filename, first.fs_name, first.name].find(
      (value): value is string => typeof value === "string" && value.length > 0
    );

    if (nested) {
      return nested;
    }
  }

  return null;
}

export function resolveGameDownloadUrl(session: RommSession, game: RommGame): string | null {
  const directCandidates = [
    game.download_url,
    game.downloadUrl,
    game.url,
    game.file_url,
    game.fileUrl
  ].filter((value): value is string => typeof value === "string" && value.length > 0);

  if (directCandidates.length > 0) {
    return absolutize(session.baseUrl, directCandidates[0]);
  }

  const fileName = resolveGameFileName(game);

  if (game.id !== undefined && game.id !== null && fileName) {
    const id = encodeURIComponent(String(game.id));
    const encodedFileName = encodeURIComponent(fileName);
    return `${session.baseUrl}/api/roms/${id}/content/${encodedFileName}`;
  }

  return null;
}

function unwrapRommList(payload: unknown): unknown[] {
  if (Array.isArray(payload)) {
    return payload;
  }

  if (payload && typeof payload === "object") {
    const record = payload as Record<string, unknown>;
    for (const key of ["items", "results", "data", "screenshots", "assets"]) {
      const value = record[key];
      if (Array.isArray(value)) {
        return value;
      }
    }
  }

  return [];
}

export function resolveGameLocalFileName(game: RommGame): string {
  return resolveGameFileName(game) || `${game.name}.iso`;
}

function sanitizePathSegment(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[<>:"/\\|?*\x00-\x1f]/g, "-")
    .replace(/\s+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "");
}

export function resolveGameRomSubdir(game: RommGame): string {
  const candidate =
    game.platform_fs_slug ||
    game.platform_slug ||
    game.platform_display_name ||
    game.platform_name ||
    "autre";

  const sanitized = sanitizePathSegment(candidate);
  return sanitized || "autre";
}
