import { useEffect, useMemo, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

export default function WindowTitlebar() {
  const appWindow = useMemo(() => getCurrentWindow(), []);
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    void appWindow.isMaximized().then(setIsMaximized).catch(() => setIsMaximized(false));
  }, [appWindow]);

  const minimizeWindow = () => {
    void appWindow.minimize();
  };

  const toggleMaximizeWindow = async () => {
    await appWindow.toggleMaximize();
    setIsMaximized(await appWindow.isMaximized());
  };

  const closeWindow = () => {
    void appWindow.close();
  };

  return (
    <header className="window-titlebar">
      <div className="window-titlebar-drag-zone" data-tauri-drag-region />

      <div className="window-titlebar-controls">
        <button
          className="window-control-button"
          type="button"
          aria-label="Minimize window"
          title="Minimize"
          onClick={minimizeWindow}
        >
          <span className="window-control-icon window-control-icon-minimize" aria-hidden="true" />
        </button>
        <button
          className="window-control-button"
          type="button"
          aria-label={isMaximized ? "Restore window" : "Maximize window"}
          title={isMaximized ? "Restore" : "Maximize"}
          onClick={() => void toggleMaximizeWindow()}
        >
          <span
            className={`window-control-icon ${
              isMaximized ? "window-control-icon-restore" : "window-control-icon-maximize"
            }`}
            aria-hidden="true"
          />
        </button>
        <button
          className="window-control-button window-control-button-close"
          type="button"
          aria-label="Close window"
          title="Close"
          onClick={closeWindow}
        >
          <span className="window-control-icon window-control-icon-close" aria-hidden="true" />
        </button>
      </div>
    </header>
  );
}
