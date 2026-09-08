use agent_mail_notifier_core::{
    install_codex_hook, restore_codex_hook, restore_codex_hook_if_owned,
};

#[test]
fn restores_only_the_previous_notify_callback_when_uninstalling() {
    let original = "model = \"gpt-5.6-sol\"\nnotify = [\"C:\\\\Tools\\\\computer-use.exe\", \"turn-ended\"]\n";
    let installed = install_codex_hook(original, r"C:\Apps\AgentMailNotifier.exe").unwrap();
    let current = format!("{}\n[desktop]\nappearanceTheme = \"dark\"\n", installed.updated_config);

    let restored = restore_codex_hook(&current, installed.previous_notify.as_deref()).unwrap();

    assert!(restored.contains("computer-use.exe"));
    assert!(!restored.contains("AgentMailNotifier.exe"));
    assert!(restored.contains("appearanceTheme = \"dark\""));
}

#[test]
fn keeps_a_notify_callback_changed_by_the_user_after_installation() {
    let current = "notify = [\"C:\\\\Tools\\\\new-callback.exe\", \"turn-ended\"]\n";
    let previous = vec![r"C:\Tools\computer-use.exe".to_owned(), "turn-ended".to_owned()];

    let result = restore_codex_hook_if_owned(
        current,
        Some(&previous),
        r"C:\Apps\AgentMailNotifier.exe",
    )
    .unwrap();

    assert_eq!(result, current);
}
