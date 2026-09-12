<h1 align="center">Agent Mail Notifier</h1>

<p align="center"><b>Codex / Claude Code 任务完成邮件通知</b>（Tauri 2 + Rust + React）</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green.svg" alt="License: MIT" /></a>
  <a href="https://github.com/ParatrooperY/AgentMailNotifier/releases"><img src="https://img.shields.io/github/v/release/ParatrooperY/AgentMailNotifier?include_prereleases" alt="Release" /></a>
  <img src="https://img.shields.io/badge/platform-Windows%20x64-lightgrey" alt="Platform" />
</p>

<p align="center"><a href="README.en.md">English</a> · <b>简体中文</b></p>

## 这是什么

Codex 和 Claude Code 跑完一轮任务后，给你发一封邮件。挂机跑长任务时不用一直守着终端，任务结束、或者中途失败需要你介入，邮件会告诉你。

程序只读取两个客户端的会话记录来判断任务是否完成，不往它们的配置里写任何东西；SMTP 授权码交给 Windows 凭据管理器保管，不落盘到配置文件。

## 功能

- **通道隔离**：Codex 与 Claude Code 分别配置发件邮箱；通知记录分开保存；托盘菜单可单独开关任一通道。
- **凭据安全**：SMTP 授权码存入 Windows 凭据管理器；配置文件只留主机、端口、加密方式等非敏感项；测试成功后输入框自动清空。
- **邮箱预设**：QQ / Foxmail、网易 163 / 126 / yeah.net、Outlook / Hotmail / Live、Gmail 自动补全主机与端口；其它邮箱手填主机、端口、SSL 或 STARTTLS。
- **只读监听**：读取会话记录判断一轮结束，不修改 Codex 的 `config.toml` 或 Claude Code 的 `settings.json`；不建立对外监听端口。
- **事件过滤**：子代理中间事件、程序内部回执一律丢弃；一轮任务只发一封，不会因为同一次完成重复投递。
- **后台常驻**：关闭窗口后留在系统托盘；托盘菜单用勾选标出两个通道当前的开关状态，退出即彻底停止监听。

## 截图

**Codex 通道**

![Codex 通道](docs/codex.png)

**Claude Code 通道**

![Claude Code 通道](docs/claudecode.png)

## 安装

从 [Releases](https://github.com/ParatrooperY/AgentMailNotifier/releases) 下载：

- **安装包（Setup）** — 推荐。安装到固定路径。
- **便携版（Portable）** — 免安装，可随意放置。移动位置后需要重新启用一次通道。

首个版本没有做代码签名，Windows SmartScreen 可能拦一下，点**更多信息**再点**仍要运行**即可。

## 使用

**第一步，配置邮箱。** 填发件邮箱和授权码，点 SMTP 测试。收到测试邮件说明通了。

授权码不是邮箱登录密码，要去邮箱设置里单独开通 SMTP 服务后获取。QQ 邮箱在「设置 → 账号 → POP3/SMTP 服务」，网易在「设置 → POP3/SMTP/IMAP」。

**第二步，启用通知。** 分别打开 Codex 和 Claude Code 的开关。

### 支持的邮箱预设

| 邮箱 | SMTP 主机 | 端口 | 加密 |
| --- | --- | --- | --- |
| QQ / Foxmail | smtp.qq.com | 465 | SSL |
| 网易 163 / 126 / yeah.net | smtp.163.com | 465 | SSL |
| Outlook / Hotmail / Live | smtp.office365.com | 587 | STARTTLS |
| Gmail | smtp.gmail.com | 465 | SSL |

其它邮箱选自定义，填主机、端口和加密方式。

## 已知限制

- **只支持 Windows x64**，依赖 Windows 凭据管理器和本机会话记录路径。
- **被长度上限截断的回复不发通知**，判定「一轮结束」依赖会话记录里的收尾标记，正常收尾和被停止串收尾都算完成，但撞到 token 上限的半截回复会跳过。
- **程序没运行时的事件会丢失**，不做离线监听队列，避免重启后收到一堆过期通知。
- **移动安装路径后**，便携版需要重新启用一次通道。

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
src/                                    React 前端
src-tauri/src/                          Tauri 主进程、状态管理、SMTP 发送
src-tauri/crates/notifier-core/         事件解析、投递判定、SMTP 预设（纯逻辑，可单测）
docs/                                   应用截图
```

前端负责界面，`src-tauri/src/` 负责监听会话记录和发信，`notifier-core` 是不依赖 Tauri 的纯逻辑层，包括事件过滤和投递条件判定。

## 安全说明

- SMTP 授权码只存 Windows 凭据管理器，不写进配置文件、不进通知记录、不出现在界面上。测试成功后输入框会自动清空。
- 程序不修改 Codex 和 Claude Code 的配置文件，只读取会话记录。
- 不建立任何对外监听端口，不上传数据到第三方，邮件直接从本机发到配置的 SMTP 服务器。

## 反馈

问题反馈或有改进想法请以 [Issue](https://github.com/ParatrooperY/AgentMailNotifier/issues) 形式反馈，目前不接受PR。

## 许可

[MIT](LICENSE)
