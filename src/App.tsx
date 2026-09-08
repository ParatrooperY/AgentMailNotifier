import { useEffect, useRef, useState } from "react";
import { Check, ChevronDown, CircleAlert, Clock3, Mail, Pencil, Power, ShieldCheck, Trash2 } from "lucide-react";
import type { DashboardState, IntegrationKind, IntegrationStatus, NotifierBridge, SmtpDraft } from "./domain";
import { smtpProviderFor } from "./smtpProvider";

export interface AppProps { bridge: NotifierBridge; }

function messageFromError(cause: unknown, fallback = "操作失败"): string {
  if (cause instanceof Error) return cause.message;
  if (typeof cause === "string" && cause.trim()) return cause;
  if (typeof cause === "object" && cause !== null && "message" in cause && typeof cause.message === "string") return cause.message;
  return fallback;
}

function EmptyHistory() {
  return <div className="empty-history"><Clock3 size={18} /><span>还没有通知记录</span></div>;
}

function draftFromStatus(status: IntegrationStatus): SmtpDraft {
  return {
    email: status.smtp.email,
    authorizationCode: "",
    customSmtp: status.smtp.provider === "custom",
    customHost: status.smtp.host ?? undefined,
    customPort: status.smtp.port ?? undefined,
    customEncryption: status.smtp.encryption ?? "ssl",
  };
}

function EncryptionSelect({ value, onChange }: { value: "ssl" | "starttls"; onChange: (value: "ssl" | "starttls") => void }) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const label = value === "starttls" ? "STARTTLS" : "SSL/TLS";

  useEffect(() => {
    const close = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", close);
    return () => document.removeEventListener("pointerdown", close);
  }, []);

  return <div className="encryption-select" ref={rootRef}>
    <button className="encryption-select-trigger" type="button" aria-label="加密方式" aria-haspopup="listbox" aria-expanded={open} onClick={() => setOpen((current) => !current)}>
      <span>{label}</span><ChevronDown size={16} />
    </button>
    {open && <div className="encryption-select-menu" role="listbox" aria-label="加密方式">
      {(["ssl", "starttls"] as const).map((option) => <button key={option} className={`encryption-option${value === option ? " is-selected" : ""}`} type="button" role="option" aria-selected={value === option} onClick={() => { onChange(option); setOpen(false); }}>{option === "ssl" ? "SSL/TLS" : "STARTTLS"}</button>)}
    </div>}
  </div>;
}

function IntegrationCard({
  status,
  draft,
  onDraftChange,
  busy,
  onSmtpTest,
  onToggle,
  onClearHistory,
}: {
  status: IntegrationStatus;
  draft: SmtpDraft;
  onDraftChange: (draft: SmtpDraft) => void;
  busy: boolean;
  onSmtpTest: (kind: IntegrationKind, draft: SmtpDraft) => Promise<boolean>;
  onToggle: (kind: IntegrationKind, enabled: boolean) => Promise<void>;
  onClearHistory: (kind: IntegrationKind) => Promise<void>;
}) {
  const isOn = status.enabledPreference && status.available;
  const showCustomSmtp = draft.customSmtp === true;
  const recognizedProvider = smtpProviderFor(draft.email)?.label.split(" · ")[0];

  async function handleSmtpTest() {
    const succeeded = await onSmtpTest(status.kind, draft);
    if (succeeded) onDraftChange({ ...draft, authorizationCode: "" });
  }

  function handleToggle() {
    if (!status.available) return;
    void onToggle(status.kind, !status.enabledPreference);
  }

  function toggleCustomSmtp() {
    onDraftChange({ ...draft, customSmtp: !showCustomSmtp });
  }

  return (
    <section className={`integration-card integration-card-${status.kind}`}>
      <div className="card-heading">
        <div className={`integration-icon integration-${status.kind}`}><Power size={19} strokeWidth={2.2} /></div>
        <div className="card-title-wrap"><h2>{status.displayName}</h2><p>{status.detail}</p></div>
        <button className="switch" type="button" role="switch" aria-checked={isOn} aria-label={`${status.displayName} 邮件通知`} disabled={busy || !status.available} title={!status.available ? "请先完成 SMTP 测试" : undefined} onClick={handleToggle}><span /></button>
      </div>

      <div className="channel-smtp">
        <div className="channel-heading"><h3>{status.displayName} SMTP 配置</h3><ShieldCheck size={17} /></div>
        <div className="smtp-form">
          <div className="smtp-main-fields">
            <label><span className="field-label"><span>{status.displayName} 邮箱地址</span>{recognizedProvider && <em className="recognized-provider">已识别：{recognizedProvider}</em>}</span><input type="email" value={draft.email} placeholder="请输入邮箱地址" onChange={(event) => onDraftChange({ ...draft, email: event.target.value })} /></label>
            <label><span>{status.displayName} 邮箱授权码</span><input type="password" value={draft.authorizationCode} placeholder={status.smtp.verified ? "已保存，重新输入可更换" : "请输入授权码"} onChange={(event) => onDraftChange({ ...draft, authorizationCode: event.target.value })} /></label>
          </div>
          <button className={`custom-smtp-toggle${showCustomSmtp ? " is-open" : ""}`} type="button" aria-label="自定义 SMTP" title="自定义 SMTP" aria-expanded={showCustomSmtp} onClick={toggleCustomSmtp} disabled={busy}>
            <Pencil size={17} />
          </button>
          {showCustomSmtp && <div className="custom-smtp-panel">
            <div className="custom-smtp-fields">
              <label><span>SMTP 服务器</span><input value={draft.customHost ?? ""} placeholder="例：smtp.example.org" onChange={(event) => onDraftChange({ ...draft, customHost: event.target.value })} /></label>
              <label><span>端口</span><input type="number" value={draft.customPort ?? ""} placeholder="465" onChange={(event) => onDraftChange({ ...draft, customPort: event.target.value === "" ? undefined : Number(event.target.value) })} /></label>
              <label><span>加密方式</span><EncryptionSelect value={draft.customEncryption ?? "ssl"} onChange={(value) => onDraftChange({ ...draft, customEncryption: value })} /></label>
            </div>
          </div>}
        </div>
        <div className="smtp-actions">
          <button className="primary-button" type="button" aria-label={`${status.displayName} SMTP 测试保存`} disabled={!draft.email || (!draft.authorizationCode && !status.smtp.verified) || busy} onClick={() => void handleSmtpTest()}><Mail size={16} />{busy ? "正在测试…" : "SMTP 测试保存"}</button>
        </div>
        {status.smtp.error && <p className="inline-error"><CircleAlert size={15} />{status.smtp.error}</p>}
      </div>

      <div className="channel-history">
        <div className="history-heading"><div><p className="eyebrow">最近活动</p><h3>{status.displayName} 最近活动</h3></div><button className="quiet-button" type="button" disabled={!status.history.length || busy} onClick={() => void onClearHistory(status.kind)}><Trash2 size={14} />清空记录</button></div>
        {status.history.length === 0 ? <EmptyHistory /> : <div className="history-list">{status.history.map((entry) => <article className="history-entry" key={entry.id}><span className={`history-result result-${entry.result}`}>{entry.result === "sent" ? <Check size={14} /> : <CircleAlert size={14} />}</span><div><strong>{entry.title}</strong><p>{entry.detail}</p></div><time>{entry.occurredAt}</time></article>)}</div>}
      </div>
    </section>
  );
}

export function App({ bridge }: AppProps) {
  const [dashboard, setDashboard] = useState<DashboardState | null>(null);
  const [drafts, setDrafts] = useState<Partial<Record<IntegrationKind, SmtpDraft>>>({});
  const [selectedKind, setSelectedKind] = useState<IntegrationKind>("codex");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);

  useEffect(() => {
    const preventContextMenu = (event: MouseEvent) => event.preventDefault();
    document.addEventListener("contextmenu", preventContextMenu);
    return () => document.removeEventListener("contextmenu", preventContextMenu);
  }, []);

  useEffect(() => {
    let active = true;
    const refresh = () => {
      bridge.getDashboard().then((next) => { if (active) setDashboard(next); }).catch((cause: unknown) => { if (active) setError(messageFromError(cause, "无法读取当前设置")); });
    };
    refresh();
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const subscription = bridge.subscribeDashboardChanges?.(refresh);
    void subscription?.then((cleanup) => {
      if (disposed) cleanup();
      else unlisten = cleanup;
    }, () => undefined);
    window.addEventListener("focus", refresh);
    document.addEventListener("visibilitychange", refresh);
    return () => { active = false; disposed = true; unlisten?.(); window.removeEventListener("focus", refresh); document.removeEventListener("visibilitychange", refresh); };
  }, [bridge]);

  useEffect(() => {
    if (!dashboard) return;
    setDrafts((current) => {
      let changed = false;
      const next = { ...current };
      for (const status of dashboard.integrations) {
        if (!next[status.kind]) {
          next[status.kind] = draftFromStatus(status);
          changed = true;
        }
      }
      return changed ? next : current;
    });
  }, [dashboard]);

  async function run(action: () => Promise<DashboardState>): Promise<boolean> {
    setBusy(true);
    setError(null);
    try { setDashboard(await action()); return true; }
    catch (cause: unknown) { setError(messageFromError(cause)); return false; }
    finally { setBusy(false); }
  }

  async function handleSmtpTest(kind: IntegrationKind, draft: SmtpDraft): Promise<boolean> {
    const succeeded = await run(() => bridge.saveAndTestSmtp(kind, draft));
    if (succeeded) { setToast("SMTP 测试邮件已发送"); setTimeout(() => setToast(null), 5000); }
    return succeeded;
  }

  async function handleToggle(kind: IntegrationKind, enabled: boolean) { await run(() => bridge.setIntegrationEnabled(kind, enabled)); }
  async function handleClearHistory(kind: IntegrationKind) { await run(() => bridge.clearHistory(kind)); }

  if (!dashboard) return <main className="app-shell loading-shell"><div className="loading-mark"><Mail size={20} />正在读取设置…</div></main>;

  const selectedStatus = dashboard.integrations.find((status) => status.kind === selectedKind) ?? dashboard.integrations[0];
  if (!selectedStatus) return <main className="app-shell loading-shell"><div className="loading-mark"><Mail size={20} />暂无可用客户端</div></main>;
  const selectedDraft = drafts[selectedStatus.kind] ?? draftFromStatus(selectedStatus);

  return (
    <main className="app-shell">
      <header className="app-header"><div className="brand-lockup"><div className="brand-mark"><Mail size={20} strokeWidth={2.2} /></div><div><h1>Agent 邮件通知</h1></div></div><div className="header-actions"><div className="client-tabs" role="tablist" aria-label="选择桌面客户端">{dashboard.integrations.map((status) => <button key={status.kind} className="client-tab" type="button" role="tab" aria-selected={selectedKind === status.kind} onClick={() => setSelectedKind(status.kind)}>{status.kind === "codex" ? "Codex Desktop" : "Claude Desktop"}</button>)}</div><span className="runtime-indicator"><span className="runtime-dot" />程序正在运行</span></div></header>
      <div className="content-grid">
        <section className="integrations-section">
          <div className="integration-grid"><IntegrationCard key={selectedStatus.kind} status={selectedStatus} draft={selectedDraft} onDraftChange={(draft) => setDrafts((current) => ({ ...current, [selectedStatus.kind]: draft }))} busy={busy} onSmtpTest={handleSmtpTest} onToggle={handleToggle} onClearHistory={handleClearHistory} /></div>
        </section>
      </div>
      {toast && <div className="success-toast" role="status"><Check size={16} />{toast}</div>}
      {error && <div className="toast-error" role="alert"><CircleAlert size={16} />{error}<button type="button" onClick={() => setError(null)}>关闭</button></div>}
    </main>
  );
}
