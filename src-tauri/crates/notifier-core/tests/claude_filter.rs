use agent_mail_notifier_core::claude_event_supported;

#[test]
fn accepts_top_level_completion_hooks_and_rejects_subagents() {
    assert!(claude_event_supported("Stop"));
    assert!(claude_event_supported("StopFailure"));
    assert!(!claude_event_supported("SubagentStop"));
    assert!(!claude_event_supported("Notification"));
}
