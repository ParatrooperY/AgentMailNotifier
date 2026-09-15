import type { ProviderKind } from "./domain";

export interface ProviderPreset {
  kind: ProviderKind;
  label: string;
  host: string;
  port: number;
  encryption: "ssl" | "starttls";
}

const PRESETS: Array<{ domains: string[]; preset: ProviderPreset }> = [
  { domains: ["qq.com", "vip.qq.com", "foxmail.com"], preset: { kind: "qq", label: "QQ 邮箱 · SSL 465", host: "smtp.qq.com", port: 465, encryption: "ssl" } },
  { domains: ["163.com", "126.com", "yeah.net"], preset: { kind: "163", label: "网易邮箱 · SSL 465", host: "smtp.163.com", port: 465, encryption: "ssl" } },
  { domains: ["outlook.com", "hotmail.com", "live.com"], preset: { kind: "outlook", label: "Outlook · STARTTLS 587", host: "smtp.office365.com", port: 587, encryption: "starttls" } },
  { domains: ["gmail.com"], preset: { kind: "gmail", label: "Gmail · SSL 465", host: "smtp.gmail.com", port: 465, encryption: "ssl" } },
];

export function smtpProviderFor(email: string): ProviderPreset | null {
  const domain = email.trim().toLowerCase().split("@").at(1);
  if (!domain) return null;
  return PRESETS.find(({ domains }) => domains.includes(domain))?.preset ?? null;
}
