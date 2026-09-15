import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, test } from "vitest";
import type { DashboardState, NotifierBridge } from "./domain";
import { App } from "./App";

afterEach(() => cleanup());

function smtp(email = "") {
  return { email, provider: null, providerLabel: "等待配置", host: null, port: null, encryption: null, verified: false, lastTestedAt: null, error: null };
}

const initialState: DashboardState = {
  integrations: [
    { kind: "codex", displayName: "Codex", enabledPreference: true, available: false, detail: "请先完成 SMTP 测试", smtp: smtp(), history: [] },
    { kind: "claude", displayName: "Claude Code", enabledPreference: false, available: false, detail: "请先完成 SMTP 测试", smtp: smtp(), history: [] },
  ],
};

function bridgeReturning(state: DashboardState, overrides: Partial<NotifierBridge> = {}): NotifierBridge {
  return {
    getDashboard: async () => state,
    saveAndTestSmtp: async () => state,
    setIntegrationEnabled: async () => state,
    clearHistory: async () => state,
    ...overrides,
  };
}

test("shows one selected desktop client homepage at a time", async () => {
  const user = userEvent.setup();
  render(<App bridge={bridgeReturning(initialState)} />);

  expect(await screen.findByRole("heading", { name: "Agent 邮件通知" })).toBeInTheDocument();
  expect(screen.queryByRole("heading", { name: "邮箱与 SMTP" })).not.toBeInTheDocument();
  expect(screen.getByRole("tab", { name: "Codex Desktop" })).toHaveAttribute("aria-selected", "true");
  expect(screen.getByRole("tab", { name: "Claude Desktop" })).toHaveAttribute("aria-selected", "false");
  expect(screen.getByRole("heading", { name: "Codex" })).toBeInTheDocument();
  expect(screen.getByLabelText("Codex 邮箱地址")).toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "Codex 最近活动" })).toBeInTheDocument();
  expect(screen.queryByLabelText("Claude Code 邮箱地址")).not.toBeInTheDocument();

  await user.click(screen.getByRole("tab", { name: "Claude Desktop" }));

  expect(screen.getByRole("tab", { name: "Claude Desktop" })).toHaveAttribute("aria-selected", "true");
  expect(screen.getByRole("heading", { name: "Claude Code" })).toBeInTheDocument();
  expect(screen.getByLabelText("Claude Code 邮箱地址")).toBeInTheDocument();
  expect(screen.queryByLabelText("Codex 邮箱地址")).not.toBeInTheDocument();
});

test("opens advanced SMTP fields only when custom SMTP is requested", async () => {
  const user = userEvent.setup();
  render(<App bridge={bridgeReturning(initialState)} />);

  const email = await screen.findByLabelText("Codex 邮箱地址");
  await user.type(email, ["owner", "example.org"].join("@"));
  expect(screen.queryByLabelText("SMTP 服务器")).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: /自定义 SMTP/ }));
  expect(screen.getByLabelText("SMTP 服务器")).toBeInTheDocument();
  expect(screen.getByLabelText("端口")).toBeInTheDocument();
  expect(screen.getByLabelText("加密方式")).toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: /自定义 SMTP/ }));
  await user.clear(email);
  await user.type(email, ["tester", "qq.com"].join("@"));
  expect(screen.queryByLabelText("SMTP 服务器")).not.toBeInTheDocument();
});

test("passes the target channel to SMTP test and clears the authorization field after success", async () => {
  const user = userEvent.setup();
  let receivedKind = "";
  const bridge = bridgeReturning(initialState, {
    saveAndTestSmtp: async (kind) => { receivedKind = kind; return initialState; },
  });
  render(<App bridge={bridge} />);

  const email = await screen.findByLabelText("Codex 邮箱地址");
  const code = screen.getByLabelText("Codex 邮箱授权码");
  await user.type(email, ["tester", "qq.com"].join("@"));
  await user.type(code, "temporary-code");
  await user.click(screen.getByRole("button", { name: "Codex SMTP 测试保存" }));

  expect(receivedKind).toBe("codex");
  expect(code).toHaveValue("");
});

test("shows a string error returned by the bridge instead of replacing it with a generic message", async () => {
  const user = userEvent.setup();
  const bridge = bridgeReturning(initialState, {
    saveAndTestSmtp: async () => { throw "SMTP 身份验证失败"; },
  });
  render(<App bridge={bridge} />);

  const email = await screen.findByLabelText("Codex 邮箱地址");
  await user.type(email, ["tester", "qq.com"].join("@"));
  await user.type(screen.getByLabelText("Codex 邮箱授权码"), "temporary-code");
  await user.click(screen.getByRole("button", { name: "Codex SMTP 测试保存" }));

  expect(await screen.findByRole("alert")).toHaveTextContent("SMTP 身份验证失败");
  expect(screen.queryByText("操作失败")).not.toBeInTheDocument();
});

test("clears only the selected client's activity", async () => {
  const user = userEvent.setup();
  const state: DashboardState = {
    integrations: [
      { ...initialState.integrations[0], history: [{ id: "codex-entry", source: "Codex", title: "Codex task", result: "sent", detail: "sent", occurredAt: "now" }] },
      { ...initialState.integrations[1], history: [{ id: "claude-entry", source: "Claude Code", title: "Claude task", result: "sent", detail: "sent", occurredAt: "now" }] },
    ],
  };
  let clearedKind = "";
  const clearedState: DashboardState = { integrations: [{ ...state.integrations[0], history: [] }, state.integrations[1]] };
  const bridge = bridgeReturning(state, {
    clearHistory: async (kind) => { clearedKind = kind; return clearedState; },
  });
  render(<App bridge={bridge} />);

  await screen.findByRole("heading", { name: "Codex 最近活动" });
  await user.click(screen.getByRole("button", { name: "清空记录" }));

  expect(clearedKind).toBe("codex");
  expect(screen.queryByText("Codex task")).not.toBeInTheDocument();
  expect(screen.queryByText("Claude task")).not.toBeInTheDocument();

  await user.click(screen.getByRole("tab", { name: "Claude Desktop" }));
  expect(screen.getByText("Claude task")).toBeInTheDocument();
});

test("always shows the notification switch without an install action", async () => {
  const user = userEvent.setup();
  render(<App bridge={bridgeReturning(initialState)} />);

  expect(await screen.findByRole("switch", { name: "Codex 邮件通知" })).toBeDisabled();
  expect(screen.queryByRole("button", { name: "安装通知" })).not.toBeInTheDocument();

  await user.click(screen.getByRole("tab", { name: "Claude Desktop" }));
  expect(screen.getByRole("switch", { name: "Claude Code 邮件通知" })).toBeDisabled();
  expect(screen.queryByRole("button", { name: "安装通知" })).not.toBeInTheDocument();
});

test("refreshes dashboard when the hidden homepage receives focus again", async () => {
  let latest = initialState;
  const bridge = bridgeReturning(initialState, {
    getDashboard: async () => latest,
  });
  render(<App bridge={bridge} />);

  await screen.findByRole("heading", { name: "Agent 邮件通知" });
  latest = {
    integrations: [
      { ...initialState.integrations[0], history: [{ id: "tray-entry", source: "Codex", title: "Tray update", result: "sent", detail: "sent", occurredAt: "now" }] },
      initialState.integrations[1],
    ],
  };
  window.dispatchEvent(new Event("focus"));

  expect(await screen.findByText("Tray update")).toBeInTheDocument();
});

test("refreshes dashboard when the tray changes a notification preference", async () => {
  let latest = initialState;
  let notify: (() => void) | undefined;
  const bridge = bridgeReturning(initialState, {
    getDashboard: async () => latest,
    subscribeDashboardChanges: async (handler) => {
      notify = handler;
      return () => undefined;
    },
  });
  render(<App bridge={bridge} />);

  await screen.findByRole("heading", { name: "Agent 邮件通知" });
  latest = {
    integrations: [
      { ...initialState.integrations[0], available: true, enabledPreference: false, smtp: { ...smtp("account"), verified: true } },
      initialState.integrations[1],
    ],
  };
  notify?.();

  expect(await screen.findByRole("switch", { name: "Codex 邮件通知" })).toHaveAttribute("aria-checked", "false");
});
