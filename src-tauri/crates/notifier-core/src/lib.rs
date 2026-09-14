use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum IntegrationKind {
    Codex,
    Claude,
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
