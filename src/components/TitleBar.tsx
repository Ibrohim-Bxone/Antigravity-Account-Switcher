import { useState, useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { Icon } from "./Icons";
import { t } from "../i18n";

export function TitleBar() {
  const [isMax, setIsMax] = useState(false);
  const appWindow = getCurrentWindow();

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    const checkMax = async () => {
      try {
        const res = await invoke<boolean>("is_window_maximized");
        setIsMax(res);
      } catch {
        try {
          const m = await appWindow.isMaximized();
          setIsMax(m);
        } catch {
          // ignore
        }
      }
    };

    void checkMax();

    appWindow.onResized(() => {
      void checkMax();
    }).then((fn) => {
      unlisten = fn;
    }).catch(() => {});

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const handleMinimize = async () => {
    try {
      await invoke("minimize_window");
    } catch {
      await appWindow.minimize();
    }
  };

  const handleMaximize = async () => {
    try {
      const next = await invoke<boolean>("toggle_maximize");
      setIsMax(next);
    } catch {
      try {
        await appWindow.toggleMaximize();
        const m = await appWindow.isMaximized();
        setIsMax(m);
      } catch (err) {
        console.error("Maximize error:", err);
      }
    }
  };

  const handleClose = async () => {
    try {
      await invoke("close_window");
    } catch {
      await appWindow.close();
    }
  };

  return (
    <div
      className="titlebar"
      data-tauri-drag-region
      onDoubleClick={handleMaximize}
      style={{ userSelect: "none" }}
    >
      <div className="titlebar__left" data-tauri-drag-region>
        <span className="titlebar__title" data-tauri-drag-region>
          {t("titlebar_title")}
        </span>
      </div>
      <div className="titlebar__right">
        <button
          className="titlebar__button titlebar__button--minimize"
          onClick={handleMinimize}
          title={t("minimize")}
          aria-label={t("minimize")}
        >
          <Icon name="minus" size={13} />
        </button>
        <button
          className="titlebar__button titlebar__button--maximize"
          onClick={handleMaximize}
          title={isMax ? "Qaytarish (Restore)" : t("maximize")}
          aria-label={isMax ? "Restore" : t("maximize")}
        >
          <Icon name={isMax ? "copy" : "square"} size={11} />
        </button>
        <button
          className="titlebar__button titlebar__button--close"
          onClick={handleClose}
          title={t("close")}
          aria-label={t("close")}
        >
          <Icon name="close" size={13} />
        </button>
      </div>
    </div>
  );
}
