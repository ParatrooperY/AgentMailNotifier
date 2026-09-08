use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum IntegrationKind {
    Codex,
    Claude,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeliveryContext {
    pub main_program_running: bool,
    pub smtp_verified: bool,
    pub integration_installed: bool,
    pub enabled_preference: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryDecision {
    Deliver,
    DropSilently,
}

pub fn decide_delivery(context: DeliveryContext) -> DeliveryDecision {
    if context.main_program_running
        && context.smtp_verified
        && context.integration_installed
        && context.enabled_preference
    {
        DeliveryDecision::Deliver
    } else {
        DeliveryDecision::DropSilently
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SmtpEncryption {
    Ssl,
    Starttls,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SmtpPreset {
    pub provider_label: &'static str,
    pub host: &'static str,
    pub port: u16,
    pub encryption: SmtpEncryption,
}

pub fn smtp_preset_for(email: &str) -> Option<SmtpPreset> {
    let domain = email.trim().rsplit_once('@')?.1.to_ascii_lowercase();
    match domain.as_str() {
        "qq.com" | "vip.qq.com" | "foxmail.com" => Some(SmtpPreset {
            provider_label: "QQ 邮箱 · SSL 465",
            host: "smtp.qq.com",
            port: 465,
            encryption: SmtpEncryption::Ssl,
        }),
        "163.com" | "126.com" | "yeah.net" => Some(SmtpPreset {
            provider_label: "网易邮箱 · SSL 465",
            host: "smtp.163.com",
            port: 465,
            encryption: SmtpEncryption::Ssl,
        }),
        "outlook.com" | "hotmail.com" | "live.com" => Some(SmtpPreset {
            provider_label: "Outlook · STARTTLS 587",
            host: "smtp.office365.com",
            port: 587,
            encryption: SmtpEncryption::Starttls,
        }),
        "gmail.com" => Some(SmtpPreset {
            provider_label: "Gmail · SSL 465",
            host: "smtp.gmail.com",
            port: 465,
            encryption: SmtpEncryption::Ssl,
        }),
        _ => None,
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CodexMetadata {
    pub title: Option<String>,
    pub source: Option<String>,
    pub agent_path: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CodexEvent {
    #[serde(rename = "thread-id")]
    pub thread_id: String,
    #[serde(rename = "turn-id")]
    pub turn_id: String,
    #[serde(rename = "input-messages", default)]
    pub input_messages: Vec<String>,
    #[serde(rename = "last-assistant-message", default)]
    pub last_assistant_message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventDisposition {
    Deliver,
    DropSilently,
}

pub fn classify_codex_event(
    event: &CodexEvent,
    metadata: Option<&CodexMetadata>,
    duration_seconds: Option<u64>,
) -> EventDisposition {
    if metadata.is_some_and(is_subagent_metadata) {
        return EventDisposition::DropSilently;
    }
    if metadata.is_none()
        && duration_seconds.is_none()
        && is_internal_metadata_reply(&event.last_assistant_message)
    {
        return EventDisposition::DropSilently;
    }
    EventDisposition::Deliver
}

pub fn resolve_codex_title(event: &CodexEvent, metadata: Option<&CodexMetadata>) -> String {
    metadata
        .and_then(|item| item.title.as_deref())
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .or_else(|| {
            event
                .input_messages
                .iter()
                .rev()
                .flat_map(|message| message.lines())
                .map(str::trim)
                .find(|line| !line.is_empty())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "未命名会话".to_owned())
}

fn is_subagent_metadata(metadata: &CodexMetadata) -> bool {
    if metadata.agent_path.as_deref().is_some_and(|value| !value.trim().is_empty()) {
        return true;
    }
    metadata
        .source
        .as_deref()
        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
        .and_then(|value| value.get("subagent").cloned())
        .and_then(|value| value.get("thread_spawn").cloned())
        .is_some_and(|value| value.is_object())
}

fn is_internal_metadata_reply(message: &str) -> bool {
    let Ok(serde_json::Value::Object(object)) = serde_json::from_str(message) else {
        return false;
    };
    !object.is_empty()
        && object.keys().all(|key| matches!(key.as_str(), "title" | "description"))
        && object.values().all(serde_json::Value::is_string)
}

pub fn claude_event_supported(hook_event_name: &str) -> bool {
    matches!(hook_event_name, "Stop" | "StopFailure")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexHookInstall {
    pub updated_config: String,
    pub previous_notify: Option<Vec<String>>,
}

pub fn install_codex_hook(
    config_text: &str,
    executable_path: &str,
) -> Result<CodexHookInstall, String> {
    let mut document = config_text
        .parse::<toml_edit::DocumentMut>()
        .map_err(|error| format!("无法解析 Codex 配置：{error}"))?;
    let current_notify = document
        .get("notify")
        .and_then(toml_edit::Item::as_array)
        .map(|array| {
            array
                .iter()
                .filter_map(toml_edit::Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|command| !command.is_empty());
    let already_owned = current_notify.as_deref().is_some_and(|command| {
        command.get(1).is_some_and(|value| value == "--hook")
            && command.get(2).is_some_and(|value| value == "codex")
            && command.first().is_some_and(|value| {
                value == executable_path
                    || Path::new(value).file_name() == Path::new(executable_path).file_name()
            })
    });
    let exact_path_owned = current_notify.as_deref().is_some_and(|command| {
        command.first().is_some_and(|value| value == executable_path)
            && command.get(1).is_some_and(|value| value == "--hook")
            && command.get(2).is_some_and(|value| value == "codex")
    });
    if exact_path_owned {
        return Ok(CodexHookInstall {
            updated_config: config_text.to_owned(),
            previous_notify: None,
        });
    }
    let mut command = toml_edit::Array::new();
    command.push(executable_path);
    command.push("--hook");
    command.push("codex");
    document["notify"] = toml_edit::value(command);
    Ok(CodexHookInstall {
        updated_config: document.to_string(),
        previous_notify: (!already_owned).then_some(current_notify).flatten(),
    })
}

pub fn install_codex_timing_hook(hooks_text: &str, command: &str) -> Result<String, String> {
    let mut settings: serde_json::Value = if hooks_text.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(hooks_text).map_err(|error| format!("无法解析 Codex hooks：{error}"))?
    };
    let object = settings
        .as_object_mut()
        .ok_or_else(|| "Codex hooks 必须是 JSON 对象".to_owned())?;
    let hooks = object
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .ok_or_else(|| "Codex hooks 必须是对象".to_owned())?;
    let list = hooks
        .entry("UserPromptSubmit")
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut()
        .ok_or_else(|| "Codex UserPromptSubmit hooks 必须是数组".to_owned())?;
    let already_installed = list.iter().any(|group| {
        group["hooks"].as_array().is_some_and(|inner| {
            inner.iter().any(|hook| is_owned_codex_timing_hook(hook, command))
        })
    });
    if !already_installed {
        list.push(serde_json::json!({
            "hooks": [{ "type": "command", "command": command }]
        }));
    }
    serde_json::to_string_pretty(&settings)
        .map_err(|error| format!("无法写入 Codex hooks：{error}"))
}

pub fn restore_codex_timing_hook_if_owned(hooks_text: &str, command: &str) -> Result<String, String> {
    let mut settings: serde_json::Value = serde_json::from_str(hooks_text)
        .map_err(|error| format!("无法解析 Codex hooks：{error}"))?;
    let Some(hooks) = settings.get_mut("hooks").and_then(serde_json::Value::as_object_mut) else {
        return Ok(hooks_text.to_owned());
    };
    let event_is_empty = {
        let Some(groups) = hooks.get_mut("UserPromptSubmit").and_then(serde_json::Value::as_array_mut) else {
            return Ok(hooks_text.to_owned());
        };
        let mut retained_groups = Vec::with_capacity(groups.len());
        for mut group in groups.drain(..) {
            if let Some(inner) = group.get_mut("hooks").and_then(serde_json::Value::as_array_mut) {
                inner.retain(|hook| !is_owned_codex_timing_hook(hook, command));
                if inner.is_empty() { continue; }
            }
            retained_groups.push(group);
        }
        *groups = retained_groups;
        groups.is_empty()
    };
    if event_is_empty { hooks.remove("UserPromptSubmit"); }
    serde_json::to_string_pretty(&settings)
        .map_err(|error| format!("无法写入 Codex hooks：{error}"))
}

pub fn codex_hook_file_is_empty(hooks_text: &str) -> Result<bool, String> {
    let settings: serde_json::Value = serde_json::from_str(hooks_text)
        .map_err(|error| format!("无法解析 Codex hooks：{error}"))?;
    let Some(object) = settings.as_object() else { return Ok(false); };
    Ok(object.is_empty() || object.len() == 1 && object.get("hooks").is_some_and(|hooks| hooks.as_object().is_some_and(|items| items.is_empty())))
}

fn is_owned_codex_timing_hook(hook: &serde_json::Value, command: &str) -> bool {
    hook["type"] == "command" && hook["command"] == command
}

pub fn install_claude_hooks(
    settings_text: &str,
    executable_path: &str,
) -> Result<String, String> {
    let mut settings: serde_json::Value = serde_json::from_str(settings_text)
        .map_err(|error| format!("无法解析 Claude 设置：{error}"))?;
    let object = settings
        .as_object_mut()
        .ok_or_else(|| "Claude 设置必须是 JSON 对象".to_owned())?;
    let hooks = object
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .ok_or_else(|| "Claude hooks 必须是对象".to_owned())?;
    for event_name in ["UserPromptSubmit", "Stop", "StopFailure"] {
        let list = hooks
            .entry(event_name)
            .or_insert_with(|| serde_json::json!([]))
            .as_array_mut()
            .ok_or_else(|| format!("Claude {event_name} hooks 必须是数组"))?;
        let already_installed = list.iter().any(|group| {
            group["hooks"].as_array().is_some_and(|inner| {
                inner.iter().any(|hook| {
                    hook["type"] == "command"
                        && hook["command"] == executable_path
                        && hook["args"].as_array().is_some_and(|args| {
                            args.iter().any(|arg| arg == "claude")
                        })
                })
            })
        });
        if !already_installed {
            list.push(serde_json::json!({
                "hooks": [{
                    "type": "command",
                    "command": executable_path,
                    "args": ["--hook", "claude"]
                }]
            }));
        }
    }
    serde_json::to_string_pretty(&settings)
        .map_err(|error| format!("无法写入 Claude 设置：{error}"))
}

pub fn restore_claude_hooks_if_owned(
    settings_text: &str,
    executable_path: &str,
) -> Result<String, String> {
    let mut settings: serde_json::Value = serde_json::from_str(settings_text)
        .map_err(|error| format!("无法解析 Claude 设置：{error}"))?;
    let Some(hooks) = settings.get_mut("hooks").and_then(serde_json::Value::as_object_mut) else {
        return Ok(settings_text.to_owned());
    };
    for event_name in ["UserPromptSubmit", "Stop", "StopFailure"] {
        let Some(groups) = hooks.get_mut(event_name).and_then(serde_json::Value::as_array_mut) else { continue; };
        let mut retained_groups = Vec::with_capacity(groups.len());
        for mut group in groups.drain(..) {
            if let Some(inner) = group.get_mut("hooks").and_then(serde_json::Value::as_array_mut) {
                inner.retain(|hook| !is_owned_claude_hook(hook, executable_path));
                if inner.is_empty() { continue; }
            }
            retained_groups.push(group);
        }
        *groups = retained_groups;
    }
    serde_json::to_string_pretty(&settings)
        .map_err(|error| format!("无法写入 Claude 设置：{error}"))
}

fn is_owned_claude_hook(hook: &serde_json::Value, executable_path: &str) -> bool {
    hook["type"] == "command"
        && hook["command"] == executable_path
        && hook["args"].as_array().is_some_and(|args| args.iter().any(|arg| arg == "claude"))
}

pub fn restore_codex_hook(
    current_config: &str,
    previous_notify: Option<&[String]>,
) -> Result<String, String> {
    let mut document = current_config
        .parse::<toml_edit::DocumentMut>()
        .map_err(|error| format!("无法解析 Codex 配置：{error}"))?;
    match previous_notify {
        Some(command) if !command.is_empty() => {
            let mut restored = toml_edit::Array::new();
            for argument in command {
                restored.push(argument.as_str());
            }
            document["notify"] = toml_edit::value(restored);
        }
        _ => {
            document.remove("notify");
        }
    }
    Ok(document.to_string())
}

pub fn restore_codex_hook_if_owned(
    current_config: &str,
    previous_notify: Option<&[String]>,
    executable_path: &str,
) -> Result<String, String> {
    let document = current_config
        .parse::<toml_edit::DocumentMut>()
        .map_err(|error| format!("无法解析 Codex 配置：{error}"))?;
    let current_notify = document
        .get("notify")
        .and_then(toml_edit::Item::as_array)
        .map(|array| {
            array
                .iter()
                .filter_map(toml_edit::Value::as_str)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let is_our_hook = current_notify
        .first()
        .is_some_and(|command| *command == executable_path)
        && current_notify.get(1) == Some(&"--hook")
        && current_notify.get(2) == Some(&"codex");
    if !is_our_hook {
        return Ok(current_config.to_owned());
    }
    restore_codex_hook(current_config, previous_notify)
}
