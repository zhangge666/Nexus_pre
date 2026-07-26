/** 本文件封装 Muse 快捷工具窗的隐藏、失焦与键盘生命周期。 */

import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauriRuntime } from "../api";

interface ToolWindowLifecycleOptions {
  hideOnBlur?: boolean;
  onFocus?: () => void;
}

/** 隐藏当前快捷工具窗，确保后续系统快捷键可以再次唤起同一窗口。 */
export async function hideToolWindow(): Promise<void> {
  if (!isTauriRuntime()) return;
  await getCurrentWindow().hide();
}

/** 注册 Esc 隐藏、可选失焦隐藏与重新聚焦后的输入恢复。 */
export function useToolWindowLifecycle({
  hideOnBlur = false,
  onFocus,
}: ToolWindowLifecycleOptions = {}): void {
  useEffect(() => {
    let disposed = false;
    let removeFocusListener: (() => void) | undefined;

    /** 处理工具窗的快速退出键。 */
    function handleKeyDown(event: KeyboardEvent): void {
      if (event.key !== "Escape") return;
      event.preventDefault();
      void hideToolWindow();
    }

    window.addEventListener("keydown", handleKeyDown);

    if (isTauriRuntime()) {
      void getCurrentWindow()
        .onFocusChanged(({ payload: focused }) => {
          if (focused) {
            onFocus?.();
          } else if (hideOnBlur) {
            void hideToolWindow();
          }
        })
        .then((unlisten) => {
          if (disposed) {
            unlisten();
            return;
          }
          removeFocusListener = unlisten;
        });
    }

    return () => {
      disposed = true;
      removeFocusListener?.();
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [hideOnBlur, onFocus]);
}
