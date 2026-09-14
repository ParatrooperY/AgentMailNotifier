export type ProviderKind = "qq" | "163" | "outlook" | "gmail" | "custom";
export type IntegrationKind = "codex" | "claude";
export type HealthTone = "ready" | "warning" | "error" | "muted";

export interface SmtpStatus {
  email: string;
  provider: ProviderKind | null;
  providerLabel: string;
  host: string | null;
  port: number | null;
  encryption: "ssl" | "starttls" | null;
  verified: boolean;
  lastTestedAt: string | null;
  error: string | null;
}

export interface IntegrationStatus {
  kind: IntegrationKind;
  displayName: string;
  installed: boolean;
  enabledPreference: boolean;
  available: boolean;
  detail: string;
  tone: HealthTone;
  smtp: SmtpStatus;
  history: HistoryEntry[];
}

export interface HistoryEntry {
  id: string;
  source: "Codex" | "Claude Code";
  title: string;
  result: "sent" | "failed";
  detail: string;
  occurredAt: string;
}

export interface DashboardState {
  integrations: IntegrationStatus[];
}

export interface SmtpDraft {
  email: string;
  authorizationCode: string;
  customSmtp?: boolean;
  customHost?: string;
  customPort?: number;
  customEncryption?: "ssl" | "starttls";
}

export interface NotifierBridge {
  getDashboard(): Promise<DashboardState>;
  subscribeDashboardChanges?: (handler: () => void) => Promise<() => void>;
  saveAndTestSmtp(kind: IntegrationKind, draft: SmtpDraft): Promise<DashboardState>;
  setIntegrationEnabled(
    kind: IntegrationKind,
    enabled: boolean,
  ): Promise<DashboardState>;
  clearHistory(kind: IntegrationKind): Promise<DashboardState>;
  openLogFolder(): Promise<void>;
  exitApplication(): Promise<void>;
}
