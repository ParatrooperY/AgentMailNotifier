use std::collections::VecDeque;

use agent_mail_notifier_core::IntegrationKind;
use serde::{Deserialize, Serialize, Serializer};

pub const CURRENT_STATE_VERSION: u32 = 2;
const MAX_HISTORY_ENTRIES: usize = 100;
const MAX_CODEX_DELIVERED_EVENTS: usize = 2_000;

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StoredSmtp {
    pub email: String,
    pub provider: Option<String>,
    pub provider_label: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub encryption: Option<String>,
    pub verified: bool,
    pub last_tested_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct StoredIntegration {
    pub enabled_preference: bool,
    pub codex_previous_notify: Option<Vec<String>>,
    pub codex_timing_hook_created: bool,
    pub codex_delivered_events: VecDeque<String>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub source: String,
    pub title: String,
    pub result: String,
    pub detail: String,
    pub occurred_at: String,
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct StoredChannel {
    pub smtp: StoredSmtp,
    pub integration: StoredIntegration,
    pub history: VecDeque<HistoryEntry>,
}

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct StoredState {
    pub version: u32,
    pub codex: StoredChannel,
    pub claude: StoredChannel,
    pub legacy_history: VecDeque<HistoryEntry>,
}

impl StoredState {
    pub fn current() -> Self {
        Self { version: CURRENT_STATE_VERSION, ..Self::default() }
    }
}

#[derive(Default, Deserialize, Serialize)]
#[serde(default)]
struct PersistedState {
    version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    smtp: Option<StoredSmtp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    codex_smtp: Option<StoredSmtp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    claude_smtp: Option<StoredSmtp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    codex: Option<StoredIntegration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    claude: Option<StoredIntegration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    history: Option<VecDeque<HistoryEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    codex_history: Option<VecDeque<HistoryEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    claude_history: Option<VecDeque<HistoryEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    legacy_history: Option<VecDeque<HistoryEntry>>,
}

impl Serialize for StoredState {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        PersistedState {
            version: Some(self.version),
            smtp: None,
            codex_smtp: Some(self.codex.smtp.clone()),
            claude_smtp: Some(self.claude.smtp.clone()),
            codex: Some(self.codex.integration.clone()),
            claude: Some(self.claude.integration.clone()),
            history: None,
            codex_history: Some(self.codex.history.clone()),
            claude_history: Some(self.claude.history.clone()),
            legacy_history: (!self.legacy_history.is_empty()).then(|| self.legacy_history.clone()),
        }
        .serialize(serializer)
    }
}

pub fn migrate_legacy_state(text: &str) -> Result<(StoredState, bool), String> {
    let raw: PersistedState = serde_json::from_str(text).map_err(|error| format!("无法读取设置：{error}"))?;
    let legacy_smtp = raw.smtp.unwrap_or_default();
    let legacy_history = raw.history.unwrap_or_default();
    let has_current_shape = raw.version == Some(CURRENT_STATE_VERSION)
        && raw.codex_smtp.is_some()
        && raw.claude_smtp.is_some()
        && raw.codex_history.is_some()
        && raw.claude_history.is_some();
    let codex_history = raw.codex_history.unwrap_or_else(|| {
        legacy_history
            .iter()
            .filter(|entry| entry.source.eq_ignore_ascii_case("codex"))
            .cloned()
            .collect()
    });
    let claude_history = raw.claude_history.unwrap_or_else(|| {
        legacy_history
            .iter()
            .filter(|entry| entry.source.eq_ignore_ascii_case("claude code"))
            .cloned()
            .collect()
    });
    let assigned_ids = codex_history
        .iter()
        .chain(claude_history.iter())
        .map(|entry| entry.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let mut unassigned_history = raw.legacy_history.unwrap_or_default();
    for entry in legacy_history {
        if !assigned_ids.contains(entry.id.as_str()) {
            unassigned_history.push_back(entry);
        }
    }
    let mut state = StoredState {
        version: CURRENT_STATE_VERSION,
        codex: StoredChannel {
            smtp: raw.codex_smtp.unwrap_or_else(|| legacy_smtp.clone()),
            integration: raw.codex.unwrap_or_default(),
            history: codex_history,
        },
        claude: StoredChannel {
            smtp: raw.claude_smtp.unwrap_or_else(|| legacy_smtp.clone()),
            integration: raw.claude.unwrap_or_default(),
            history: claude_history,
        },
        legacy_history: unassigned_history,
    };
    bound_history(&mut state.codex.history);
    bound_history(&mut state.claude.history);
    while state.codex.integration.codex_delivered_events.len() > MAX_CODEX_DELIVERED_EVENTS {
        state.codex.integration.codex_delivered_events.pop_front();
    }
    Ok((state, !has_current_shape))
}

fn bound_history(history: &mut VecDeque<HistoryEntry>) {
    while history.len() > MAX_HISTORY_ENTRIES {
        history.pop_back();
    }
}

pub fn channel(state: &StoredState, kind: IntegrationKind) -> &StoredChannel {
    match kind {
        IntegrationKind::Codex => &state.codex,
        IntegrationKind::Claude => &state.claude,
    }
}

pub fn channel_mut(state: &mut StoredState, kind: IntegrationKind) -> &mut StoredChannel {
    match kind {
        IntegrationKind::Codex => &mut state.codex,
        IntegrationKind::Claude => &mut state.claude,
    }
}

pub fn channel_name(kind: IntegrationKind) -> &'static str {
    match kind {
        IntegrationKind::Codex => "codex",
        IntegrationKind::Claude => "claude",
    }
}

pub fn credential_service_name(kind: IntegrationKind) -> String {
    format!("agent-mail-notifier-{}", channel_name(kind))
}

pub fn source_name(kind: IntegrationKind) -> &'static str {
    match kind {
        IntegrationKind::Codex => "Codex",
        IntegrationKind::Claude => "Claude Code",
    }
}

pub fn mark_disconnected_for_uninstall(state: &mut StoredState) {
    for channel in [&mut state.codex, &mut state.claude] {
        channel.integration.enabled_preference = false;
    }
}

#[cfg(test)]
mod tests {
    use super::{credential_service_name, mark_disconnected_for_uninstall, migrate_legacy_state, HistoryEntry, StoredState};
    use agent_mail_notifier_core::IntegrationKind;
    use serde_json::json;

    #[test]
    fn migrates_shared_smtp_and_history_into_independent_channels() {
        let legacy = json!({
            "smtp": {
                "email": "codex-user",
                "provider": "preset",
                "provider_label": "Test SMTP",
                "host": "smtp.example.invalid",
                "port": 465,
                "encryption": "ssl",
                "verified": true,
                "last_tested_at": "2026-08-18 10:00",
                "error": null
            },
            "history": [
                {"id":"1","source":"Codex","title":"C","result":"sent","detail":"ok","occurred_at":"2026-08-18 10:01"},
                {"id":"2","source":"Claude Code","title":"K","result":"failed","detail":"failed","occurred_at":"2026-08-18 10:02"}
            ]
        });

        let (state, migrated) = migrate_legacy_state(&legacy.to_string()).expect("legacy state should parse");

        assert!(migrated);
        assert_eq!(state.codex.smtp.email, "codex-user");
        assert_eq!(state.claude.smtp.host.as_deref(), Some("smtp.example.invalid"));
        assert_eq!(state.codex.history.len(), 1);
        assert_eq!(state.claude.history.len(), 1);
        assert_eq!(state.codex.history[0].source, "Codex");
        assert_eq!(state.claude.history[0].source, "Claude Code");
    }

    #[test]
    fn current_state_is_idempotent_and_keeps_channels_isolated() {
        let current = json!({
            "version": 2,
            "codex_smtp": {"email":"codex-user","provider_label":"Codex","verified":true},
            "claude_smtp": {"email":"claude-user","provider_label":"Claude","verified":false},
            "codex": {"enabled_preference":true},
            "claude": {"enabled_preference":false},
            "codex_history": [],
            "claude_history": []
        });

        let (state, migrated) = migrate_legacy_state(&current.to_string()).expect("current state should parse");

        assert!(!migrated);
        assert_eq!(state.codex.smtp.email, "codex-user");
        assert_eq!(state.claude.smtp.email, "claude-user");
        assert!(state.codex.integration.enabled_preference);
        assert!(!state.claude.integration.enabled_preference);
    }

    #[test]
    fn a_settings_file_written_by_the_shipped_build_keeps_its_verified_channels() {
        // Field names here must match what the application actually writes to
        // disk: camelCase inside smtp and history entries, snake_case inside
        // the integration block.
        let on_disk = json!({
            "version": 2,
            "codex_smtp": {"email":"codex-user","provider":"qq","providerLabel":"QQ 邮箱 · SSL 465","host":"smtp.qq.com","port":465,"encryption":"ssl","verified":true,"lastTestedAt":"2026-08-22 16:26","error":null},
            "claude_smtp": {"email":"claude-user","provider":"163","providerLabel":"网易邮箱 · SSL 465","host":"smtp.163.com","port":465,"encryption":"ssl","verified":true,"lastTestedAt":"2026-08-22 23:54","error":null},
            "codex": {"enabled_preference":true,"codex_previous_notify":null,"codex_timing_hook_created":false,"codex_delivered_events":["event-1"]},
            "claude": {"enabled_preference":true,"codex_previous_notify":null,"codex_timing_hook_created":false,"codex_delivered_events":[]},
            "codex_history": [{"id":"entry-1","source":"Codex","title":"任务","result":"sent","detail":"","occurredAt":"2026-08-22 16:26"}],
            "claude_history": [{"id":"entry-2","source":"Claude Code","title":"任务","result":"sent","detail":"","occurredAt":"2026-08-22 23:54"}]
        });

        let (state, migrated) = migrate_legacy_state(&on_disk.to_string()).expect("shipped settings should parse");

        assert!(!migrated, "a file already in the current shape must not be migrated again");
        assert!(state.codex.smtp.verified);
        assert!(state.claude.smtp.verified);
        assert_eq!(state.codex.smtp.provider_label, "QQ 邮箱 · SSL 465");
        assert_eq!(state.codex.smtp.port, Some(465));
        assert!(state.codex.integration.enabled_preference);
        assert_eq!(state.codex.integration.codex_delivered_events.len(), 1);
        assert_eq!(state.codex.history[0].occurred_at, "2026-08-22 16:26");
        assert_eq!(state.claude.history[0].occurred_at, "2026-08-22 23:54");
    }

    #[test]
    fn credential_service_names_are_distinct_per_channel() {
        assert_ne!(
            credential_service_name(IntegrationKind::Codex),
            credential_service_name(IntegrationKind::Claude)
        );
        assert_eq!(
            credential_service_name(IntegrationKind::Codex),
            "agent-mail-notifier-codex"
        );
    }

    #[test]
    fn migrated_state_round_trips_without_repeating_migration() {
        let legacy = serde_json::json!({
            "smtp": {"email":"codex-user","provider_label":"Test SMTP","verified":true},
            "history": []
        });
        let (state, first_migration) = migrate_legacy_state(&legacy.to_string()).expect("legacy state should parse");
        let serialized = serde_json::to_string(&state).expect("current state should serialize");
        let (reloaded, second_migration) = migrate_legacy_state(&serialized).expect("current state should parse");

        assert!(first_migration);
        assert!(!second_migration);
        assert_eq!(reloaded.codex.smtp.email, "codex-user");
        assert_eq!(reloaded.claude.smtp.email, "codex-user");
    }

    #[test]
    fn retaining_data_on_uninstall_disconnects_channels_without_clearing_data() {
        let mut state = StoredState::current();
        state.codex.smtp.email = "codex-user".to_owned();
        state.codex.integration.enabled_preference = true;
        state.codex.history.push_back(HistoryEntry { id: "entry".to_owned(), title: "task".to_owned(), ..HistoryEntry::default() });

        mark_disconnected_for_uninstall(&mut state);

        assert!(!state.codex.integration.enabled_preference);
        assert_eq!(state.codex.smtp.email, "codex-user");
        assert_eq!(state.codex.history.len(), 1);
    }

    #[test]
    fn migration_keeps_history_bounded_to_the_newest_entries() {
        let history = (0..105).map(|index| json!({
            "id": index.to_string(),
            "source": "Codex",
            "title": "task",
            "result": "sent",
            "detail": "ok",
            "occurred_at": "now"
        })).collect::<Vec<_>>();
        let legacy = json!({"history": history});

        let (state, _) = migrate_legacy_state(&legacy.to_string()).expect("legacy state should parse");

        assert_eq!(state.codex.history.len(), 100);
        assert_eq!(state.codex.history.front().map(|entry| entry.id.as_str()), Some("0"));
        assert_eq!(state.codex.history.back().map(|entry| entry.id.as_str()), Some("99"));
    }

}
