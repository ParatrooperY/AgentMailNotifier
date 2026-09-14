<h1 align="center">Agent Mail Notifier</h1>

<p align="center"><b>Codex Desktop / Claude Code Desktop 任务完成邮件通知</b>（Tauri 2 + Rust + React）</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green.svg" alt="License: MIT" /></a>
  <a href="https://github.com/ParatrooperY/AgentMailNotifier/releases"><img src="https://img.shields.io/github/v/release/ParatrooperY/AgentMailNotifier?include_prereleases" alt="Release" /></a>
  <img src="https://img.shields.io/badge/platform-Windows%20x64-lightgrey" alt="Platform" />
</p>

<p align="center"><a href="README.en.md">English</a> · <b>简体中文</b></p>

## 这是什么

Codex Desktop 和 Claude Code Desktop 跑完一轮任务后，给你发一封邮件。挂机跑长任务时不用一直守着终端，任务结束、或者中途失败需要你介入，邮件会告诉你。

程序只读取两个客户端的会话记录来判断任务是否完成，不往它们的配置里写任何东西；SMTP 授权码交给 Windows 凭据管理器保管，不落盘到配置文件。

## 功能

- **通道隔离**：Codex Desktop 与 Claude Code Desktop 分别配置发件邮箱；通知记录分开保存；托盘菜单可单独开关任一通道。
- **凭据安全**：SMTP 授权码存入 Windows 凭据管理器；配置文件只留主机、端口、加密方式等非敏感项；测试成功后输入框自动清空。
- **邮箱预设**：QQ / Foxmail、网易 163 / 126 / yeah.net、Outlook / Hotmail / Live、Gmail 自动补全主机与端口；其它邮箱手填主机、端口、SSL 或 STARTTLS。
- **只读监听**：读取会话记录判断一轮结束，不修改 Codex 的 `config.toml` 或 Claude Code 的 `settings.json`；不建立对外监听端口。
- **事件过滤**：子代理中间事件、程序内部回执一律丢弃；一轮任务只发一封，不会因为同一次完成重复投递。
- **后台常驻**：关闭窗口后留在系统托盘；托盘菜单用勾选标出两个通道当前的开关状态，退出即彻底停止监听。

## 截图

**Codex Desktop**

![Codex](docs/codex.png)

**Claude Code Desktop**

![Claude Code](docs/claudecode.png)

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

## 技术栈

界面是网页技术写的，跑在系统自带的 WebView2 里；读文件、发邮件、管托盘这些活由 Rust 做。Tauri 负责把两半拼成一个 exe。

| 位置 | 技术 | 作用 |
| --- | --- | --- |
| 界面 | React 18 + TypeScript 5.6 | 画界面 |
| 界面 | Vite 6 | 开发时实时编译，打包时输出静态文件 |
| 界面 | lucide-react | 图标 |
| 界面测试 | Vitest + Testing Library + jsdom | 假浏览器环境跑界面测试 |
| 后端 | Rust 2024 edition | 监听会话记录、发信、托盘 |
| 后端 | lettre | SMTP 发信 |
| 后端 | keyring | 授权码存入 Windows 凭据管理器 |
| 后端 | serde / serde_json | 读写 JSON 配置 |
| 后端 | chrono / uuid | 时间、记录编号 |
| 粘合 | Tauri 2 | 窗口、界面与 Rust 通信、托盘、打包 |
| 打包 | NSIS | Windows 安装程序 |

Rust 代码分两个目录：`src-tauri/src/` 是主程序，`src-tauri/crates/notifier-core/` 只放不碰系统的纯判断逻辑（事件该不该发信、邮箱对应哪个服务器），分出来是为了好测。

安装：

- **Node.js** —— 界面那半
- **Rust 工具链** —— 后端那半
- **Visual Studio Desktop C++ 构建工具** —— Rust 在 Windows 上要借微软的链接器，勾「使用 C++ 的桌面开发」即可，不用装完整 Visual Studio

### 依赖构建

```bash
npm install
```

这条只装界面那半的依赖，下载到 `node_modules/`。Rust 那半不用手动装，首次构建时 Cargo 自行下载，编译产物堆在 `src-tauri/target/`。两个目录都不入库。

### 开发

```bash
npm run tauri dev
```

启动前端开发服务器（Vite），通过 `http://localhost:1420` 访问，编译 Rust 打开桌面窗口。

改 `src/` 界面代码窗口即时刷新；改 `src-tauri/` 需重新编译并重开窗口。首次编译要下载并编译数百个 Rust 依赖，十几分钟正常，之后走缓存。

### 测试

界面部分：

```bash
npm test
```

Rust 部分，`--manifest-path` 指出配置文件位置，免得先 `cd` 进子目录：

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

### 构建安装包

```bash
npm run tauri build
```

执行流程：

- `tsc --noEmit` 检查类型有没有写错；
- Vite 把界面编译成静态文件放进 `dist/`；
- release 模式编译 Rust；
- 界面文件和 Rust 程序打包进 exe；
- 套一层 NSIS 安装程序；

产物位置：

`src-tauri/target/release/bundle/nsis/` —— 安装包

`src-tauri/target/release/agent-mail-notifier.exe` —— 免安装版

## 项目结构

```
src/                                    React 前端
src-tauri/src/                          Tauri 主进程、状态管理、SMTP 发送
src-tauri/crates/notifier-core/         事件解析、投递判定、SMTP 预设
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
