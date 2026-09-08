use agent_mail_notifier_core::install_codex_hook;

#[test]
fn installs_codex_entry_without_losing_the_existing_callback_or_other_settings() {
    let original = r#"model = "gpt-5.6-sol"
notify = ["C:\\Tools\\computer-use.exe", "turn-ended"]

[desktop]
appearanceTheme = "dark"
"#;

    let result = install_codex_hook(
        original,
        r"C:\Program Files\Agent Mail Notifier\AgentMailNotifier.exe",
    )
    .expect("valid TOML should be editable");

    assert_eq!(
        result.previous_notify,
        Some(vec![
            r"C:\Tools\computer-use.exe".to_owned(),
            "turn-ended".to_owned(),
        ]),
    );
    assert!(result.updated_config.contains("model = \"gpt-5.6-sol\""));
    assert!(result.updated_config.contains("appearanceTheme = \"dark\""));
    assert!(result.updated_config.contains("AgentMailNotifier.exe"));
    assert!(result.updated_config.contains("--hook"));
    assert!(result.updated_config.contains("codex"));
    assert!(!result.updated_config.contains("computer-use.exe"));
}

#[test]
fn repairing_an_owned_codex_hook_does_not_replace_the_original_callback() {
    let executable = r"C:\Program Files\Agent Mail Notifier\AgentMailNotifier.exe";
    let original = r#"notify = ["C:\\Tools\\computer-use.exe", "turn-ended"]
"#;
    let first = install_codex_hook(original, executable).expect("first install should succeed");
    let second = install_codex_hook(&first.updated_config, executable).expect("repair should succeed");

    assert_eq!(first.previous_notify, Some(vec![
        r"C:\Tools\computer-use.exe".to_owned(),
        "turn-ended".to_owned(),
    ]));
    assert_eq!(second.previous_notify, None);
    assert_eq!(second.updated_config, first.updated_config);
}

#[test]
fn reinstalling_after_the_app_path_changes_keeps_the_original_callback() {
    let original = r#"notify = ["C:\\Tools\\computer-use.exe", "turn-ended"]
"#;
    let first = install_codex_hook(original, r"C:\Old\Agent Mail Notifier\agent-mail-notifier.exe").unwrap();
    let repaired = install_codex_hook(&first.updated_config, r"C:\New\Agent Mail Notifier\agent-mail-notifier.exe").unwrap();

    assert_eq!(repaired.previous_notify, None);
    assert!(repaired.updated_config.contains(r"C:\New\Agent Mail Notifier\agent-mail-notifier.exe"));
}

#[test]
fn repairing_an_owned_codex_hook_preserves_the_existing_config_text() {
    let original = r#"model_provider = "custom"
model = "gpt-5.6-sol"
model_reasoning_effort = "high"
notify = ['D:\Agent Mail Notifier\agent-mail-notifier.exe', "--hook", "codex"]

[desktop]
followUpQueueMode = "queue"
appearanceTheme = "dark"
"#;

    let result = install_codex_hook(
        original,
        r"D:\Agent Mail Notifier\agent-mail-notifier.exe",
    )
    .expect("valid TOML should be editable");

    assert_eq!(result.previous_notify, None);
    assert_eq!(result.updated_config, original);
}
