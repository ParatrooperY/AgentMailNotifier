use agent_mail_notifier_core::{restore_codex_hook, restore_codex_hook_if_owned};

#[test]
fn restores_only_the_previous_notify_callback_when_uninstalling() {
    let current = "model = \"gpt-5.6-sol\"\nnotify = [\"C:\\\\Apps\\\\AgentMailNotifier.exe\", \"--hook\", \"codex\"]\n[desktop]\nappearanceTheme = \"dark\"\n";
    let previous = vec![r"C:\Tools\computer-use.exe".to_owned(), "turn-ended".to_owned()];

    let restored = restore_codex_hook(current, Some(&previous)).unwrap();

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
