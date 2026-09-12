<h1 align="center">Agent Mail Notifier</h1>

<p align="center"><b>Email notifications for finished Codex Desktop / Claude Code Desktop tasks</b> (Tauri 2 + Rust + React)</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green.svg" alt="License: MIT" /></a>
  <a href="https://github.com/ParatrooperY/AgentMailNotifier/releases"><img src="https://img.shields.io/github/v/release/ParatrooperY/AgentMailNotifier?include_prereleases" alt="Release" /></a>
  <img src="https://img.shields.io/badge/platform-Windows%20x64-lightgrey" alt="Platform" />
</p>

<p align="center"><b>English</b> · <a href="README.md">简体中文</a></p>

## What this is

Sends you an email when a Codex Desktop or Claude Code Desktop task finishes. Leave long jobs running unattended — when a turn ends, or fails and needs you, a mail arrives.

The app only reads the two clients' session transcripts to decide whether a task finished; it never writes to their configuration. SMTP authorization codes are handed to Windows Credential Manager instead of being stored in the settings file.

## Features

- **Isolated channels**: separate sender mailboxes for Codex Desktop and Claude Code Desktop; separate activity history; the tray menu toggles either one on its own.
- **Credential safety**: SMTP authorization codes go into Windows Credential Manager; the settings file keeps only host, port, encryption and other non-secret fields; the input is cleared after a successful test.
- **Provider presets**: QQ / Foxmail, NetEase 163 / 126 / yeah.net, Outlook / Hotmail / Live and Gmail fill in host and port automatically; anything else takes a manual host, port and SSL or STARTTLS.
- **Read-only listeners**: transcripts decide when a turn ended, so Codex's `config.toml` and Claude Code's `settings.json` are never patched; no inbound port is opened.
- **Event filtering**: subagent events and internal receipts are dropped; one finished turn produces exactly one mail, never a duplicate for the same completion.
- **Runs in the background**: closing the window leaves it in the system tray; the tray menu check marks show each channel's current state, and quitting stops listening entirely.

## Screenshots

**Codex Desktop**

![Codex channel](docs/codex.png)

**Claude Code Desktop**

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

### SMTP presets

| Provider | Host | Port | Encryption |
| --- | --- | --- | --- |
| QQ / Foxmail | smtp.qq.com | 465 | SSL |
| NetEase 163 / 126 / yeah.net | smtp.163.com | 465 | SSL |
| Outlook / Hotmail / Live | smtp.office365.com | 587 | STARTTLS |
| Gmail | smtp.gmail.com | 465 | SSL |

Use Custom for any other provider and fill in host, port, and encryption yourself.

## Known limitations

- **Windows x64 only**, since it relies on Windows Credential Manager and local transcript paths.
- **Replies cut off by the token limit are not mailed**, because a turn counts as finished on `end_turn` or `stop_sequence`, and a `max_tokens` truncation is skipped.
- **Events while the app is stopped are dropped**, with no offline listening queue, so a restart does not dump a backlog of stale mail.
- **Moving the portable exe** requires re-enabling each channel.

## Development

### The app is two halves

The interface half is written with web technology (React) and runs inside an embedded browser window. The working half is Rust: it reads transcripts, sends mail, and owns the tray icon. Tauri is the framework in between, and it fuses both halves into one exe.

That means two toolchains: **Node.js** for the interface half, **Rust** for the working half.

On Windows you also need the **Visual Studio Desktop C++ build tools**. Rust does not ship its own linker — the program that stitches compiled fragments into an exe — so on Windows it borrows Microsoft's. In Visual Studio Installer, tick "Desktop development with C++"; a full Visual Studio install is not required.

### Install dependencies

```bash
npm install
```

This only installs the interface half, into `node_modules/`. The Rust half needs no manual step: on the first build, Cargo (Rust's package manager) downloads what it needs and piles build output into `src-tauri/target/`. Both directories are large and are kept out of the repository by `.gitignore`.

### Dev mode

```bash
npm run tauri dev
```

One command, three things behind it, always in this order:

First it runs `beforeDevCommand` from `src-tauri/tauri.conf.json`, which starts the frontend dev server (Vite) and serves the interface at `http://localhost:1420`.

Then it compiles the Rust half and opens a desktop window whose contents point at that address.

Then it keeps watching. Edit interface code under `src/` and the window refreshes immediately with no restart (this is hot reload). Edit Rust code under `src-tauri/` and it recompiles and reopens the window, which is slower.

**The first run is slow.** Rust compiles several hundred dependency crates from scratch, so ten-plus minutes is normal. Later runs hit the cache and take seconds.

### Tests

Each half has its own tests and its own command.

Interface:

```bash
npm test
```

Rust:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

`--manifest-path` tells Cargo where the config file lives. The Rust code sits in the `src-tauri/` subdirectory, so without that flag you would have to `cd` there first.

### Package an installer

```bash
npm run tauri build
```

This adds a few steps on top of dev mode:

It runs `tsc --noEmit` first to check for type errors (`--noEmit` means check only, write nothing). An error stops the build rather than shipping a broken package.

Then Vite compiles the interface into static files under `dist/`.

Then Rust is compiled in release mode. Unlike dev mode this enables optimisation: slower to compile, faster and smaller to run.

Finally the interface files and the Rust program are packed into one exe, wrapped in an NSIS installer.

Artifacts land in two places:

`src-tauri/target/release/bundle/nsis/` — the installer

`src-tauri/target/release/agent-mail-notifier.exe` — the standalone exe, which can be renamed and shipped as the portable build

Auto-update is intentionally off, so publishing a new version means uploading both files to GitHub Releases by hand.

## Layout

```
src/                                    React UI
src-tauri/src/                          Tauri process, state, SMTP send
src-tauri/crates/notifier-core/         Event parsing, delivery rules, SMTP presets
docs/                                   App screenshots
```

The UI lives in `src/`, `src-tauri/src/` watches transcripts and sends mail, and `notifier-core` is a Tauri-free logic crate covering event filtering and delivery rules.

## Safety

- SMTP authorization codes are stored only in Windows Credential Manager. They are not written to the settings file, history, or the UI. The input is cleared after a successful test.
- The app does not modify Codex or Claude Code config files. It only reads session transcripts.
- No inbound ports and no third-party upload, mail goes straight from this machine to the configured SMTP server.

## Feedback

Submit an [Issue](https://github.com/ParatrooperY/AgentMailNotifier/issues) for bugs or ideas. PRs are not accepted at present.

## License

[MIT](LICENSE)
