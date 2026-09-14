use agent_mail_notifier_core::restore_claude_hooks_if_owned;
use serde_json::Value;

#[test]
fn restores_only_its_claude_hooks_and_preserves_unrelated_hooks() {
    let installed = r#"{
  "hooks": {
    "Stop": [
      {"hooks": [{"type": "command", "command": "other-tool.exe"}]},
      {"hooks": [{"type": "command", "command": "C:\\Program Files\\Agent Mail Notifier\\AgentMailNotifier.exe", "args": ["--hook", "claude"]}]}
    ],
    "StopFailure": [
      {"hooks": [{"type": "command", "command": "C:\\Program Files\\Agent Mail Notifier\\AgentMailNotifier.exe", "args": ["--hook", "claude"]}]}
    ]
  }
}"#;

    let restored = restore_claude_hooks_if_owned(
        installed,
        r"C:\Program Files\Agent Mail Notifier\AgentMailNotifier.exe",
    )
    .expect("installed JSON should be restorable");
    let settings: Value = serde_json::from_str(&restored).unwrap();

    assert_eq!(settings["hooks"]["Stop"][0]["hooks"][0]["command"], "other-tool.exe");
    assert!(settings["hooks"]["StopFailure"].as_array().unwrap().is_empty());
    assert!(!restored.contains("AgentMailNotifier.exe"));
}
