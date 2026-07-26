/**
 * 本文件封装 Orbit 对 Tauri 记忆变更事件的订阅。
 * 浏览器预览环境不会加载 Tauri 事件模块，避免开发预览依赖桌面运行时。
 */
import { useEffect, useRef } from "react";
import { isTauriRuntime } from "./index";
import type { MemoryChangedEvent } from "./types";
import { isAndroidPlatform } from "../platform";

type Unlisten = () => void;

/**
 * 在桌面环境订阅 core 提交后的记忆变更；返回清理函数以释放原生事件监听。
 */
export async function subscribeToMemoryChanges(
  onChanged: (event: MemoryChangedEvent) => void,
  onError?: (error: unknown) => void,
): Promise<Unlisten> {
  if (!isTauriRuntime()) return () => undefined;

  try {
    const { listen } = await import("@tauri-apps/api/event");
    return await listen<MemoryChangedEvent>("memory-changed", (event) => onChanged(event.payload));
  } catch (error) {
    onError?.(error);
    return () => undefined;
  }
}

/**
 * 将最新回调保存在 ref 中，使页面刷新函数变化时无需重复注册原生监听器。
 */
export function useMemoryChanges(
  onChanged: (event: MemoryChangedEvent) => void,
  onError?: (error: unknown) => void,
): void {
  const changedRef = useRef(onChanged);
  const errorRef = useRef(onError);
  changedRef.current = onChanged;
  errorRef.current = onError;

  useEffect(() => {
    let disposed = false;
    let unlisten: Unlisten | undefined;
    let refreshTimer: number | undefined;

    /** Android 回到前台、恢复网络或达到刷新周期时重新拉取远程数据。 */
    const refreshRemote = (): void => {
      if (document.visibilityState === "visible") {
        changedRef.current({ type: "memory_updated", id: "remote-refresh", source: "orbit" });
      }
    };

    void subscribeToMemoryChanges(
      (event) => changedRef.current(event),
      (error) => errorRef.current?.(error),
    ).then((cleanup) => {
      // 异步加载完成前组件可能已卸载，必须立即释放刚建立的监听器。
      if (disposed) cleanup();
      else unlisten = cleanup;
    });

    if (isAndroidPlatform()) {
      window.addEventListener("online", refreshRemote);
      document.addEventListener("visibilitychange", refreshRemote);
      refreshTimer = window.setInterval(refreshRemote, 60_000);
    }

    return () => {
      disposed = true;
      unlisten?.();
      window.removeEventListener("online", refreshRemote);
      document.removeEventListener("visibilitychange", refreshRemote);
      if (refreshTimer !== undefined) window.clearInterval(refreshTimer);
    };
  }, []);
}
