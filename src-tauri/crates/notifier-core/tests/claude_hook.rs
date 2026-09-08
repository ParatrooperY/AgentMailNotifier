use agent_mail_notifier_core::{install_claude_hooks, restore_claude_hooks_if_owned};
use serde_json::Value;

#[test]
fn adds_only_its_stop_hooks_and_preserves_existing_settings() {
    let original = r#"{
  "env": {"ANTHROPIC_BASE_URL": "https://example.invalid"},
  "hooks": {
    "Stop": [{"hooks": [{"type": "command", "command": "other-tool.exe"}]}]
  }
}"#;

    let updated = install_claude_hooks(
        original,
        r"C:\Program Files\Agent Mail Notifier\AgentMailNotifier.exe",
    )
    .expect("valid JSON should be editable");
    let settings: Value = serde_json::from_str(&updated).unwrap();

    assert_eq!(settings["env"]["ANTHROPIC_BASE_URL"], "https://example.invalid");
    assert_eq!(settings["hooks"]["Stop"][0]["hooks"][0]["command"], "other-tool.exe");
    assert_eq!(settings["hooks"]["Stop"].as_array().unwrap().len(), 2);
    assert_eq!(settings["hooks"]["StopFailure"].as_array().unwrap().len(), 1);
    let serialized = settings.to_string();
    assert!(serialized.contains("AgentMailNotifier.exe"));
    assert!(!serialized.contains("SubagentStop"));
}

#[test]
fn restores_only_its_claude_hooks_and_preserves_unrelated_hooks() {
    let installed = install_claude_hooks(
        r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"other-tool.exe"}]}]}}"#,
        r"C:\Program Files\Agent Mail Notifier\AgentMailNotifier.exe",
    )
    .expect("valid JSON should be editable");

    let restored = restore_claude_hooks_if_owned(
        &installed,
        r"C:\Program Files\Agent Mail Notifier\AgentMailNotifier.exe",
    )
    .expect("installed JSON should be restorable");
    let settings: Value = serde_json::from_str(&restored).unwrap();

    assert_eq!(settings["hooks"]["Stop"][0]["hooks"][0]["command"], "other-tool.exe");
    assert!(settings["hooks"]["StopFailure"].as_array().unwrap().is_empty());
    assert!(!restored.contains("AgentMailNotifier.exe"));
}
