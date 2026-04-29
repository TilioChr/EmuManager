import { emit } from "@tauri-apps/api/event";

export type DebugLogLevel = "debug" | "info" | "success" | "warning" | "error";
export type DebugLogSource = "frontend" | "backend";

export interface DebugLogEntry {
  id: string;
  timestamp: number;
  level: DebugLogLevel;
  source: DebugLogSource;
  scope: string;
  message: string;
  details?: string;
}

const STORAGE_KEY = "emumanager.debugLogs";
const MAX_LOGS = 700;

export function readDebugLogs(): DebugLogEntry[] {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) {
      return [];
    }

    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter(isDebugLogEntry) : [];
  } catch {
    return [];
  }
}

export async function debugLog(
  level: DebugLogLevel,
  scope: string,
  message: string,
  details?: unknown
): Promise<void> {
  const entry: DebugLogEntry = {
    id: buildLogId("frontend"),
    timestamp: Date.now(),
    level,
    source: "frontend",
    scope,
    message,
    details: serializeDetails(details)
  };

  recordDebugLogEntry(entry);
  await emitDebugLogEntry(entry);
}

export function recordDebugLogEntry(entry: DebugLogEntry): void {
  const logs = readDebugLogs();
  if (logs.some((existing) => existing.id === entry.id)) {
    return;
  }

  const next = [...logs, entry].slice(-MAX_LOGS);
  window.localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  window.dispatchEvent(new CustomEvent<DebugLogEntry>("debug-log-entry-local", { detail: entry }));
}

export async function clearDebugLogs(): Promise<void> {
  window.localStorage.removeItem(STORAGE_KEY);
  window.dispatchEvent(new Event("debug-logs-cleared-local"));

  try {
    await emit("debug-logs-cleared");
  } catch {
    // Debug logging should never break the app.
  }
}

async function emitDebugLogEntry(entry: DebugLogEntry): Promise<void> {
  try {
    await emit("debug-log-entry", entry);
  } catch {
    // Debug logging should never break the app.
  }
}

function buildLogId(source: DebugLogSource): string {
  return `${source}-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

function serializeDetails(details: unknown): string | undefined {
  if (details === undefined || details === null) {
    return undefined;
  }

  if (typeof details === "string") {
    return details;
  }

  try {
    return JSON.stringify(details, null, 2);
  } catch {
    return String(details);
  }
}

function isDebugLogEntry(value: unknown): value is DebugLogEntry {
  if (!value || typeof value !== "object") {
    return false;
  }

  const candidate = value as Partial<DebugLogEntry>;
  return (
    typeof candidate.id === "string" &&
    typeof candidate.timestamp === "number" &&
    typeof candidate.level === "string" &&
    typeof candidate.source === "string" &&
    typeof candidate.scope === "string" &&
    typeof candidate.message === "string"
  );
}
