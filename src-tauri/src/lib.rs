use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use agent_mail_notifier_core::{
    IntegrationKind, SmtpEncryption, codex_hook_file_is_empty, restore_claude_hooks_if_owned,
    restore_codex_hook_if_owned, restore_codex_timing_hook_if_owned, smtp_preset_for,
};
use chrono::{DateTime, Local};
use keyring::Entry;
use lettre::{
    Message, SmtpTransport, Transport,
    transport::smtp::{
        authentication::Credentials,
        client::{Tls, TlsParameters},
    },
};
use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, Manager, State,
    menu::{CheckMenuItem, Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

mod state;

use state::{
    HistoryEntry, StoredSmtp, StoredState, channel,
    channel_mut, channel_name, credential_service_name, mark_disconnected_for_uninstall,
    migrate_legacy_state, source_name,
};

const KEYRING_SERVICE: &str = "agent-mail-notifier";
const MAX_CODEX_DELIVERED_EVENTS: usize = 2_000;

#[derive(Default, Deserialize)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
struct SmtpDraft {
    email: String,
    authorization_code: String,
    custom_smtp: bool,
    custom_host: Option<String>,
    custom_port: Option<u16>,
    custom_encryption: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IntegrationView {
    kind: String,
    display_name: String,
    smtp: StoredSmtp,
    installed: bool,
    enabled_preference: bool,
    available: bool,
    detail: String,
    tone: String,
    history: Vec<HistoryEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Dashboard {
    integrations: Vec<IntegrationView>,
}

struct AppState {
    path: PathBuf,
    value: Mutex<StoredState>,
    tray: Mutex<Option<TrayControls>>,
}

#[derive(Default)]
struct RolloutCursor {
    offset: u64,
    incomplete_line: Vec<u8>,
    created_at: Option<std::time::SystemTime>,
}

struct CodexRolloutCompletion {
    thread_id: String,
    turn_id: String,
    problem: String,
    detail: String,
    working_directory: Option<String>,
    elapsed: Option<Duration>,
}

struct ClaudeTranscriptCompletion {
    completion_id: String,
    chat: Option<String>,
    problem: String,
    detail: String,
    working_directory: Option<String>,
    elapsed: Option<Duration>,
}

#[derive(Clone)]
struct TrayControls {
    codex: CheckMenuItem<tauri::Wry>,
    claude: CheckMenuItem<tauri::Wry>,
}

fn sync_tray_controls(controls: &TrayControls, stored: &StoredState) {
    let _ = controls.codex.set_checked(stored.codex.integration.enabled_preference);
    let _ = controls.claude.set_checked(stored.claude.integration.enabled_preference);
    let _ = controls.codex.set_enabled(stored.codex.smtp.verified);
    let _ = controls.claude.set_enabled(stored.claude.smtp.verified);
}

impl AppState {
    fn load() -> Self {
        let root = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("AgentMailNotifier");
        let path = root.join("settings.json");
        let (value, migrated) = match fs::read_to_string(&path) {
            Ok(text) => migrate_legacy_state(&text).unwrap_or_else(|_| (StoredState::current(), true)),
            Err(_) => (StoredState::current(), false),
        };
        let state = Self { path, value: Mutex::new(value), tray: Mutex::new(None) };
        if migrated {
            let credential_migration = state.value.lock().ok().map(|value| migrate_legacy_credentials(&value));
            if !matches!(credential_migration, Some(LegacyCredentialMigration::Failed)) {
                if let Ok(value) = state.value.lock() {
                    if state.save(&value).is_ok() {
                        if let Some(LegacyCredentialMigration::Copied(email)) = credential_migration {
                            delete_legacy_credential(&email);
                        }
                    }
                }
            }
        }
        state
    }

    fn save(&self, value: &StoredState) -> Result<(), String> {
        let parent = self.path.parent().ok_or("无效的设置路径")?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let temporary = self.path.with_extension("json.tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
        fs::rename(temporary, &self.path).map_err(|error| error.to_string())
    }

    fn runtime_path(&self) -> PathBuf { self.path.with_file_name("runtime.json") }

    fn sync_tray(&self, stored: &StoredState) {
        let Ok(guard) = self.tray.lock() else { return; };
        let Some(controls) = guard.as_ref() else { return; };
        sync_tray_controls(controls, stored);
    }
}

fn view(kind: IntegrationKind, state: &StoredState) -> IntegrationView {
    let current = channel(state, kind);
    let ready = current.smtp.verified;
    let (detail, tone) = if !current.smtp.verified {
        ("请先完成 SMTP 测试".to_owned(), "muted".to_owned())
    } else if current.integration.enabled_preference {
        ("程序运行时会发送完成邮件".to_owned(), "ready".to_owned())
    } else {
        ("邮件通知已关闭".to_owned(), "muted".to_owned())
    };
    IntegrationView {
        kind: channel_name(kind).to_owned(),
        display_name: source_name(kind).to_owned(),
        smtp: current.smtp.clone(),
        installed: true,
        enabled_preference: current.integration.enabled_preference,
        available: ready,
        detail,
        tone,
        history: current.history.iter().cloned().collect(),
    }
}

fn dashboard(state: &StoredState) -> Dashboard {
    Dashboard { integrations: vec![view(IntegrationKind::Codex, state), view(IntegrationKind::Claude, state)] }
}

#[tauri::command]
fn get_dashboard(state: State<'_, Arc<AppState>>) -> Result<Dashboard, String> {
    let stored = state.value.lock().map_err(|_| "设置锁不可用")?;
    Ok(dashboard(&stored))
}

#[tauri::command]
fn credential_service(kind: IntegrationKind) -> String {
    credential_service_name(kind)
}

fn credential_entry(kind: IntegrationKind, email: &str) -> Result<Entry, String> {
    Entry::new(&credential_service(kind), email).map_err(|error| error.to_string())
}

fn legacy_credential_entry(email: &str) -> Result<Entry, String> {
    Entry::new(KEYRING_SERVICE, email).map_err(|error| error.to_string())
}

trait CredentialStore {
    fn read_password(&self, kind: IntegrationKind, email: &str) -> Result<Option<String>, String>;
    fn write_password(&self, kind: IntegrationKind, email: &str, password: &str) -> Result<(), String>;
    fn delete_password(&self, kind: IntegrationKind, email: &str) -> Result<(), String>;
}

struct KeyringCredentialStore;

impl CredentialStore for KeyringCredentialStore {
    fn read_password(&self, kind: IntegrationKind, email: &str) -> Result<Option<String>, String> {
        let entry = credential_entry(kind, email).map_err(|_| "无法访问授权码存储，请稍后重试".to_owned())?;
        match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err("无法访问授权码存储，请稍后重试".to_owned()),
        }
    }

    fn write_password(&self, kind: IntegrationKind, email: &str, password: &str) -> Result<(), String> {
        credential_entry(kind, email)
            .map_err(|_| "无法访问授权码存储，请稍后重试".to_owned())?
            .set_password(password)
            .map_err(|_| "无法保存授权码，请稍后重试".to_owned())
    }

    fn delete_password(&self, kind: IntegrationKind, email: &str) -> Result<(), String> {
        let entry = credential_entry(kind, email).map_err(|_| "无法访问授权码存储，请稍后重试".to_owned())?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err("无法恢复原授权码，请重新输入后重试".to_owned()),
        }
    }
}

enum LegacyCredentialMigration {
    NotNeeded,
    Copied(String),
    Failed,
}

fn migrate_legacy_credentials(state: &StoredState) -> LegacyCredentialMigration {
    let legacy_password = |email: &str| {
        let Ok(legacy) = legacy_credential_entry(email) else { return Err("无法访问旧版授权码存储".to_owned()); };
        match legacy.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err("无法访问旧版授权码存储".to_owned()),
        }
    };
    migrate_legacy_credentials_with_store(state, &KeyringCredentialStore, legacy_password)
}

fn migrate_legacy_credentials_with_store(
    state: &StoredState,
    credential_store: &impl CredentialStore,
    read_legacy_password: impl FnOnce(&str) -> Result<Option<String>, String>,
) -> LegacyCredentialMigration {
    let Some(email) = (!state.codex.smtp.email.is_empty()).then(|| state.codex.smtp.email.clone()) else { return LegacyCredentialMigration::NotNeeded; };
    if state.claude.smtp.email != email { return LegacyCredentialMigration::NotNeeded; }
    let password = match read_legacy_password(&email) {
        Ok(Some(password)) => password,
        Ok(None) => return LegacyCredentialMigration::NotNeeded,
        Err(_) => return LegacyCredentialMigration::Failed,
    };
    for kind in [IntegrationKind::Codex, IntegrationKind::Claude] {
        match credential_store.read_password(kind, &email) {
            Ok(Some(_)) => {}
            Ok(None) if credential_store.write_password(kind, &email, &password).is_ok() => {}
            _ => return LegacyCredentialMigration::Failed,
        }
    }
    LegacyCredentialMigration::Copied(email)
}

fn delete_legacy_credential(email: &str) {
    if let Ok(entry) = legacy_credential_entry(email) {
        let _ = entry.delete_credential();
    }
}

fn saved_password(kind: IntegrationKind, email: &str) -> Result<String, String> {
    credential_entry(kind, email)
        .and_then(|entry| entry.get_password().map_err(|error| error.to_string()))
        .or_else(|_| {
            legacy_credential_entry(email)
                .and_then(|entry| entry.get_password().map_err(|error| error.to_string()))
        })
        .map_err(|_| "找不到已保存的授权码，请重新输入".to_owned())
}

fn smtp_error(error: String) -> String {
    let lower = error.to_ascii_lowercase();
    if lower.contains("authentication") || lower.contains("credential") || lower.contains("login") {
        "SMTP 身份验证失败，请检查邮箱和授权码".to_owned()
    } else if lower.contains("timed out") || lower.contains("timeout") {
        "SMTP 连接超时，请检查网络、服务器和端口".to_owned()
    } else if lower.contains("certificate") || lower.contains("tls") || lower.contains("ssl") {
        "SMTP 安全连接失败，请检查加密方式和服务器设置".to_owned()
    } else if lower.contains("connection") || lower.contains("connect") || lower.contains("network") || lower.contains("dns") || lower.contains("resolve") {
        "SMTP 无法连接服务器，请检查网络、服务器和端口".to_owned()
    } else if lower.contains("mailbox") || lower.contains("address") || lower.contains("invalid") {
        "SMTP 配置无效，请检查邮箱地址、服务器和端口".to_owned()
    } else {
        "SMTP 服务器拒绝了请求，请检查服务器设置和授权码".to_owned()
    }
}

fn persist_verified_smtp(
    stored: &mut StoredState,
    kind: IntegrationKind,
    next_smtp: StoredSmtp,
    password: &str,
    credential_store: &impl CredentialStore,
    save: impl Fn(&StoredState) -> Result<(), String>,
) -> Result<(), String> {
    let next_email = next_smtp.email.trim().to_owned();
    let previous_smtp = channel(stored, kind).smtp.clone();
    let previous_email = previous_smtp.email.trim().to_owned();
    let previous_password = if previous_email.is_empty() {
        None
    } else {
        credential_store.read_password(kind, &previous_email)?
    };
    let next_password = if previous_email.eq_ignore_ascii_case(&next_email) {
        previous_password.clone()
    } else if next_email.is_empty() {
        None
    } else {
        credential_store.read_password(kind, &next_email)?
    };

    credential_store.write_password(kind, &next_email, password)?;
    let email_changed = !previous_email.is_empty() && !previous_email.eq_ignore_ascii_case(&next_email);
    if email_changed {
        if let Err(error) = credential_store.delete_password(kind, &previous_email) {
            let restored = match next_password {
                Some(previous) => credential_store.write_password(kind, &next_email, &previous),
                None => credential_store.delete_password(kind, &next_email),
            };
            return if restored.is_ok() { Err(error) } else { Err("无法清理旧授权码，请手动检查后重试".to_owned()) };
        }
    }

    channel_mut(stored, kind).smtp = next_smtp;
    if let Err(error) = save(stored) {
        channel_mut(stored, kind).smtp = previous_smtp;
        let restored_old = match previous_password {
            Some(previous) if !previous_email.is_empty() => credential_store.write_password(kind, &previous_email, &previous),
            _ if !previous_email.is_empty() => credential_store.delete_password(kind, &previous_email),
            _ => Ok(()),
        };
        let restored_next = if next_email.is_empty() {
            Ok(())
        } else {
            match next_password {
                Some(previous) => credential_store.write_password(kind, &next_email, &previous),
                None => credential_store.delete_password(kind, &next_email),
            }
        };
        return if restored_old.is_ok() && restored_next.is_ok() {
            Err(error)
        } else {
            Err("无法保存 SMTP 设置，也无法恢复原授权码，请重新输入后重试".to_owned())
        };
    }
    Ok(())
}

#[tauri::command]
fn save_and_test_smtp(kind: IntegrationKind, draft: SmtpDraft, state: State<'_, Arc<AppState>>) -> Result<Dashboard, String> {
    let email = draft.email.trim().to_owned();
    if email.is_empty() { return Err("请输入邮箱".into()); }
    let previous = {
        let stored = state.value.lock().map_err(|_| "设置锁不可用")?;
        channel(&stored, kind).smtp.clone()
    };
    let password = if draft.authorization_code.is_empty() {
        if previous.verified && previous.email.eq_ignore_ascii_case(&email) {
            saved_password(kind, &email)?
        } else {
            return Err("请输入邮箱对应的授权码".into());
        }
    } else {
        draft.authorization_code.clone()
    };
    let preset = if draft.custom_smtp { None } else { smtp_preset_for(&email) };
    let (provider, provider_label, host, port, encryption) = if let Some(preset) = preset {
        (Some("preset".to_owned()), preset.provider_label.to_owned(), preset.host.to_owned(), preset.port, preset.encryption)
    } else {
        if !draft.custom_smtp {
            return Err("请先打开“自定义 SMTP”并填写服务器参数".into());
        }
        let host = draft.custom_host.filter(|value| !value.trim().is_empty()).ok_or("请填写 SMTP 服务器")?;
        let port = draft.custom_port.unwrap_or(465);
        let encryption = match draft.custom_encryption.as_deref() { Some("starttls") => SmtpEncryption::Starttls, _ => SmtpEncryption::Ssl };
        (Some("custom".to_owned()), "自定义 SMTP 配置".to_owned(), host, port, encryption)
    };
    test_smtp(&email, &password, &host, port, encryption)?;
    let mut stored = state.value.lock().map_err(|_| "设置锁不可用")?;
    let next_smtp = StoredSmtp { email, provider, provider_label, host: Some(host), port: Some(port), encryption: Some(match encryption { SmtpEncryption::Ssl => "ssl", SmtpEncryption::Starttls => "starttls" }.into()), verified: true, last_tested_at: Some(Local::now().format("%Y-%m-%d %H:%M").to_string()), error: None };
    persist_verified_smtp(&mut stored, kind, next_smtp, &password, &KeyringCredentialStore, |next| state.save(next))?;
    state.sync_tray(&stored);
    Ok(dashboard(&stored))
}

fn test_smtp(email: &str, password: &str, host: &str, port: u16, encryption: SmtpEncryption) -> Result<(), String> {
    let transport = smtp_transport(email, password, host, port, encryption)?;
    let mailbox = email.parse::<lettre::message::Mailbox>().map_err(|_| "邮箱格式无效，请检查邮箱地址".to_owned())?;
    let message = Message::builder().from(mailbox.clone()).to(mailbox).subject("[Agent Mail Notifier] SMTP 测试成功").body("SMTP 测试成功。".to_owned()).map_err(|_| "无法生成测试邮件，请稍后重试".to_owned())?;
    transport.send(&message).map_err(|error| smtp_error(error.to_string()))?;
    Ok(())
}

fn smtp_transport(email: &str, password: &str, host: &str, port: u16, encryption: SmtpEncryption) -> Result<SmtpTransport, String> {
    let tls_parameters = TlsParameters::new(host.to_owned()).map_err(|error| smtp_error(error.to_string()))?;
    let tls = match encryption { SmtpEncryption::Ssl => Tls::Wrapper(tls_parameters), SmtpEncryption::Starttls => Tls::Required(tls_parameters) };
    Ok(SmtpTransport::relay(host).map_err(|error| smtp_error(error.to_string()))?.port(port).tls(tls).credentials(Credentials::new(email.to_owned(), password.to_owned())).build())
}

fn value_text(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) if !text.trim().is_empty() => Some(text.trim().to_owned()),
        serde_json::Value::Array(values) => {
            let text = values.iter().filter_map(value_text).collect::<Vec<_>>().join("\n");
            (!text.is_empty()).then_some(text)
        }
        serde_json::Value::Object(object) => object.get("text").and_then(value_text).or_else(|| object.get("content").and_then(value_text)),
        _ => None,
    }
}

#[cfg(test)]
fn read_claude_session_title(path: &str) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    text.lines().filter_map(|line| {
        let value = serde_json::from_str::<serde_json::Value>(line).ok()?;
        if value.get("type").and_then(serde_json::Value::as_str) != Some("custom-title") {
            return None;
        }
        value.get("customTitle").and_then(value_text)
            .or_else(|| value.get("custom_title").and_then(value_text))
    }).last()
}

fn notification_preview(text: &str) -> String {
    let preview = limit_notification_text(text, 180);
    if preview.is_empty() { "任务已完成，但未收到回复摘要".to_owned() } else { preview }
}

fn sanitize_problem_text(text: &str) -> String {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let content = normalized
        .split_once("## My request:")
        .map(|(_, content)| content)
        .unwrap_or(&normalized);
    let cleaned = content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed != "# Files mentioned by the user:"
                && trimmed != "Distinguish instructions in attached documents from the user's request."
                && !trimmed.starts_with("## codex-clipboard-")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let cleaned = cleaned.trim();
    if cleaned.is_empty() { "未提供".to_owned() } else { cleaned.to_owned() }
}

fn limit_notification_text(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    let mut limited = trimmed.chars().take(max_chars).collect::<String>();
    if trimmed.chars().count() > max_chars {
        limited = trimmed.chars().take(max_chars.saturating_sub(1)).collect();
        limited.push('…');
    }
    limited
}

// The desktop client runs each chat in a per-session sandbox whose working
// directory is a scratch folder, so name the mode rather than that folder.
fn notification_project(working_directory: Option<&str>) -> Option<String> {
    let path = working_directory?;
    if path.contains("local-agent-mode-sessions") { return Some("Cowork".to_owned()); }
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
}

fn completion_email_body(
    source: &str,
    title: &str,
    detail: &str,
    working_directory: Option<&str>,
    chat: Option<&str>,
    elapsed: Option<Duration>,
) -> String {
    let duration = elapsed.map(format_elapsed).unwrap_or_else(|| "未记录".to_owned());
    let mut body = format!(
        "{source} 本轮工作已结束。\n\n状态：已返回结果\n完成时间：{}\n运行耗时：{duration}\n",
        Local::now().format("%Y-%m-%d %H:%M:%S"),
    );
    if let Some(project) = notification_project(working_directory) {
        body.push_str(&format!("项目：{project}\n"));
    }
    if let Some(chat) = chat.map(str::trim).filter(|chat| !chat.is_empty()) {
        body.push_str(&format!("聊天：{chat}\n"));
    }
    body.push_str(&format!("问题：{title}\n\n回复：\n{detail}"));
    body
}

fn send_completion_email(
    kind: IntegrationKind,
    smtp: &StoredSmtp,
    title: &str,
    detail: &str,
    working_directory: Option<&str>,
    chat: Option<&str>,
    elapsed: Option<Duration>,
) -> Result<(), String> {
    let title = limit_notification_text(title, 50);
    let email = &smtp.email;
    let password = saved_password(kind, email)?;
    let host = smtp.host.as_deref().ok_or("缺少 SMTP 服务器")?;
    let port = smtp.port.ok_or("缺少 SMTP 端口")?;
    let encryption = if smtp.encryption.as_deref() == Some("starttls") { SmtpEncryption::Starttls } else { SmtpEncryption::Ssl };
    let transport = smtp_transport(email, &password, host, port, encryption)?;
    let source = source_name(kind);
    let mailbox = email.parse::<lettre::message::Mailbox>().map_err(|_| "邮箱格式无效，请检查邮箱地址".to_owned())?;
    let body = completion_email_body(source, &title, detail, working_directory, chat, elapsed);
    let message = Message::builder().from(mailbox.clone()).to(mailbox).subject(format!("[{source}] {title}")).body(body).map_err(|_| "无法生成通知邮件，请稍后重试".to_owned())?;
    transport.send(&message).map_err(|error| smtp_error(error.to_string()))?;
    Ok(())
}

fn run_hook_mode() -> bool {
    let mut arguments = std::env::args().skip(1);
    let Some(mode) = arguments.next() else { return false; };
    match mode.as_str() {
        "--uninstall-cleanup" => {
            let purge = arguments.next().as_deref() == Some("--purge");
            if cleanup_application(purge).is_err() { std::process::exit(1); }
            true
        }
        _ => false,
    }
}

fn app_data_root() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("AgentMailNotifier")
}

fn stop_running_instances() {
    let Ok(executable) = std::env::current_exe() else { return; };
    let Some(name) = executable.file_name().and_then(|value| value.to_str()) else { return; };
    let current_pid = std::process::id().to_string();
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/T", "/FI", &format!("IMAGENAME eq {name}"), "/FI", &format!("PID ne {current_pid}")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    thread::sleep(Duration::from_millis(250));
}

fn restore_owned_hooks(state: &StoredState, executable: &str) -> Result<(), String> {
    let home = std::env::var_os("USERPROFILE").map(PathBuf::from).ok_or("找不到用户目录")?;
    let codex_path = home.join(".codex").join("config.toml");
    if let Ok(original) = fs::read_to_string(&codex_path) {
        let previous = state.codex.integration.codex_previous_notify.as_deref();
        let restored = restore_codex_hook_if_owned(&original, previous, executable)?;
        if restored != original { write_with_backup(&codex_path, restored)?; }
    }
    let codex_hooks_path = home.join(".codex").join("hooks.json");
    if let Ok(original) = fs::read_to_string(&codex_hooks_path) {
        let command = codex_timing_command(executable);
        let restored = restore_codex_timing_hook_if_owned(&original, &command)?;
        if restored != original {
            if state.codex.integration.codex_timing_hook_created && codex_hook_file_is_empty(&restored)? {
                remove_exact_file(&codex_hooks_path)?;
            } else {
                write_with_backup(&codex_hooks_path, restored)?;
            }
        }
    }
    let claude_path = home.join(".claude").join("settings.json");
    if let Ok(original) = fs::read_to_string(&claude_path) {
        let restored = restore_claude_hooks_if_owned(&original, executable)?;
        if restored != original { write_with_backup(&claude_path, restored)?; }
    }
    Ok(())
}

fn format_elapsed(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds < 60 { return format!("{seconds}秒"); }
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    if minutes < 60 { return format!("{minutes}分{seconds}秒"); }
    format!("{}小时{}分{}秒", minutes / 60, minutes % 60, seconds)
}

fn read_codex_chat_title(path: &Path, thread_id: &str) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    text.lines().filter_map(|line| {
        let value = serde_json::from_str::<serde_json::Value>(line).ok()?;
        if value.get("id").and_then(serde_json::Value::as_str) != Some(thread_id) { return None; }
        value.get("thread_name").and_then(serde_json::Value::as_str).filter(|name| !name.trim().is_empty()).map(str::to_owned)
    }).last()
}

fn collect_codex_rollouts(root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else { return; };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_codex_rollouts(&path, files);
        } else if path.extension().and_then(|value| value.to_str()) == Some("jsonl") {
            files.push(path);
        }
    }
}

fn rollout_cursor_at_end(path: &Path) -> Option<RolloutCursor> {
    let metadata = fs::metadata(path).ok()?;
    Some(RolloutCursor {
        offset: metadata.len(),
        incomplete_line: Vec::new(),
        created_at: metadata.created().ok(),
    })
}

fn appended_codex_task_completions(path: &Path, cursor: &mut RolloutCursor) -> Vec<String> {
    let Ok(metadata) = fs::metadata(path) else { return Vec::new(); };
    let replaced = cursor.created_at.zip(metadata.created().ok())
        .is_some_and(|(previous, current)| previous != current);
    if replaced || metadata.len() < cursor.offset {
        cursor.offset = metadata.len();
        cursor.incomplete_line.clear();
        cursor.created_at = metadata.created().ok();
        return Vec::new();
    }
    if cursor.created_at.is_none() { cursor.created_at = metadata.created().ok(); }
    if metadata.len() == cursor.offset { return Vec::new(); }

    let Ok(mut file) = fs::File::open(path) else { return Vec::new(); };
    if file.seek(SeekFrom::Start(cursor.offset)).is_err() { return Vec::new(); }
    let mut appended = Vec::new();
    if file.read_to_end(&mut appended).is_err() { return Vec::new(); }
    cursor.offset += appended.len() as u64;

    let mut bytes = std::mem::take(&mut cursor.incomplete_line);
    bytes.extend_from_slice(&appended);
    let complete_length = bytes.iter().rposition(|byte| *byte == b'\n').map(|index| index + 1).unwrap_or(0);
    cursor.incomplete_line = bytes.split_off(complete_length);

    bytes[..complete_length]
        .split(|byte| *byte == b'\n')
        .filter_map(|line| {
            let value = serde_json::from_slice::<serde_json::Value>(line).ok()?;
            if value.get("type").and_then(serde_json::Value::as_str) != Some("event_msg") { return None; }
            let payload = value.get("payload")?;
            if payload.get("type").and_then(serde_json::Value::as_str) != Some("task_complete") { return None; }
            payload.get("turn_id").and_then(serde_json::Value::as_str).map(str::to_owned)
        })
        .collect()
}

fn read_codex_rollout_completion(path: &Path, target_turn_id: &str) -> Option<CodexRolloutCompletion> {
    let text = fs::read_to_string(path).ok()?;
    let mut thread_id = None;
    let mut working_directory = None;
    let mut active_turn = None;
    let mut problems = HashMap::<String, String>::new();
    let mut completion = None;

    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else { continue; };
        match value.get("type").and_then(serde_json::Value::as_str) {
            Some("session_meta") => {
                let payload = value.get("payload")?;
                if payload.get("parent_thread_id").is_some() || payload.get("source").is_some_and(serde_json::Value::is_object) {
                    return None;
                }
                thread_id = payload.get("id").and_then(serde_json::Value::as_str)
                    .or_else(|| payload.get("session_id").and_then(serde_json::Value::as_str))
                    .map(str::to_owned);
                working_directory = payload.get("cwd").and_then(serde_json::Value::as_str).map(str::to_owned);
            }
            Some("event_msg") => {
                let Some(payload) = value.get("payload") else { continue; };
                match payload.get("type").and_then(serde_json::Value::as_str) {
                    Some("task_started") => {
                        active_turn = payload.get("turn_id").and_then(serde_json::Value::as_str).map(str::to_owned);
                    }
                    Some("user_message") => {
                        if let Some(turn_id) = active_turn.as_ref()
                            && let Some(problem) = payload.get("message").and_then(value_text)
                        {
                            problems.insert(turn_id.clone(), limit_notification_text(&sanitize_problem_text(&problem), 50));
                        }
                    }
                    Some("task_complete") if payload.get("turn_id").and_then(serde_json::Value::as_str) == Some(target_turn_id) => {
                        let detail = payload.get("last_agent_message").and_then(value_text)
                            .or_else(|| payload.get("error").and_then(value_text))
                            .map(|text| notification_preview(&text))
                            .unwrap_or_else(|| "任务已完成，但未收到回复摘要".to_owned());
                        let elapsed = payload.get("duration_ms").and_then(serde_json::Value::as_u64).map(Duration::from_millis);
                        completion = Some((detail, elapsed));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    let (detail, elapsed) = completion?;
    Some(CodexRolloutCompletion {
        thread_id: thread_id?,
        turn_id: target_turn_id.to_owned(),
        problem: problems.remove(target_turn_id).unwrap_or_else(|| "Codex 任务完成".to_owned()),
        detail,
        working_directory,
        elapsed,
    })
}

fn deliver_codex_rollout_completion(state: &AppState, completion: CodexRolloutCompletion) {
    let chat = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .and_then(|home| read_codex_chat_title(&home.join(".codex").join("session_index.jsonl"), &completion.thread_id));
    let smtp = {
        let Ok(stored) = state.value.lock() else { return; };
        let current = &stored.codex;
        if !current.smtp.verified || !current.integration.enabled_preference { return; }
        current.smtp.clone()
    };
    let result = send_completion_email(
        IntegrationKind::Codex,
        &smtp,
        &completion.problem,
        &completion.detail,
        completion.working_directory.as_deref(),
        chat.as_deref(),
        completion.elapsed,
    );
    let Ok(mut stored) = state.value.lock() else { return; };
    let entry = HistoryEntry {
        id: uuid::Uuid::new_v4().to_string(),
        source: source_name(IntegrationKind::Codex).into(),
        title: completion.problem,
        result: if result.is_ok() { "sent".into() } else { "failed".into() },
        detail: result.err().unwrap_or(completion.detail),
        occurred_at: Local::now().format("%Y-%m-%d %H:%M").to_string(),
    };
    stored.codex.history.push_front(entry);
    stored.codex.history.truncate(100);
    let _ = state.save(&stored);
}

fn claim_codex_completion(state: &AppState, key: &str) -> bool {
    let Ok(mut stored) = state.value.lock() else { return false; };
    if !stored.codex.smtp.verified || !stored.codex.integration.enabled_preference {
        return false;
    }
    let previous = stored.codex.integration.codex_delivered_events.clone();
    if !record_codex_delivery(&mut stored, key) {
        return false;
    }
    if state.save(&stored).is_ok() {
        true
    } else {
        stored.codex.integration.codex_delivered_events = previous;
        false
    }
}

fn record_codex_delivery(stored: &mut StoredState, key: &str) -> bool {
    let delivered = &mut stored.codex.integration.codex_delivered_events;
    if delivered.iter().any(|existing| existing == key) {
        return false;
    }
    delivered.push_back(key.to_owned());
    while delivered.len() > MAX_CODEX_DELIVERED_EVENTS {
        delivered.pop_front();
    }
    true
}

fn start_codex_rollout_listener(state: Arc<AppState>) -> Result<(), String> {
    let root = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .ok_or("找不到用户目录")?
        .join(".codex")
        .join("sessions");
    let mut files = Vec::new();
    collect_codex_rollouts(&root, &mut files);
    let mut cursors = files.into_iter().filter_map(|path| {
        rollout_cursor_at_end(&path).map(|cursor| (path, cursor))
    }).collect::<HashMap<_, _>>();

    thread::spawn(move || {
        let mut delivered = HashSet::<String>::new();
        loop {
            let mut files = Vec::new();
            collect_codex_rollouts(&root, &mut files);
            for path in files {
                if !cursors.contains_key(&path) {
                    if let Some(cursor) = rollout_cursor_at_end(&path) {
                        cursors.insert(path, cursor);
                    }
                    continue;
                }
                let Some(cursor) = cursors.get_mut(&path) else { continue; };
                for turn_id in appended_codex_task_completions(&path, cursor) {
                    let Some(completion) = read_codex_rollout_completion(&path, &turn_id) else { continue; };
                    let key = format!("{}:{}", completion.thread_id, completion.turn_id);
                    if delivered.insert(key.clone()) && claim_codex_completion(&state, &key) {
                        deliver_codex_rollout_completion(&state, completion);
                    }
                }
            }
            thread::sleep(Duration::from_secs(1));
        }
    });
    Ok(())
}

fn collect_claude_transcripts(root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else { return; };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|value| value.to_str()) != Some("subagents") {
                collect_claude_transcripts(&path, files);
            }
        } else if path.extension().and_then(|value| value.to_str()) == Some("jsonl") {
            files.push(path);
        }
    }
}

fn claude_text_completion_id(value: &serde_json::Value) -> Option<String> {
    let stop_reason = value
        .pointer("/message/stop_reason")
        .or_else(|| value.pointer("/message/stopReason"))
        .and_then(serde_json::Value::as_str);
    if value.get("type").and_then(serde_json::Value::as_str) != Some("assistant")
        || value.get("isSidechain").and_then(serde_json::Value::as_bool) == Some(true)
        || value.pointer("/message/role").and_then(serde_json::Value::as_str) != Some("assistant")
        || !matches!(stop_reason, Some("end_turn") | Some("stop_sequence"))
        || value.pointer("/message/content").and_then(value_text).is_none()
    {
        return None;
    }
    value.get("uuid").and_then(serde_json::Value::as_str).map(str::to_owned)
}

fn appended_claude_task_completions(path: &Path, cursor: &mut RolloutCursor) -> Vec<String> {
    let Ok(metadata) = fs::metadata(path) else { return Vec::new(); };
    if metadata.len() < cursor.offset {
        cursor.offset = 0;
        cursor.incomplete_line.clear();
    }
    if metadata.len() == cursor.offset { return Vec::new(); }

    let Ok(mut file) = fs::File::open(path) else { return Vec::new(); };
    if file.seek(SeekFrom::Start(cursor.offset)).is_err() { return Vec::new(); }
    let mut appended = Vec::new();
    if file.read_to_end(&mut appended).is_err() { return Vec::new(); }
    cursor.offset += appended.len() as u64;

    let mut bytes = std::mem::take(&mut cursor.incomplete_line);
    bytes.extend_from_slice(&appended);
    let complete_length = bytes.iter().rposition(|byte| *byte == b'\n').map(|index| index + 1).unwrap_or(0);
    cursor.incomplete_line = bytes.split_off(complete_length);

    let mut completion_ids = bytes[..complete_length]
        .split(|byte| *byte == b'\n')
        .filter_map(|line| serde_json::from_slice::<serde_json::Value>(line).ok())
        .filter_map(|value| claude_text_completion_id(&value))
        .collect::<Vec<_>>();
    if !cursor.incomplete_line.is_empty()
        && let Ok(value) = serde_json::from_slice::<serde_json::Value>(&cursor.incomplete_line)
        && let Some(completion_id) = claude_text_completion_id(&value)
    {
        completion_ids.push(completion_id);
        cursor.incomplete_line.clear();
    }
    completion_ids
}

fn select_claude_completion_ids(ids: Vec<String>, new_file: bool) -> Vec<String> {
    if new_file {
        ids.into_iter().last().into_iter().collect()
    } else {
        ids
    }
}

fn read_claude_transcript_completion(path: &Path, target_completion_id: &str) -> Option<ClaudeTranscriptCompletion> {
    let text = fs::read_to_string(path).ok()?;
    let mut session_id = path.file_stem().and_then(|value| value.to_str()).map(str::to_owned);
    let mut custom_title = None;
    let mut ai_title = None;
    let records = text.lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .collect::<Vec<_>>();
    let mut indexes = HashMap::<String, usize>::new();
    for (index, value) in records.iter().enumerate() {
        if let Some(id) = value.get("sessionId").and_then(serde_json::Value::as_str) {
            session_id = Some(id.to_owned());
        }
        if let Some(id) = value.get("uuid").and_then(serde_json::Value::as_str) {
            indexes.insert(id.to_owned(), index);
        }
        match value.get("type").and_then(serde_json::Value::as_str) {
            Some("custom-title") => custom_title = value.get("customTitle").and_then(value_text)
                .or_else(|| value.get("custom_title").and_then(value_text)).or(custom_title),
            Some("ai-title") => ai_title = value.get("aiTitle").and_then(value_text).or(ai_title),
            _ => {}
        }
    }

    let target_index = records.iter().position(|value| {
        value.get("uuid").and_then(serde_json::Value::as_str) == Some(target_completion_id)
    })?;
    let target = records.get(target_index)?;
    claude_text_completion_id(target)?;

    let is_human_prompt = |value: &serde_json::Value| {
        value.get("type").and_then(serde_json::Value::as_str) == Some("user")
            && value.get("isSidechain").and_then(serde_json::Value::as_bool) != Some(true)
            && value.get("isMeta").and_then(serde_json::Value::as_bool) != Some(true)
            && (value.pointer("/origin/kind").and_then(serde_json::Value::as_str) == Some("human")
                || value.get("promptSource").and_then(serde_json::Value::as_str).is_some())
            && value.pointer("/message/content").is_some_and(serde_json::Value::is_string)
    };
    if records.iter().enumerate().rev().find_map(|(index, value)| {
        is_human_prompt(value).then_some(index)
    }).is_some_and(|latest_prompt_index| latest_prompt_index > target_index) {
        return None;
    }
    let mut parent_uuid = target.get("parentUuid").and_then(serde_json::Value::as_str).map(str::to_owned);
    let mut prompt_index = None;
    for _ in 0..64 {
        let Some(uuid) = parent_uuid.as_deref() else { break; };
        let Some(index) = indexes.get(uuid).copied() else { break; };
        let parent = records.get(index)?;
        if is_human_prompt(parent) {
            prompt_index = Some(index);
            break;
        }
        parent_uuid = parent.get("parentUuid").and_then(serde_json::Value::as_str).map(str::to_owned);
    }
    let prompt_index = prompt_index.or_else(|| {
        records[..target_index].iter().enumerate().rev()
            .find_map(|(index, value)| is_human_prompt(value).then_some(index))
    });
    let prompt = prompt_index.and_then(|index| records.get(index));
    let session_id = session_id?;
    let problem = prompt.and_then(|value| value.pointer("/message/content").and_then(value_text))
        .map(|text| limit_notification_text(&sanitize_problem_text(&text), 50))
        .unwrap_or_else(|| "Claude Code 任务完成".to_owned());
    let detail = target.pointer("/message/content").and_then(value_text)
        .map(|text| notification_preview(&text))?;
    let started_at = prompt.and_then(|value| value.get("timestamp").and_then(serde_json::Value::as_str));
    let ended_at = target.get("timestamp").and_then(serde_json::Value::as_str);
    let elapsed = started_at.zip(ended_at).and_then(|(start, end)| {
        let start = DateTime::parse_from_rfc3339(start).ok()?;
        let end = DateTime::parse_from_rfc3339(end).ok()?;
        end.signed_duration_since(start).to_std().ok()
    });
    Some(ClaudeTranscriptCompletion {
        completion_id: target_completion_id.to_owned(),
        chat: custom_title.or(ai_title).or_else(|| desktop_session_title(&session_id)),
        problem,
        detail,
        working_directory: prompt.and_then(|value| value.get("cwd").and_then(serde_json::Value::as_str).map(str::to_owned))
            .or_else(|| target.get("cwd").and_then(serde_json::Value::as_str).map(str::to_owned)),
        elapsed,
    })
}

// The record uuid survives a chat being forked into a new session file, so it
// identifies a completion where the session id no longer does.
fn claude_delivery_key(completion: &ClaudeTranscriptCompletion) -> String {
    completion.completion_id.clone()
}

fn deliver_claude_transcript_completion(state: &AppState, completion: ClaudeTranscriptCompletion) {
    let smtp = {
        let Ok(stored) = state.value.lock() else { return; };
        let current = &stored.claude;
        if !current.smtp.verified || !current.integration.enabled_preference { return; }
        current.smtp.clone()
    };
    let result = send_completion_email(
        IntegrationKind::Claude,
        &smtp,
        &completion.problem,
        &completion.detail,
        completion.working_directory.as_deref(),
        completion.chat.as_deref(),
        completion.elapsed,
    );
    let Ok(mut stored) = state.value.lock() else { return; };
    let entry = HistoryEntry {
        id: uuid::Uuid::new_v4().to_string(),
        source: source_name(IntegrationKind::Claude).into(),
        title: completion.problem,
        result: if result.is_ok() { "sent".into() } else { "failed".into() },
        detail: result.err().unwrap_or(completion.detail),
        occurred_at: Local::now().format("%Y-%m-%d %H:%M").to_string(),
    };
    stored.claude.history.push_front(entry);
    stored.claude.history.truncate(100);
    let _ = state.save(&stored);
}

// Claude Code keeps CLI transcripts under the user profile, while the desktop
// client stores its local agent sessions beside the application data.
fn claude_transcript_roots_from(user_profile: Option<&Path>, local_app_data: Option<&Path>) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(profile) = user_profile {
        roots.push(profile.join(".claude").join("projects"));
    }
    if let Some(local) = local_app_data {
        roots.push(local.join("Claude-3p").join("local-agent-mode-sessions"));
    }
    roots
}

// Desktop chat titles live in per-session metadata files rather than in the
// transcript. Those files also hold the signed-in account address, so only
// cliSessionId and title are ever read out of them.
fn desktop_session_title_in(root: &Path, session_id: &str) -> Option<String> {
    fn walk(directory: &Path, session_id: &str, depth: usize) -> Option<String> {
        if depth > 4 { return None; }
        let entries = fs::read_dir(directory).ok()?;
        let mut directories = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                directories.push(path);
                continue;
            }
            let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
            if !name.starts_with("local_") || !name.ends_with(".json") { continue; }
            let Ok(text) = fs::read_to_string(&path) else { continue; };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else { continue; };
            if value.get("cliSessionId").and_then(serde_json::Value::as_str) != Some(session_id) { continue; }
            if let Some(title) = value.get("title").and_then(serde_json::Value::as_str) {
                let title = title.trim();
                if !title.is_empty() { return Some(title.to_owned()); }
            }
        }
        directories.into_iter().find_map(|path| walk(&path, session_id, depth + 1))
    }
    walk(root, session_id, 0)
}

fn desktop_session_title(session_id: &str) -> Option<String> {
    let root = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)?.join("Claude-3p");
    desktop_session_title_in(&root, session_id)
}

fn claude_transcript_roots() -> Vec<PathBuf> {
    claude_transcript_roots_from(
        std::env::var_os("USERPROFILE").map(PathBuf::from).as_deref(),
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from).as_deref(),
    )
}

fn start_claude_transcript_listener(state: Arc<AppState>) -> Result<(), String> {
    let roots = claude_transcript_roots();
    if roots.is_empty() { return Err("找不到用户目录".to_owned()); }
    let mut files = Vec::new();
    for root in &roots {
        collect_claude_transcripts(root, &mut files);
    }
    let mut cursors = files.into_iter().filter_map(|path| {
        fs::metadata(&path).ok().map(|metadata| (path, RolloutCursor { offset: metadata.len(), incomplete_line: Vec::new(), created_at: metadata.created().ok() }))
    }).collect::<HashMap<_, _>>();

    thread::spawn(move || {
        let mut delivered = HashSet::<String>::new();
        loop {
            let mut files = Vec::new();
            for root in &roots {
                collect_claude_transcripts(root, &mut files);
            }
            for path in files {
                let new_file = !cursors.contains_key(&path);
                let cursor = cursors.entry(path.clone()).or_default();
                let completion_ids = appended_claude_task_completions(&path, cursor);
                for completion_id in select_claude_completion_ids(completion_ids, new_file) {
                    let Some(completion) = read_claude_transcript_completion(&path, &completion_id) else { continue; };
                    let key = claude_delivery_key(&completion);
                    if delivered.insert(key) {
                        deliver_claude_transcript_completion(&state, completion);
                    }
                }
            }
            thread::sleep(Duration::from_secs(1));
        }
    });
    Ok(())
}

fn remove_exact_file(path: &std::path::Path) -> Result<(), String> {
    if !path.exists() { return Ok(()); }
    fs::remove_file(path).map_err(|error| error.to_string())
}

fn remove_settings_files(root: &Path) -> Result<(), String> {
    remove_exact_file(&root.join("runtime.json"))?;
    remove_exact_file(&root.join("settings.json.tmp"))?;
    remove_exact_file(&root.join("settings.json"))
}

fn remove_channel_credentials_checked(state: &StoredState) -> Result<(), String> {
    remove_channel_credentials_checked_with(state, |service, email| {
        let entry = Entry::new(service, email).map_err(|_| "无法访问授权码存储，请稍后重试".to_owned())?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err("无法删除保存的授权码，请手动检查后重试".to_owned()),
        }
    })
}

fn remove_channel_credentials_checked_with(
    state: &StoredState,
    mut remove: impl FnMut(&str, &str) -> Result<(), String>,
) -> Result<(), String> {
    let mut failures = false;
    let mut visited = HashSet::new();
    for (service, email) in [
        (credential_service(IntegrationKind::Codex), state.codex.smtp.email.as_str()),
        (credential_service(IntegrationKind::Claude), state.claude.smtp.email.as_str()),
        (KEYRING_SERVICE.to_owned(), state.codex.smtp.email.as_str()),
        (KEYRING_SERVICE.to_owned(), state.claude.smtp.email.as_str()),
    ] {
        if email.is_empty() || !visited.insert(format!("{service}\n{email}")) { continue; }
        if remove(&service, email).is_err() {
            failures = true;
        }
    }
    if failures { Err("无法删除保存的授权码，请手动检查后重试".to_owned()) } else { Ok(()) }
}

fn cleanup_application(purge: bool) -> Result<(), String> {
    stop_running_instances();
    let root = app_data_root();
    let settings_path = root.join("settings.json");
    let (mut state, _) = fs::read_to_string(&settings_path)
        .ok()
        .and_then(|text| migrate_legacy_state(&text).ok())
        .unwrap_or_else(|| (StoredState::current(), false));
    let executable = std::env::current_exe().map_err(|error| error.to_string())?.to_string_lossy().to_string();
    restore_owned_hooks(&state, &executable)?;
    if purge {
        remove_channel_credentials_checked(&state)?;
        remove_settings_files(&root)?;
    } else {
        remove_exact_file(&root.join("runtime.json"))?;
        mark_disconnected_for_uninstall(&mut state);
        let parent = settings_path.parent().ok_or("无效的设置路径")?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        fs::write(settings_path.with_extension("json.tmp"), serde_json::to_vec_pretty(&state).map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
        fs::rename(settings_path.with_extension("json.tmp"), settings_path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn write_with_backup(path: &std::path::Path, content: String) -> Result<(), String> {
    let file_name = path.file_name().and_then(|name| name.to_str()).ok_or("无效的配置文件名")?;
    let backup = path.with_file_name(format!("{file_name}.{}.bak", Local::now().format("%Y%m%d-%H%M%S")));
    fs::copy(path, backup).map_err(|error| error.to_string())?;
    fs::write(path, content).map_err(|error| error.to_string())
}

fn codex_timing_command(executable: &str) -> String {
    format!("\"{executable}\" --hook codex")
}

#[tauri::command]
fn set_integration_enabled(kind: IntegrationKind, enabled: bool, state: State<'_, Arc<AppState>>) -> Result<Dashboard, String> {
    let mut stored = state.value.lock().map_err(|_| "设置锁不可用")?;
    let current = channel_mut(&mut stored, kind);
    if !current.smtp.verified { return Err("请先完成该通道的 SMTP 测试".into()); }
    current.integration.enabled_preference = enabled;
    state.save(&stored)?;
    state.sync_tray(&stored);
    Ok(dashboard(&stored))
}

#[tauri::command]
fn clear_history(kind: IntegrationKind, state: State<'_, Arc<AppState>>) -> Result<Dashboard, String> {
    let mut stored = state.value.lock().map_err(|_| "设置锁不可用")?;
    channel_mut(&mut stored, kind).history.clear();
    state.save(&stored)?;
    Ok(dashboard(&stored))
}

#[tauri::command]
fn open_log_folder(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let folder = state.path.parent().ok_or("无效的日志目录")?;
    fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    std::process::Command::new("explorer").arg(folder).spawn().map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn exit_application(app: AppHandle) { app.exit(0); }

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn remove_runtime_record(state: &AppState) {
    let _ = fs::remove_file(state.runtime_path());
}

fn setup_tray(app: &tauri::App, state: Arc<AppState>) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "打开主页", true, None::<&str>)?;
    let codex = CheckMenuItem::with_id(app, "codex-toggle", "Codex 邮件通知", true, false, None::<&str>)?;
    let claude = CheckMenuItem::with_id(app, "claude-toggle", "Claude Code 邮件通知", true, false, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出程序", true, None::<&str>)?;
    if let Ok(mut tray) = state.tray.lock() {
        *tray = Some(TrayControls { codex: codex.clone(), claude: claude.clone() });
    }
    if let Ok(stored) = state.value.lock() {
        state.sync_tray(&stored);
    }
    let menu = Menu::with_items(app, &[&open, &codex, &claude, &quit])?;
    let menu_state = state.clone();
    TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("Agent 邮件通知")
        .menu(&menu)
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. }
            | TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } => show_main_window(tray.app_handle()),
            _ => {}
        })
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "open" => show_main_window(app),
            "quit" => app.exit(0),
            "codex-toggle" | "claude-toggle" => {
                if let Ok(mut stored) = menu_state.value.lock() {
                    let kind = if event.id.as_ref() == "codex-toggle" { IntegrationKind::Codex } else { IntegrationKind::Claude };
                    let current = channel_mut(&mut stored, kind);
                    if current.smtp.verified {
                        current.integration.enabled_preference = !current.integration.enabled_preference;
                        if menu_state.save(&stored).is_ok() {
                            menu_state.sync_tray(&stored);
                            let _ = app.emit("dashboard-changed", ());
                        }
                    }
                }
            }
            _ => {}
        })
        .build(app)?;
    Ok(())
}

pub fn run() {
    if run_hook_mode() { return; }
    let state = Arc::new(AppState::load());
    let setup_state = state.clone();
    let cleanup_state = state.clone();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .manage(state.clone())
        .setup(move |app| {
            start_codex_rollout_listener(setup_state.clone())
                .map_err(std::io::Error::other)?;
            start_claude_transcript_listener(setup_state.clone())
                .map_err(std::io::Error::other)?;
            setup_tray(app, setup_state.clone())?;
            if let Some(window) = app.get_webview_window("main") {
                let window_for_close = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_for_close.hide();
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_dashboard, save_and_test_smtp, set_integration_enabled, clear_history, open_log_folder, exit_application])
        .build(tauri::generate_context!())
        .expect("运行 Agent Mail Notifier 时发生错误");
    app.run(move |_app, event| {
        if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
            remove_runtime_record(&cleanup_state);
        }
    });
}

#[cfg(test)]
mod tests {
    use std::{cell::{Cell, RefCell}, fs, io::Write, path::Path};
    use std::time::Duration;

    use super::{
        CredentialStore, LegacyCredentialMigration, migrate_legacy_credentials_with_store,
        persist_verified_smtp, remove_channel_credentials_checked_with, remove_settings_files,
        appended_claude_task_completions, appended_codex_task_completions, collect_claude_transcripts,
        credential_service,
        read_claude_transcript_completion, read_codex_rollout_completion, record_codex_delivery,
        rollout_cursor_at_end, smtp_error, RolloutCursor,
        limit_notification_text, sanitize_problem_text, select_claude_completion_ids,
    };
    use crate::state::{StoredSmtp, StoredState};
    use agent_mail_notifier_core::IntegrationKind;

    struct FakeCredentialStore {
        passwords: RefCell<[Option<String>; 2]>,
        fail_write: Cell<Option<IntegrationKind>>,
    }

    impl CredentialStore for FakeCredentialStore {
        fn read_password(&self, kind: IntegrationKind, _email: &str) -> Result<Option<String>, String> {
            Ok(self.passwords.borrow()[kind_index(kind)].clone())
        }

        fn write_password(&self, kind: IntegrationKind, _email: &str, password: &str) -> Result<(), String> {
            if self.fail_write.get() == Some(kind) {
                return Err("凭据写入失败".to_owned());
            }
            self.passwords.borrow_mut()[kind_index(kind)] = Some(password.to_owned());
            Ok(())
        }

        fn delete_password(&self, kind: IntegrationKind, _email: &str) -> Result<(), String> {
            self.passwords.borrow_mut()[kind_index(kind)] = None;
            Ok(())
        }
    }

    fn kind_index(kind: IntegrationKind) -> usize {
        match kind {
            IntegrationKind::Codex => 0,
            IntegrationKind::Claude => 1,
        }
    }

    #[test]
    fn failed_smtp_state_save_restores_the_previous_credential_and_settings() {
        let mut state = StoredState::current();
        state.codex.smtp.email = "account".to_owned();
        state.codex.smtp.provider_label = "旧配置".to_owned();
        let credential_store = FakeCredentialStore {
            passwords: RefCell::new([Some("previous".to_owned()), None]),
            fail_write: Cell::new(None),
        };
        let next_smtp = StoredSmtp { email: "account".to_owned(), provider_label: "新配置".to_owned(), verified: true, ..StoredSmtp::default() };

        let result = persist_verified_smtp(
            &mut state,
            IntegrationKind::Codex,
            next_smtp,
            "replacement",
            &credential_store,
            |_| Err("设置保存失败".to_owned()),
        );

        assert_eq!(result, Err("设置保存失败".to_owned()));
        assert_eq!(state.codex.smtp.provider_label, "旧配置");
        assert_eq!(credential_store.passwords.borrow()[0].as_deref(), Some("previous"));
    }

    #[test]
    fn legacy_migration_reports_a_channel_credential_write_failure() {
        let mut state = StoredState::current();
        state.codex.smtp.email = "account".to_owned();
        state.claude.smtp.email = "account".to_owned();
        let credential_store = FakeCredentialStore {
            passwords: RefCell::new([None, None]),
            fail_write: Cell::new(Some(IntegrationKind::Claude)),
        };

        let result = migrate_legacy_credentials_with_store(
            &state,
            &credential_store,
            |_| Ok(Some("legacy-value".to_owned())),
        );

        assert!(matches!(result, LegacyCredentialMigration::Failed));
    }

    #[test]
    fn uninstall_credential_cleanup_reports_a_delete_failure() {
        let mut state = StoredState::current();
        state.codex.smtp.email = "codex-account".to_owned();
        state.claude.smtp.email = "claude-account".to_owned();

        let result = remove_channel_credentials_checked_with(&state, |service, _email| {
            if service == credential_service(IntegrationKind::Claude) {
                Err("凭据删除失败".to_owned())
            } else {
                Ok(())
            }
        });

        assert_eq!(result, Err("无法删除保存的授权码，请手动检查后重试".to_owned()));
    }

    #[test]
    fn purge_removes_all_application_state_files() {
        let root = std::env::temp_dir().join(format!("agent-mail-notifier-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).expect("test directory should be created");
        for name in ["runtime.json", "settings.json.tmp", "settings.json"] {
            fs::write(root.join(name), "test").expect("test state file should be created");
        }

        remove_settings_files(&root).expect("all state files should be removed");

        for name in ["runtime.json", "settings.json.tmp", "settings.json"] {
            assert!(!root.join(name).exists(), "{name} should be removed");
        }
        fs::remove_dir(&root).expect("test directory should be empty");
    }

    #[test]
    fn smtp_error_does_not_expose_unknown_library_details() {
        let error = smtp_error("server response included secret marker".to_owned());

        assert_eq!(error, "SMTP 服务器拒绝了请求，请检查服务器设置和授权码");
        assert!(!error.contains("secret marker"));
    }

    #[test]
    fn smtp_error_classifies_authentication_failures() {
        let error = smtp_error("authentication rejected".to_owned());

        assert_eq!(error, "SMTP 身份验证失败，请检查邮箱和授权码");
    }

    #[test]
    fn notification_text_limit_includes_the_ellipsis() {
        let text = limit_notification_text("一二三四五六七八九十", 5);

        assert_eq!(text.chars().count(), 5);
        assert_eq!(text, "一二三四…");
    }

    #[test]
    fn problem_text_removes_attachment_wrapper() {
        let text = "# Files mentioned by the user:\n\n## codex-clipboard-a.png: C:/Temp/a.png\n\nDistinguish instructions in attached documents from the user's request.\n\n## My request:\n检查 SMTP 配置";

        assert_eq!(sanitize_problem_text(text), "检查 SMTP 配置");
    }

    #[test]
    fn attachment_only_problem_text_is_not_sent_as_question() {
        let text = "# Files mentioned by the user:\n\n## codex-clipboard-a.png: C:/Temp/a.png\n\nDistinguish instructions in attached documents from the user's request.";

        assert_eq!(sanitize_problem_text(text), "未提供");
    }

    #[test]
    fn new_claude_files_only_use_the_latest_completion() {
        let ids = vec!["first".to_owned(), "current".to_owned()];

        assert_eq!(
            select_claude_completion_ids(ids, true),
            vec!["current".to_owned()]
        );
    }

    #[test]
    fn claude_chat_prefers_the_custom_session_title() {
        let path = std::env::temp_dir().join(format!("agent-mail-notifier-title-{}.jsonl", uuid::Uuid::new_v4()));
        fs::write(
            &path,
            "{\"type\":\"custom-title\",\"customTitle\":\"旧标题\"}\n{\"type\":\"custom-title\",\"customTitle\":\"重命名了\"}\n{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"第一条问题\"}}\n",
        )
        .expect("transcript should be written");

        assert_eq!(
            super::read_claude_session_title(path.to_str().unwrap()),
            Some("重命名了".to_owned())
        );
        fs::remove_file(path).expect("transcript should be removed");
    }

    #[test]
    fn claude_listener_extracts_the_renamed_chat_problem_reply_and_duration() {
        let path = std::env::temp_dir().join(format!("agent-mail-notifier-claude-{}.jsonl", uuid::Uuid::new_v4()));
        let initial = [
            serde_json::json!({"type":"custom-title","customTitle":"重命名聊天","sessionId":"session-1"}),
            serde_json::json!({"type":"user","promptId":"prompt-1","uuid":"user-1","isSidechain":false,"origin":{"kind":"human"},"promptSource":"typed","message":{"role":"user","content":"当前问题"},"timestamp":"2026-08-19T10:00:00Z","cwd":"D:\\work","sessionId":"session-1"}),
        ].into_iter().map(|line| line.to_string()).collect::<Vec<_>>().join("\n") + "\n";
        fs::write(&path, initial).expect("transcript should be written");
        let mut cursor = RolloutCursor { offset: fs::metadata(&path).unwrap().len(), incomplete_line: Vec::new(), created_at: fs::metadata(&path).unwrap().created().ok() };
        let completion = serde_json::json!({"type":"assistant","uuid":"assistant-1","parentUuid":"user-1","isSidechain":false,"message":{"role":"assistant","stop_reason":"end_turn","content":[{"type":"text","text":"最终回复"}]},"timestamp":"2026-08-19T10:00:05Z","sessionId":"session-1"}).to_string();
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(completion.as_bytes()).unwrap();
        assert_eq!(appended_claude_task_completions(&path, &mut cursor), vec!["assistant-1"]);
        file.write_all(b"\n").unwrap();
        assert!(appended_claude_task_completions(&path, &mut cursor).is_empty());
        let parsed = read_claude_transcript_completion(&path, "assistant-1").expect("completion should be parsed");
        let next_prompt = serde_json::json!({"type":"user","promptId":"prompt-2","uuid":"user-2","isSidechain":false,"origin":{"kind":"human"},"promptSource":"typed","message":{"role":"user","content":"下一条问题"}}).to_string() + "\n";
        file.write_all(next_prompt.as_bytes()).unwrap();
        assert!(read_claude_transcript_completion(&path, "assistant-1").is_none());

        assert_eq!(parsed.chat.as_deref(), Some("重命名聊天"));
        assert_eq!(parsed.problem, "当前问题");
        assert_eq!(parsed.detail, "最终回复");
        assert_eq!(parsed.working_directory.as_deref(), Some("D:\\work"));
        assert_eq!(parsed.elapsed, Some(Duration::from_secs(5)));
        fs::remove_file(path).expect("transcript should be removed");
    }

    #[test]
    fn a_notification_body_names_cowork_instead_of_its_scratch_directory() {
        let sandbox = "C:\\Users\\demo\\AppData\\Local\\Claude-3p\\local-agent-mode-sessions\\aaaa\\0000\\bbbb\\outputs";

        let without = super::completion_email_body("Claude Code", "问题", "回复", None, None, None);
        let sandboxed = super::completion_email_body("Claude Code", "问题", "回复", Some(sandbox), None, None);
        let with_project = super::completion_email_body("Claude Code", "问题", "回复", Some("D:\\work\\MyProject"), None, None);

        assert!(!without.contains("项目："), "nothing to name means no project line at all");
        assert!(sandboxed.contains("项目：Cowork"));
        assert!(!sandboxed.contains("outputs"), "the scratch directory name is meaningless to the reader");
        assert!(with_project.contains("项目：MyProject"));
    }

    #[test]
    fn a_notification_body_omits_the_chat_line_when_no_title_was_found() {
        let untitled = super::completion_email_body("Claude Code", "问题", "回复", None, None, None);
        let titled = super::completion_email_body("Claude Code", "问题", "回复", None, Some("Blender MCP 安装"), None);

        assert!(!untitled.contains("聊天："), "an unresolved title must not fall back to a session id");
        assert!(untitled.contains("问题：问题"));
        assert!(titled.contains("聊天：Blender MCP 安装"));
    }

    #[test]
    fn a_desktop_session_title_is_read_from_the_matching_metadata_file() {
        let root = std::env::temp_dir().join(format!("agent-mail-notifier-title-{}", uuid::Uuid::new_v4()));
        let store = root.join("local-agent-mode-sessions").join("aaaa").join("0000");
        fs::create_dir_all(&store).expect("session store should be created");
        // Real metadata files also carry an account address; only cliSessionId
        // and title may ever be read out of them.
        fs::write(
            store.join("local_bbbb.json"),
            serde_json::json!({"cliSessionId":"session-1","title":"Blender MCP 安装","emailAddress":"must-not-be-read"}).to_string(),
        ).expect("metadata should be written");
        fs::write(
            store.join("local_cccc.json"),
            serde_json::json!({"cliSessionId":"session-2","title":"另一个会话"}).to_string(),
        ).expect("metadata should be written");

        assert_eq!(super::desktop_session_title_in(&root, "session-1"), Some("Blender MCP 安装".to_owned()));
        assert_eq!(super::desktop_session_title_in(&root, "session-2"), Some("另一个会话".to_owned()));
        assert_eq!(super::desktop_session_title_in(&root, "session-3"), None);
        fs::remove_dir_all(root).expect("temporary tree should be removed");
    }

    #[test]
    fn a_completion_copied_into_a_new_session_file_is_delivered_once() {
        // The desktop client forks a chat by copying the whole transcript into a
        // new file under a fresh sessionId, keeping every record uuid. Keying
        // delivery on the session would treat the copy as a second completion.
        let first = std::env::temp_dir().join(format!("agent-mail-notifier-fork-a-{}.jsonl", uuid::Uuid::new_v4()));
        let second = std::env::temp_dir().join(format!("agent-mail-notifier-fork-b-{}.jsonl", uuid::Uuid::new_v4()));
        let records = |session: &str| {
            [
                serde_json::json!({"type":"user","promptId":"prompt-1","uuid":"user-1","isSidechain":false,"origin":{"kind":"human"},"promptSource":"sdk","message":{"role":"user","content":"同一个问题"},"sessionId":session}),
                serde_json::json!({"type":"assistant","uuid":"assistant-1","parentUuid":"user-1","isSidechain":false,"message":{"role":"assistant","stop_reason":"end_turn","content":[{"type":"text","text":"同一个回复"}]},"sessionId":session}),
            ].into_iter().map(|line| line.to_string()).collect::<Vec<_>>().join("\n") + "\n"
        };
        fs::write(&first, records("session-old")).expect("transcript should be written");
        fs::write(&second, records("session-forked")).expect("forked transcript should be written");

        let from_first = read_claude_transcript_completion(&first, "assistant-1").expect("original should parse");
        let from_second = read_claude_transcript_completion(&second, "assistant-1").expect("fork should parse");

        assert!(
            fs::read_to_string(&first).unwrap().contains("session-old")
                && fs::read_to_string(&second).unwrap().contains("session-forked"),
            "the two files must carry different session ids for this to prove anything",
        );
        assert_eq!(
            super::claude_delivery_key(&from_first),
            super::claude_delivery_key(&from_second),
            "the same completion must produce one delivery key across both files",
        );
        fs::remove_file(first).expect("transcript should be removed");
        fs::remove_file(second).expect("forked transcript should be removed");
    }

    #[test]
    fn claude_listener_accepts_a_reply_closed_by_a_stop_sequence() {
        let path = std::env::temp_dir().join(format!("agent-mail-notifier-stopseq-{}.jsonl", uuid::Uuid::new_v4()));
        let initial = serde_json::json!({"type":"user","promptId":"prompt-1","uuid":"user-1","isSidechain":false,"origin":{"kind":"human"},"promptSource":"sdk","message":{"role":"user","content":"当前问题"},"timestamp":"2026-08-19T10:00:00Z","sessionId":"session-1"}).to_string() + "\n";
        fs::write(&path, initial).expect("transcript should be written");
        let mut cursor = RolloutCursor { offset: fs::metadata(&path).unwrap().len(), incomplete_line: Vec::new(), created_at: fs::metadata(&path).unwrap().created().ok() };

        let completion = serde_json::json!({"type":"assistant","uuid":"assistant-1","parentUuid":"user-1","isSidechain":false,"message":{"role":"assistant","stop_reason":"stop_sequence","content":[{"type":"text","text":"被停止串收尾的回复"}]},"timestamp":"2026-08-19T10:00:05Z","sessionId":"session-1"}).to_string() + "\n";
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(completion.as_bytes()).unwrap();

        assert_eq!(appended_claude_task_completions(&path, &mut cursor), vec!["assistant-1"]);
        let parsed = read_claude_transcript_completion(&path, "assistant-1").expect("stop_sequence reply should be treated as a finished turn");
        assert_eq!(parsed.problem, "当前问题");
        assert_eq!(parsed.detail, "被停止串收尾的回复");
        fs::remove_file(path).expect("transcript should be removed");
    }

    #[test]
    fn claude_listener_ignores_a_reply_truncated_by_the_token_limit() {
        let path = std::env::temp_dir().join(format!("agent-mail-notifier-maxtok-{}.jsonl", uuid::Uuid::new_v4()));
        let initial = serde_json::json!({"type":"user","promptId":"prompt-1","uuid":"user-1","isSidechain":false,"origin":{"kind":"human"},"promptSource":"sdk","message":{"role":"user","content":"当前问题"},"sessionId":"session-1"}).to_string() + "\n";
        fs::write(&path, initial).expect("transcript should be written");
        let mut cursor = RolloutCursor { offset: fs::metadata(&path).unwrap().len(), incomplete_line: Vec::new(), created_at: fs::metadata(&path).unwrap().created().ok() };

        let truncated = serde_json::json!({"type":"assistant","uuid":"assistant-1","parentUuid":"user-1","isSidechain":false,"message":{"role":"assistant","stop_reason":"max_tokens","content":[{"type":"text","text":"半截回复"}]},"sessionId":"session-1"}).to_string() + "\n";
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(truncated.as_bytes()).unwrap();

        assert!(appended_claude_task_completions(&path, &mut cursor).is_empty());
        fs::remove_file(path).expect("transcript should be removed");
    }

    #[test]
    fn claude_transcript_roots_cover_the_cli_and_desktop_session_stores() {
        let roots = super::claude_transcript_roots_from(Some(Path::new("C:\\Users\\demo")), Some(Path::new("C:\\Users\\demo\\AppData\\Local")));
        assert_eq!(roots.len(), 2);
        assert!(roots[0].ends_with("projects"));
        assert!(roots[1].ends_with("local-agent-mode-sessions"));

        assert!(super::claude_transcript_roots_from(None, None).is_empty());
    }

    #[test]
    fn collect_claude_transcripts_walks_hidden_session_directories() {
        let root = std::env::temp_dir().join(format!("agent-mail-notifier-walk-{}", uuid::Uuid::new_v4()));
        let nested = root.join("workspace").join(".claude").join("projects").join("session");
        fs::create_dir_all(&nested).expect("nested session directory should be created");
        fs::write(nested.join("chat.jsonl"), "{}\n").expect("transcript should be written");
        fs::create_dir_all(root.join("subagents")).expect("subagent directory should be created");
        fs::write(root.join("subagents").join("ignored.jsonl"), "{}\n").expect("subagent transcript should be written");

        let mut files = Vec::new();
        collect_claude_transcripts(&root, &mut files);

        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("chat.jsonl"));
        fs::remove_dir_all(root).expect("temporary tree should be removed");
    }

    #[test]
    fn codex_chat_uses_the_latest_renamed_session_index_entry() {
        let path = std::env::temp_dir().join(format!("agent-mail-notifier-codex-index-{}.jsonl", uuid::Uuid::new_v4()));
        fs::write(
            &path,
            "{\"id\":\"thread-1\",\"thread_name\":\"整理10条常用Git命令\"}\n{\"id\":\"thread-1\",\"thread_name\":\"测试用\"}\n",
        )
        .expect("session index should be written");

        assert_eq!(
            super::read_codex_chat_title(&path, "thread-1"),
            Some("测试用".to_owned())
        );
        fs::remove_file(path).expect("session index should be removed");
    }

    #[test]
    fn rollout_cursor_reports_only_new_complete_task_lines() {
        let path = std::env::temp_dir().join(format!("agent-mail-notifier-rollout-{}.jsonl", uuid::Uuid::new_v4()));
        fs::write(&path, "{\"type\":\"session_meta\"}\n").expect("rollout should be written");
        let mut cursor = rollout_cursor_at_end(&path).expect("rollout cursor should initialize at the end");
        let line = "{\"type\":\"event_msg\",\"payload\":{\"type\":\"task_complete\",\"turn_id\":\"turn-1\"}}";
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(line.as_bytes()).unwrap();
        assert!(appended_codex_task_completions(&path, &mut cursor).is_empty());
        file.write_all(b"\n").unwrap();
        assert_eq!(appended_codex_task_completions(&path, &mut cursor), vec!["turn-1"]);
        fs::remove_file(path).expect("rollout should be removed");
    }

    #[test]
    fn rollout_cursor_does_not_replay_after_file_truncation() {
        let path = std::env::temp_dir().join(format!("agent-mail-notifier-rollout-truncate-{}.jsonl", uuid::Uuid::new_v4()));
        let historical = "{\"type\":\"event_msg\",\"payload\":{\"type\":\"task_complete\",\"turn_id\":\"historical-turn\"}}\n";
        fs::write(&path, format!("{historical}{historical}{historical}"))
            .expect("historical rollout should be written");
        let mut cursor = rollout_cursor_at_end(&path).expect("rollout cursor should initialize at the end");

        fs::write(&path, historical).expect("rollout should be truncated and rewritten");
        assert!(appended_codex_task_completions(&path, &mut cursor).is_empty());

        let current = "{\"type\":\"event_msg\",\"payload\":{\"type\":\"task_complete\",\"turn_id\":\"current-turn\"}}\n";
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(current.as_bytes()).unwrap();
        assert_eq!(appended_codex_task_completions(&path, &mut cursor), vec!["current-turn"]);
        fs::remove_file(path).expect("rollout should be removed");
    }

    #[test]
    fn codex_delivery_claim_survives_state_round_trip() {
        let mut state = StoredState::current();
        assert!(record_codex_delivery(&mut state, "thread-1:turn-1"));
        assert!(!record_codex_delivery(&mut state, "thread-1:turn-1"));

        let serialized = serde_json::to_string(&state).expect("state should serialize");
        let (mut reloaded, _) = crate::state::migrate_legacy_state(&serialized).expect("state should reload");

        assert!(!record_codex_delivery(&mut reloaded, "thread-1:turn-1"));
        assert!(record_codex_delivery(&mut reloaded, "thread-1:turn-2"));
    }

    #[test]
    fn rollout_completion_extracts_problem_reply_and_duration() {
        let path = std::env::temp_dir().join(format!("agent-mail-notifier-rollout-{}.jsonl", uuid::Uuid::new_v4()));
        let lines = [
            serde_json::json!({"type":"session_meta","payload":{"id":"thread-1","source":"vscode","cwd":"D:\\work"}}),
            serde_json::json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-1"}}),
            serde_json::json!({"type":"event_msg","payload":{"type":"user_message","message":"检查归档"}}),
            serde_json::json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-1","duration_ms":1234,"last_agent_message":"已完成"}}),
        ].into_iter().map(|line| line.to_string()).collect::<Vec<_>>().join("\n") + "\n";
        fs::write(&path, lines).expect("rollout should be written");

        let completion = read_codex_rollout_completion(&path, "turn-1").expect("completion should be parsed");
        assert_eq!(completion.thread_id, "thread-1");
        assert_eq!(completion.problem, "检查归档");
        assert_eq!(completion.detail, "已完成");
        assert_eq!(completion.elapsed, Some(Duration::from_millis(1234)));
        fs::remove_file(path).expect("rollout should be removed");
    }
}
