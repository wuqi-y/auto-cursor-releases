import { useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

export function TitleBar() {
  const appWindow = useMemo(() => getCurrentWindow(), []);
  const [isMaximized, setIsMaximized] = useState(false);
  const isTogglingRef = useRef(false);

  const handleToggleMaximize = async () => {
    if (isTogglingRef.current) return;
    isTogglingRef.current = true;
    try {
      const maximized = await appWindow.isMaximized();
      if (maximized) {
        await appWindow.unmaximize();
      } else {
        await appWindow.maximize();
      }
      setIsMaximized(await appWindow.isMaximized());
    } catch {
      // Ignore transient platform errors to avoid blocking UI interaction.
    } finally {
      isTogglingRef.current = false;
    }
  };

  useEffect(() => {
    const syncMaximizedState = () => {
      void appWindow
        .isMaximized()
        .then((maximized) => setIsMaximized(maximized))
        .catch(() => {
          // Ignore transient platform errors to keep UI responsive.
        });
    };

    syncMaximizedState();
    window.addEventListener("resize", syncMaximizedState);

    return () => {
      window.removeEventListener("resize", syncMaximizedState);
    };
  }, [appWindow]);

  const controls = (
    <div className="flex items-center gap-2">
      <button
        type="button"
        className="titlebar-btn"
        aria-label="最小化"
        onClick={() => appWindow.minimize()}
      >
        <span className="titlebar-glyph">—</span>
      </button>
      <button
        type="button"
        className="titlebar-btn"
        aria-label={isMaximized ? "还原" : "最大化"}
        onClick={handleToggleMaximize}
      >
        <span className="titlebar-glyph">{isMaximized ? "❐" : "□"}</span>
      </button>
      <button
        type="button"
        className="titlebar-btn titlebar-btn-close"
        aria-label="关闭"
        onClick={() => appWindow.close()}
      >
        <span className="titlebar-glyph">×</span>
      </button>
    </div>
  );

  return (
    <div className="titlebar">
      <div className="titlebar-inner" data-tauri-drag-region>
        <div className="titlebar-left">
          <div className="titlebar-appname">
            Auto Cursor
          </div>
        </div>
        <div className="titlebar-center" data-tauri-drag-region />
        <div className="titlebar-right">
          {controls}
        </div>
      </div>
    </div>
  );
}

