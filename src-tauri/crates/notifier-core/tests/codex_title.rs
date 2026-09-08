use agent_mail_notifier_core::{CodexEvent, CodexMetadata, resolve_codex_title};

fn event(messages: Vec<&str>) -> CodexEvent {
    CodexEvent {
        thread_id: "thread-1".into(),
        turn_id: "turn-1".into(),
        input_messages: messages.into_iter().map(str::to_owned).collect(),
        last_assistant_message: "完成".into(),
    }
}

#[test]
fn uses_the_existing_local_title_and_falls_back_to_the_latest_request_when_missing() {
    let metadata = CodexMetadata {
        title: Some("你是什么模型".into()),
        ..CodexMetadata::default()
    };
    assert_eq!(resolve_codex_title(&event(vec!["查询模型和美国时间"]), Some(&metadata)), "你是什么模型");
    assert_eq!(resolve_codex_title(&event(vec!["\n\n查询模型和美国时间\n细节"]), None), "查询模型和美国时间");
}
