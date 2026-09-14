import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  DashboardState,
  IntegrationKind,
  NotifierBridge,
  SmtpDraft,
} from "./domain";

export const tauriBridge: NotifierBridge = {
  getDashboard: () => invoke("get_dashboard"),
  subscribeDashboardChanges: (handler) => listen("dashboard-changed", handler),
  saveAndTestSmtp: (kind: IntegrationKind, draft: SmtpDraft) =>
    invoke("save_and_test_smtp", { kind, draft }),
  setIntegrationEnabled: (kind: IntegrationKind, enabled: boolean) =>
    invoke("set_integration_enabled", { kind, enabled }),
  clearHistory: (kind: IntegrationKind) => invoke("clear_history", { kind }),
  openLogFolder: () => invoke("open_log_folder"),
  exitApplication: () => invoke("exit_application"),
};
