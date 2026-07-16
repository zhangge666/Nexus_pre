/**
 * 本文件渲染 Orbit 本地 Memory Protocol 服务的低干扰诊断状态。
 * 状态仅用于说明桌面端是否已连通本地服务，并提供就地重试入口。
 */
import type React from "react";
import { useCallback, useEffect, useState } from "react";
import { RefreshCw } from "lucide-react";
import { getServiceStatus, isTauriRuntime } from "../core";
import type { ServiceStatus as ServiceStatusData } from "../core";

/** 渲染可访问的服务状态，并在桌面端请求真实本地诊断数据。 */
export function ServiceStatus(): React.JSX.Element | null {
  const [status, setStatus] = useState<ServiceStatusData | null>(null);
  const [checking, setChecking] = useState(false);

  /** 请求最新诊断数据；失败也保留可重试的明确提示。 */
  const refresh = useCallback(async (): Promise<void> => {
    setChecking(true);
    try {
      setStatus(await getServiceStatus());
    } catch (error) {
      setStatus({
        role: "client",
        endpoint: "本地服务",
        available: false,
        message: `无法读取服务状态：${String(error)}`,
      });
    } finally {
      setChecking(false);
    }
  }, []);

  useEffect(() => {
    if (isTauriRuntime()) void refresh();
  }, [refresh]);

  if (!isTauriRuntime() || !status) return null;

  const label = status.available ? "本地服务已就绪" : "本地服务不可用";
  return (
    <div className={`service-status${status.available ? "" : " is-error"}`} title={status.message ?? status.endpoint}>
      <span className="service-status-dot" aria-hidden="true" />
      <span aria-live="polite">{label}</span>
      <button
        className="icon-button service-status-retry"
        onClick={() => void refresh()}
        disabled={checking}
        aria-label="重新检查本地服务"
        title="重新检查本地服务"
      >
        <RefreshCw size={13} className={checking ? "is-spinning" : ""} />
      </button>
    </div>
  );
}
