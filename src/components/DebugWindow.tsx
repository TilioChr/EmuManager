import { useEffect, useMemo, useRef, useState, type Dispatch, type SetStateAction } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  clearDebugLogs,
  readDebugLogs,
  type DebugLogEntry,
  type DebugLogLevel
} from "../lib/debugLog";

const levelOptions: Array<DebugLogLevel | "all"> = [
  "all",
  "debug",
  "info",
  "success",
  "warning",
  "error"
];

export default function DebugWindow() {
  const [logs, setLogs] = useState<DebugLogEntry[]>(() => readDebugLogs());
  const [levelFilter, setLevelFilter] = useState<DebugLogLevel | "all">("all");
  const [search, setSearch] = useState("");
  const [autoScroll, setAutoScroll] = useState(true);
  const endRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    document.title = "EmuManager Debug Logs";
  }, []);

  useEffect(() => {
    let unlistenLog: UnlistenFn | null = null;
    let unlistenClear: UnlistenFn | null = null;

    const handleLocalLog = (event: Event) => {
      const entry = (event as CustomEvent<DebugLogEntry>).detail;
      appendLog(setLogs, entry);
    };

    const handleLocalClear = () => setLogs([]);

    const setupListeners = async () => {
      unlistenLog = await listen<DebugLogEntry>("debug-log-entry", (event) => {
        appendLog(setLogs, event.payload);
      });
      unlistenClear = await listen("debug-logs-cleared", () => setLogs([]));
    };

    window.addEventListener("debug-log-entry-local", handleLocalLog);
    window.addEventListener("debug-logs-cleared-local", handleLocalClear);
    void setupListeners();

    return () => {
      window.removeEventListener("debug-log-entry-local", handleLocalLog);
      window.removeEventListener("debug-logs-cleared-local", handleLocalClear);
      if (unlistenLog) {
        unlistenLog();
      }
      if (unlistenClear) {
        unlistenClear();
      }
    };
  }, []);

  useEffect(() => {
    if (autoScroll) {
      endRef.current?.scrollIntoView({ block: "end" });
    }
  }, [autoScroll, logs]);

  const filteredLogs = useMemo(() => {
    const needle = search.trim().toLowerCase();

    return logs.filter((entry) => {
      const matchesLevel = levelFilter === "all" || entry.level === levelFilter;
      const matchesSearch =
        !needle ||
        [entry.scope, entry.message, entry.details ?? "", entry.source]
          .join(" ")
          .toLowerCase()
          .includes(needle);

      return matchesLevel && matchesSearch;
    });
  }, [levelFilter, logs, search]);

  return (
    <main className="debug-shell">
      <header className="debug-header">
        <div>
          <h1>Debug Logs</h1>
          <p>
            {filteredLogs.length} / {logs.length} entries
          </p>
        </div>

        <div className="debug-toolbar">
          <select
            value={levelFilter}
            onChange={(event) => setLevelFilter(event.target.value as DebugLogLevel | "all")}
            aria-label="Log level"
          >
            {levelOptions.map((level) => (
              <option key={level} value={level}>
                {level === "all" ? "All levels" : level}
              </option>
            ))}
          </select>

          <input
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            placeholder="Search logs"
            aria-label="Search logs"
          />

          <label className="debug-checkbox">
            <input
              type="checkbox"
              checked={autoScroll}
              onChange={(event) => setAutoScroll(event.target.checked)}
            />
            Auto-scroll
          </label>

          <button className="ghost-button compact-button" onClick={() => void clearDebugLogs()}>
            Clear
          </button>
        </div>
      </header>

      <section className="debug-log-list" aria-live="polite">
        {filteredLogs.map((entry) => (
          <article key={entry.id} className={`debug-log-row debug-log-${entry.level}`}>
            <div className="debug-log-meta">
              <time>{formatTimestamp(entry.timestamp)}</time>
              <span>{entry.level}</span>
              <span>{entry.source}</span>
              <strong>{entry.scope}</strong>
            </div>
            <p>{entry.message}</p>
            {entry.details ? (
              <details>
                <summary>Details</summary>
                <pre>{entry.details}</pre>
              </details>
            ) : null}
          </article>
        ))}

        {!filteredLogs.length ? (
          <div className="debug-empty">
            <p>No logs yet.</p>
          </div>
        ) : null}
        <div ref={endRef} />
      </section>
    </main>
  );
}

function appendLog(
  setLogs: Dispatch<SetStateAction<DebugLogEntry[]>>,
  entry: DebugLogEntry
) {
  setLogs((previous) => {
    if (previous.some((existing) => existing.id === entry.id)) {
      return previous;
    }

    return [...previous, entry].slice(-700);
  });
}

function formatTimestamp(timestamp: number): string {
  const date = new Date(timestamp);
  const time = date.toLocaleTimeString("fr-FR", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit"
  });
  return `${time}.${date.getMilliseconds().toString().padStart(3, "0")}`;
}
