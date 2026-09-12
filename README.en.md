# Agent Mail Notifier

English | [简体中文](README.md)

Sends you an email when a Codex or Claude Code task finishes.

Leave long jobs running unattended. When a turn ends — or fails and needs you — a mail arrives.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Release](https://img.shields.io/github/v/release/ParatrooperY/AgentMailNotifier?include_prereleases)](https://github.com/ParatrooperY/AgentMailNotifier/releases)
![Platform](https://img.shields.io/badge/platform-Windows%20x64-lightgrey)

## Features

- **Separate channels.** Codex and Claude Code each have their own mailbox settings and activity history.
- **Authorization codes stay off disk.** SMTP credentials live in Windows Credential Manager. The settings file only keeps host, port, and other non-secret fields.
- **Presets for common providers.** QQ, NetEase, Outlook, and Gmail fill in the SMTP host and port automatically. Anything else can be entered by hand.
- **Read-only listeners.** The app watches session transcripts to decide when a task finished. It does not patch Codex or Claude Code configuration.
- **Stays in the tray.** Closing the window leaves it running. The tray menu can toggle either channel.
- **Noise is filtered.** Subagent events and internal receipts are dropped. One finished turn produces one mail.

## Screenshots

**Codex**

![Codex channel](docs/codex.png)

**Claude Code**

![Claude Code channel](docs/claudecode.png)

## Install

Download from [Releases](https://github.com/ParatrooperY/AgentMailNotifier/releases):

- **Setup** — recommended. Installs to a stable path.
- **Portable** — no installer. After you move it, re-enable each channel once.

The first release is unsigned. Windows SmartScreen may block it; choose **More info**, then **Run anyway**.

## Usage

**1. Configure mail.** Enter the sender address and SMTP authorization code, then run SMTP test. A test message in the inbox means it works.

The authorization code is not the mailbox login password. Enable SMTP in the provider's settings first. For QQ Mail: Settings → Account → POP3/SMTP. For NetEase: Settings → POP3/SMTP/IMAP.

**2. Enable notifications.** Turn on Codex and Claude Code independently.

**3. Leave it running.** Closing the window is fine as long as the tray icon is there. Events that happen while the app is not running are discarded and not replayed later.

### SMTP presets

| Provider | Host | Port | Encryption |
| --- | --- | --- | --- |
| QQ / Foxmail | smtp.qq.com | 465 | SSL |
| NetEase 163 / 126 / yeah.net | smtp.163.com | 465 | SSL |
| Outlook / Hotmail / Live | smtp.office365.com | 587 | STARTTLS |
| Gmail | smtp.gmail.com | 465 | SSL |

Use Custom for any other provider and fill in host, port, and encryption yourself.

## Known limitations

- **Windows x64 only.** Relies on Windows Credential Manager and local transcript paths.
- **Replies cut off by the token limit are not mailed.** A turn is treated as finished on `end_turn` or `stop_sequence`. A `max_tokens` truncation is skipped.
- **Events while the app is stopped are dropped.** There is no offline queue, so a restart does not dump a backlog of stale mail.
- **Moving the portable exe** requires re-enabling each channel.

## Development

Requires Node.js, the Rust toolchain, and Visual Studio Desktop C++ build tools.

```bash
npm install
```

Dev mode:

```bash
npm run tauri dev
```

Tests:

```bash
npm test
```

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Package:

```bash
npm run tauri build
```

Artifacts land in `src-tauri/target/release/bundle/`. Auto-update is intentionally off; publish installers to GitHub Releases by hand.

## Layout

```
src/                        React UI
src-tauri/src/              Tauri process, state, SMTP send
src-tauri/crates/
  notifier-core/            Event parsing, delivery rules, SMTP presets
docs/                       README screenshots
```

The UI lives in `src/`. `src-tauri/src/` watches transcripts and sends mail. `notifier-core` is a Tauri-free logic crate; most tests cover it.

## Safety

- SMTP authorization codes are stored only in Windows Credential Manager. They are not written to the settings file, history, or the UI. The input is cleared after a successful test.
- The app does not modify Codex or Claude Code config files. It only reads session transcripts.
- No inbound ports, no third-party upload. Mail goes from this machine to the SMTP server you configured.

## Feedback

Open an [Issue](https://github.com/ParatrooperY/AgentMailNotifier/issues) for bugs or ideas. Code contributions are not accepted at this time.

## License

[MIT](LICENSE)
