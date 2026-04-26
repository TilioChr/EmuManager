export type NotificationKind = "info" | "success" | "warning" | "error";

export interface NotificationEntry {
  id: number;
  kind: NotificationKind;
  message: string;
  createdAt: number;
  exiting?: boolean;
}

interface NotificationOverlayProps {
  notifications: NotificationEntry[];
  history: NotificationEntry[];
  historyOpen: boolean;
  onToggleHistory: () => void;
  onDismiss: (id: number) => void;
  onClearHistory: () => void;
}

const kindLabels: Record<NotificationKind, string> = {
  info: "Info",
  success: "Success",
  warning: "Warning",
  error: "Error"
};

export default function NotificationOverlay({
  notifications,
  history,
  historyOpen,
  onToggleHistory,
  onDismiss,
  onClearHistory
}: NotificationOverlayProps) {
  return (
    <div className="notification-overlay" aria-live="polite" aria-relevant="additions">
      {historyOpen ? (
        <section className="notification-history-panel" aria-label="Notification history">
          <div className="notification-history-header">
            <strong>History</strong>
            <button className="notification-clear-button" type="button" onClick={onClearHistory}>
              Clear
            </button>
          </div>

          <div className="notification-history-list">
            {history.length ? (
              history.slice(0, 40).map((entry) => (
                <div key={`${entry.id}-history`} className={`notification-history-row notification-${entry.kind}`}>
                  <span>{formatNotificationTime(entry.createdAt)}</span>
                  <strong>{kindLabels[entry.kind]}</strong>
                  <p>{entry.message}</p>
                </div>
              ))
            ) : (
              <p className="notification-empty">No notifications yet</p>
            )}
          </div>
        </section>
      ) : null}

      <div className="notification-stack">
        {notifications.map((entry) => (
          <article
            key={entry.id}
            className={`notification-bubble notification-${entry.kind} ${
              entry.exiting ? "notification-exit" : ""
            }`}
          >
            <div>
              <strong>{kindLabels[entry.kind]}</strong>
              <p>{entry.message}</p>
            </div>
            <button
              className="notification-dismiss-button"
              type="button"
              onClick={() => onDismiss(entry.id)}
              aria-label="Dismiss notification"
            >
              x
            </button>
          </article>
        ))}
      </div>

      <button className="notification-history-button" type="button" onClick={onToggleHistory}>
        History
        {history.length ? <span>{history.length}</span> : null}
      </button>
    </div>
  );
}

function formatNotificationTime(value: number): string {
  return new Date(value).toLocaleTimeString("fr-FR", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit"
  });
}
