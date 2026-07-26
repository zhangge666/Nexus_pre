/** 本文件实现 Android E2E 工作区初始化、恢复短语、二维码配对和设备撤销设置。 */

import type React from "react";
import { useEffect, useState } from "react";
import { Check, Copy, KeyRound, RefreshCw, ShieldCheck, Smartphone, Trash2 } from "lucide-react";
import {
  approveE2ePairing,
  completeE2ePairing,
  createE2ePairingOffer,
  getE2ePairingStatus,
  getE2eContentStatus,
  getE2eStatus,
  getRecoveryPhrase,
  initializeE2e,
  listE2eDevices,
  requestE2ePairing,
  restoreE2e,
  revokeE2eDevice,
  syncE2eContent,
} from "../core";
import type {
  E2eContentStatus,
  E2eDevice,
  E2ePairingJoin,
  E2ePairingOffer,
  E2ePairingStatus,
  E2eStatus,
} from "../core";

/** 渲染 E2E 设置中的统一标签、说明和操作区域。 */
function SecurityRow({
  label,
  description,
  children,
}: {
  label: string;
  description?: string;
  children: React.ReactNode;
}): React.JSX.Element {
  return (
    <div className="setting-row e2e-setting-row">
      <div className="setting-label-group">
        <span className="setting-label">{label}</span>
        {description && <p className="setting-desc">{description}</p>}
      </div>
      <div className="setting-control e2e-setting-control">{children}</div>
    </div>
  );
}

/** 格式化中继 Unix 毫秒时间。 */
function formatTime(value: number): string {
  return new Date(value).toLocaleString();
}

/** 渲染 Android E2E 安全设置并协调全部异步密钥流程。 */
export function E2eSecuritySection(): React.JSX.Element {
  const [status, setStatus] = useState<E2eStatus | null>(null);
  const [devices, setDevices] = useState<E2eDevice[]>([]);
  const [contentStatus, setContentStatus] = useState<E2eContentStatus | null>(null);
  const [offer, setOffer] = useState<E2ePairingOffer | null>(null);
  const [pairingStatus, setPairingStatus] = useState<E2ePairingStatus | null>(null);
  const [joinStatus, setJoinStatus] = useState<E2ePairingJoin | null>(null);
  const [deviceName, setDeviceName] = useState("我的 Android 设备");
  const [recoveryInput, setRecoveryInput] = useState("");
  const [pairingUri, setPairingUri] = useState("");
  const [recoveryPhrase, setRecoveryPhrase] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  /** 刷新本机 E2E 状态，并在已配置时读取中继设备清单。 */
  async function refresh(): Promise<void> {
    const nextStatus = await getE2eStatus();
    setStatus(nextStatus);
    if (nextStatus.configured) {
      const [nextDevices, nextContentStatus] = await Promise.all([
        listE2eDevices(),
        getE2eContentStatus(),
      ]);
      setDevices(nextDevices);
      setContentStatus(nextContentStatus);
    } else {
      setDevices([]);
      setContentStatus(null);
    }
  }

  /** 初次进入安全设置时读取 Keystore 状态。 */
  useEffect(() => {
    void refresh().catch((reason) => setError(`E2E 状态加载失败：${String(reason)}`));
  }, []);

  /** 统一执行安全操作，避免重复提交并保持结果反馈。 */
  async function run(action: () => Promise<void>, success: string): Promise<void> {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      await action();
      setMessage(success);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  /** 创建首个同步根密钥和设备身份。 */
  function handleInitialize(): void {
    void run(async () => {
      setStatus(await initializeE2e(deviceName));
      await refresh();
    }, "端到端同步工作区已创建，请立即备份恢复短语。");
  }

  /** 使用用户输入的 BIP39 恢复短语登记当前设备。 */
  function handleRestore(): void {
    void run(async () => {
      setStatus(await restoreE2e(recoveryInput, deviceName));
      setRecoveryInput("");
      await refresh();
    }, "工作区已恢复，当前设备已经登记到中继。");
  }

  /** 读取并临时展示恢复短语。 */
  function handleRevealPhrase(): void {
    if (!window.confirm("恢复短语可以解密完整同步数据。确认仅在私密环境中显示？")) return;
    void run(async () => setRecoveryPhrase(await getRecoveryPhrase()), "恢复短语仅在当前页面临时显示。");
  }

  /** 创建十分钟有效的二维码邀请。 */
  function handleCreateOffer(): void {
    void run(async () => {
      const nextOffer = await createE2ePairingOffer();
      setOffer(nextOffer);
      setPairingStatus(null);
      setStatus(await getE2eStatus());
    }, "配对二维码已创建，请在十分钟内由新设备扫描。");
  }

  /** 查询另一台设备是否已经提交待批准申请。 */
  function handleRefreshPairing(): void {
    void run(async () => setPairingStatus(await getE2ePairingStatus()), "配对状态已刷新。");
  }

  /** 核对确认码后批准新设备。 */
  function handleApprovePairing(): void {
    if (!pairingStatus?.pendingDevice) return;
    if (!window.confirm(`确认批准“${pairingStatus.pendingDevice.name}”加入当前加密工作区？`)) return;
    void run(async () => {
      await approveE2ePairing();
      setOffer(null);
      setPairingStatus(null);
      await refresh();
    }, "新设备已批准，正在等待它领取加密密钥包。");
  }

  /** 使用扫描或粘贴的二维码 URI 申请加入既有工作区。 */
  function handleRequestPairing(): void {
    void run(async () => {
      const result = await requestE2ePairing(pairingUri, deviceName);
      setJoinStatus(result);
      setStatus(await getE2eStatus());
    }, "加入申请已提交，请在已有设备核对确认码并批准。");
  }

  /** 领取已批准的配对包并将根密钥封存进 Keystore。 */
  function handleCompletePairing(): void {
    void run(async () => {
      setStatus(await completeE2ePairing());
      setJoinStatus(null);
      setPairingUri("");
      await refresh();
    }, "配对完成，根密钥与设备私钥已写入 Android Keystore。");
  }

  /** 撤销指定设备并刷新工作区设备列表。 */
  function handleRevokeDevice(device: E2eDevice): void {
    if (!window.confirm(`撤销“${device.name}”后，该设备将不能继续同步。是否继续？`)) return;
    void run(async () => {
      await revokeE2eDevice(device.deviceId);
      await refresh();
    }, "设备已撤销。");
  }

  /** 手动执行一次密文增量上传、拉取、合并和游标确认。 */
  function handleSyncContent(): void {
    void run(async () => {
      setContentStatus(await syncE2eContent());
      await refresh();
    }, "端到端内容同步已完成。");
  }

  /** 将二维码 URI 或恢复短语复制到系统剪贴板。 */
  function copyText(value: string, success: string): void {
    void navigator.clipboard.writeText(value).then(() => setMessage(success)).catch((reason) => setError(String(reason)));
  }

  return (
    <section className="settings-content-inner" aria-labelledby="settings-security-title">
      <div className="settings-section-heading">
        <span className="settings-section-icon" aria-hidden="true"><ShieldCheck size={15} /></span>
        <div>
          <h2 id="settings-security-title" className="setting-group-title">端到端加密</h2>
          <p>根密钥和设备私钥由 Android Keystore 保护，中继只保存签名密文与设备公钥。</p>
        </div>
      </div>

      <div className="settings-group">
        <SecurityRow
          label="加密状态"
          description={status?.configured ? `工作区 ${status.workspaceId?.slice(0, 12)}…` : "当前设备尚未持有同步根密钥。"}
        >
          <span className={`e2e-status${status?.configured ? " active" : ""}`}>
            {status?.configured ? <><Check size={13} />已配置</> : "未配置"}
          </span>
        </SecurityRow>

        <SecurityRow label="设备名称" description="只作为设备管理中的可识别名称，不包含账号信息。">
          <input className="settings-input" value={deviceName} maxLength={80} onChange={(event) => setDeviceName(event.target.value)} disabled={busy || status?.configured} />
        </SecurityRow>

        {!status?.configured && <>
          <SecurityRow label="创建新工作区" description="生成新的 256 位根密钥、恢复短语和 Ed25519 设备身份。">
            <button type="button" className="primary-small" onClick={handleInitialize} disabled={busy}>创建加密工作区</button>
          </SecurityRow>
          <SecurityRow label="恢复短语" description="输入已有工作区的 24 词 BIP39 英文恢复短语。">
            <div className="e2e-stack">
              <textarea className="settings-textarea e2e-secret-input" value={recoveryInput} onChange={(event) => setRecoveryInput(event.target.value)} placeholder="word1 word2 … word24" rows={3} disabled={busy} />
              <button type="button" className="secondary-button" onClick={handleRestore} disabled={busy || recoveryInput.trim().split(/\s+/).length !== 24}>使用短语恢复</button>
            </div>
          </SecurityRow>
          <SecurityRow label="扫码加入" description="扫描二维码后会得到 nexus://pair URI；也可从另一台设备复制粘贴。">
            <div className="e2e-stack">
              <textarea className="settings-textarea e2e-secret-input" value={pairingUri} onChange={(event) => setPairingUri(event.target.value)} placeholder="nexus://pair?…" rows={3} disabled={busy} />
              <button type="button" className="secondary-button" onClick={handleRequestPairing} disabled={busy || !pairingUri.trim()}>提交加入申请</button>
              {(joinStatus || status?.pendingJoin) && <button type="button" className="primary-small" onClick={handleCompletePairing} disabled={busy}>领取已批准密钥包</button>}
              {joinStatus && <span className="e2e-verification-code">确认码 {joinStatus.verificationCode}</span>}
            </div>
          </SecurityRow>
        </>}

        {status?.configured && <>
          <SecurityRow
            label="内容同步"
            description={contentStatus
              ? `游标 ${contentStatus.cursor} · 待上传 ${contentStatus.pendingChanges} · 冲突留痕 ${contentStatus.conflictCount}${contentStatus.lastSyncAt ? ` · ${formatTime(contentStatus.lastSyncAt)}` : ""}`
              : "本机副本尚未完成第一次密文增量同步。"}
          >
            <button type="button" className="secondary-button" onClick={handleSyncContent} disabled={busy}>
              <RefreshCw size={13} />立即同步
            </button>
          </SecurityRow>

          <SecurityRow label="恢复短语" description="丢失全部设备时只能使用该短语恢复；中继无法找回。">
            <div className="e2e-stack">
              {!recoveryPhrase && <button type="button" className="secondary-button" onClick={handleRevealPhrase} disabled={busy}><KeyRound size={13} />显示恢复短语</button>}
              {recoveryPhrase && <>
                <textarea className="settings-textarea e2e-secret-input" value={recoveryPhrase} readOnly rows={4} />
                <div className="e2e-inline-actions">
                  <button type="button" className="secondary-button" onClick={() => copyText(recoveryPhrase, "恢复短语已复制。")}><Copy size={13} />复制</button>
                  <button type="button" className="secondary-button" onClick={() => setRecoveryPhrase(null)}>隐藏</button>
                </div>
              </>}
            </div>
          </SecurityRow>

          <SecurityRow label="新设备配对" description="二维码秘密不会上传中继；六位码只用于两端人工核对。">
            <div className="e2e-stack">
              {!offer && <button type="button" className="secondary-button" onClick={handleCreateOffer} disabled={busy}>创建配对二维码</button>}
              {offer && <>
                {offer.qrDataUrl && <img className="e2e-qr" src={offer.qrDataUrl} alt="Orbit E2E 新设备配对二维码" />}
                <span className="e2e-verification-code">确认码 {offer.verificationCode}</span>
                <span className="e2e-expiry">有效至 {formatTime(offer.expiresAt)}</span>
                <button type="button" className="secondary-button" onClick={() => copyText(offer.pairingUri, "配对 URI 已复制。")}><Copy size={13} />复制配对 URI</button>
                <button type="button" className="secondary-button" onClick={handleRefreshPairing} disabled={busy}><RefreshCw size={13} />检查申请</button>
              </>}
              {pairingStatus?.pendingDevice && <div className="e2e-pending-device">
                <Smartphone size={16} />
                <div><strong>{pairingStatus.pendingDevice.name}</strong><span>{pairingStatus.pendingDevice.deviceId}</span></div>
                <button type="button" className="primary-small" onClick={handleApprovePairing} disabled={busy}>批准</button>
              </div>}
            </div>
          </SecurityRow>

          <SecurityRow label="已登记设备" description="撤销后中继会拒绝该设备的后续签名信封。">
            <div className="e2e-device-list">
              {devices.map((device) => (
                <div className={`e2e-device${device.revokedAt ? " revoked" : ""}`} key={device.deviceId}>
                  <Smartphone size={15} />
                  <div>
                    <strong>{device.name}{device.deviceId === status.deviceId ? "（当前设备）" : ""}</strong>
                    <span>{device.revokedAt ? `已撤销 · ${formatTime(device.revokedAt)}` : `最近活动 · ${formatTime(device.lastSeenAt)}`}</span>
                  </div>
                  {!device.revokedAt && device.deviceId !== status.deviceId && <button type="button" className="icon-button danger-text" aria-label={`撤销 ${device.name}`} onClick={() => handleRevokeDevice(device)} disabled={busy}><Trash2 size={14} /></button>}
                </div>
              ))}
            </div>
          </SecurityRow>
        </>}
      </div>

      {(message || error) && <p className={`inline-notice e2e-notice${error ? " danger-text" : ""}`} role={error ? "alert" : "status"}>{error ?? message}</p>}
    </section>
  );
}
