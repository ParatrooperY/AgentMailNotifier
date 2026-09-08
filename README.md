# Agent Mail Notifier

Codex 和 Claude Code 跑完一轮任务后，给你发一封邮件。

挂机跑长任务时不用一直守着终端。任务结束、或者中途失败需要你介入，邮件会告诉你。

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
![Platform](https://img.shields.io/badge/platform-Windows%20x64-lightgrey)
![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB)

## 功能

- **两个通道各自独立**：Codex 和 Claude Code 分别配置发件邮箱、分别保存通知记录，互不影响。
- **授权码不落盘**：SMTP 授权码存进 Windows 凭据管理器，配置文件里只有主机、端口这类非敏感项。
- **常见邮箱免填服务器**：输入 QQ、网易、Outlook、Gmail 地址会自动补全 SMTP 主机和端口，其它邮箱可手填。
- **只读监听，不改你的配置**：通过读取会话记录判断任务完成，不往 Codex 或 Claude Code 的配置里插东西。
- **托盘常驻**：关掉窗口继续在后台跑，托盘菜单可直接开关两个通道。
- **过滤噪音**：子代理产生的中间事件、程序内部回执不会触发邮件，一轮任务只发一封。

## 安装

从 [Releases](https://github.com/ParatrooperY/AgentMailNotifier/releases) 下载：

- **AgentMailNotifier-Setup.exe** — 推荐。安装到固定路径。
- **AgentMailNotifier-Portable.exe** — 免安装，可随意放置。

首个版本没有做代码签名，Windows SmartScreen 可能拦一下，点**更多信息**再点**仍要运行**即可。

## 使用

**第一步，配置邮箱。** 填发件邮箱和授权码，点 SMTP 测试。收到测试邮件说明通了。

授权码不是邮箱登录密码，要去邮箱设置里单独开通 SMTP 服务后获取。QQ 邮箱在「设置 → 账号 → POP3/SMTP 服务」，网易在「设置 → POP3/SMTP/IMAP」。

**第二步，启用通知。** 分别打开 Codex 和 Claude Code 的开关。

**第三步，让它待着。** 窗口关掉不影响，托盘图标在就行。程序没运行时不会发邮件，也不会堆积后补发。

### 支持的邮箱预设

| 邮箱 | SMTP 主机 | 端口 | 加密 |
| --- | --- | --- | --- |
| QQ / Foxmail | smtp.qq.com | 465 | SSL |
| 网易 163 / 126 / yeah.net | smtp.163.com | 465 | SSL |
| Outlook / Hotmail / Live | smtp.office365.com | 587 | STARTTLS |
| Gmail | smtp.gmail.com | 465 | SSL |

其它邮箱选自定义，手填主机、端口和加密方式。

## 已知限制

- **只支持 Windows x64。** 依赖 Windows 凭据管理器和本机会话记录路径。
- **被长度上限截断的回复不发通知。** 判定「一轮结束」依赖会话记录里的收尾标记，正常收尾和被停止串收尾都算完成，但撞到 token 上限的半截回复会跳过。
- **程序没运行时的事件会丢。** 设计如此，不做离线队列，避免重启后收到一堆过期通知。
- **移动安装路径后**便携版需要重新启用一次通道。

## 开发

需要 Node.js、Rust 工具链和 Visual Studio Desktop C++ 构建工具。

```bash
npm install
```

开发模式：

```bash
npm run tauri dev
```

跑测试：

```bash
npm test
```

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

打包：

```bash
npm run tauri build
```

产物在 `src-tauri/target/release/bundle/`。项目故意没有启用自动更新，发版需要手动上传安装包到 Releases。

## 项目结构

```
src/                        React 前端
src-tauri/src/              Tauri 主进程、状态管理、SMTP 发送
src-tauri/crates/
  notifier-core/            事件解析、投递判定、SMTP 预设（纯逻辑，可单测）
```

前端负责界面，`src-tauri/src/` 负责监听会话记录和发信，`notifier-core` 是不依赖 Tauri 的纯逻辑层，事件过滤和投递条件判定都在这里，测试主要覆盖它。

## 安全说明

- SMTP 授权码只存 Windows 凭据管理器，不写进配置文件、不进通知记录、不出现在界面上。测试成功后输入框会自动清空。
- 程序不修改 Codex 和 Claude Code 的配置文件，只读取会话记录。
- 不建立任何对外监听端口，不上传数据到第三方。邮件直接从本机发到你配置的 SMTP 服务器。

## 贡献

欢迎 Issue 和 PR。改动前建议先开 Issue 说一下方向，避免白做。

提交前请确保：

```bash
npm run build && npm test
```

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

## 许可

[MIT](LICENSE)
