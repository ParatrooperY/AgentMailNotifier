use agent_mail_notifier_core::{
    CodexEvent, CodexMetadata, EventDisposition, classify_codex_event,
};

fn event(message: &str) -> CodexEvent {
    CodexEvent {
        thread_id: "thread-1".into(),
        turn_id: "turn-1".into(),
        input_messages: vec!["请处理这个任务".into()],
        last_assistant_message: message.into(),
    }
}

#[test]
fn drops_hidden_codex_work_without_suppressing_a_regular_json_reply() {
    let subagent = CodexMetadata {
        title: Some("内部研究".into()),
        source: Some(r#"{"subagent":{"thread_spawn":{"depth":1}}}"#.into()),
        agent_path: Some("/root/research".into()),
    };
    assert_eq!(
        classify_codex_event(&event("完成"), Some(&subagent), Some(10)),
        EventDisposition::DropSilently,
    );

    let metadata_reply = event(
        r#"{"title":"优化 Codex 邮件通知信息","description":"过滤内部任务"}"#,
    );
    assert_eq!(
        classify_codex_event(&metadata_reply, None, None),
        EventDisposition::DropSilently,
    );

    let regular_json = event(r#"{"status":"ok","items":[1,2,3]}"#);
    assert_eq!(
        classify_codex_event(&regular_json, None, None),
        EventDisposition::Deliver,
    );
}
