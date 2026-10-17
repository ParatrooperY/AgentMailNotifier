## Agent Mail Notifier v0.1.5

Codex 需要你回答问题时，现在也能收到邮件提醒，无需等到整轮任务结束。

### 变更

- 新增 Codex 交互提问邮件通知，在等待你回答期间即可发送；邮件包含问题、选项及选项说明，并显示可用的项目和聊天名称。
- 同一问题在会话记录中出现多次时只通知一次，已发送记录会保存到本地；提问通知与任务完成通知分别记录。
- 改进新会话的读取方式，避免漏掉通知程序运行期间新建会话中的早期提问和任务完成事件。
- 修复 Codex 通知监听器中路径被移动后再次使用导致的编译错误。

### Changes

Receive an email when Codex needs your input, without waiting for the entire task to finish.

- Added email notifications for interactive Codex questions while an answer is pending. Emails include the question, choices and their descriptions, plus project and chat names when available.
- Deduplicated repeated log entries for the same question and persisted delivery records locally. Question notifications and task completion notifications are tracked separately.
- Improved reading of sessions created while the notifier is running, so early questions and task completion events are not skipped.
- Fixed a compilation error caused by reusing a moved path in the Codex notification listener.

**完整变更 / Full changelog**: https://github.com/ParatrooperY/AgentMailNotifier/compare/v0.1.4...v0.1.5
