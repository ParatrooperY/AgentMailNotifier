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

## 开发

### 这个应用由两半组成

界面那一半是网页技术写的（React），跑在一个内嵌的浏览器窗口里。干活那一半是 Rust 写的，负责读会话记录、发邮件、管托盘图标。中间的框架叫 Tauri，它把这两半拼成一个 exe。

所以要装两套工具链：**Node.js** 管界面那半，**Rust** 管干活那半。

Windows 上还要装 **Visual Studio Desktop C++ 构建工具**。Rust 自己不带链接器（把编译好的碎片拼成 exe 的那个程序），在 Windows 上要借微软的。装 Visual Studio Installer 时勾「使用 C++ 的桌面开发」就行，不用装完整的 Visual Studio。

### 装依赖

```bash
npm install
```

这条只装界面那半的依赖，下载到 `node_modules/`。Rust 那半不用手动装 —— 第一次构建时 Cargo（Rust 的包管理器）会自己去下载，编译产物堆在 `src-tauri/target/`。这两个目录都很大，已经写进 `.gitignore` 不入库。

### 开发模式

```bash
npm run tauri dev
```

这一条命令背后做了三件事，顺序是固定的：

先按 `src-tauri/tauri.conf.json` 里的 `beforeDevCommand` 启动前端开发服务器（Vite），它把界面挂在 `http://localhost:1420`。

然后编译 Rust 那半，开一个桌面窗口，窗口内容指向刚才那个地址。

最后保持监听。改 `src/` 里的界面代码，窗口里立刻刷新，不用重启（专业叫法：热更新）。改 `src-tauri/` 里的 Rust 代码，它会重新编译再重开窗口，慢一些。

**第一次跑会很慢**，Rust 要从零编译几百个依赖包，十几分钟正常。之后有缓存，几秒到几十秒。

### 跑测试

两半各有各的测试，命令也是分开的。

界面部分：

```bash
npm test
```

Rust 部分：

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

`--manifest-path` 是告诉 Cargo「配置文件在这儿」。因为 Rust 代码在 `src-tauri/` 子目录里，不加这个参数就得先 `cd` 进去。

### 打包成安装包

```bash
npm run tauri build
```

这条比开发模式多了几步：

先跑 `tsc --noEmit` 检查类型有没有写错（`--noEmit` 意思是只检查、不产出文件），有错就停下，不会打出一个坏包。

再用 Vite 把界面编译成静态文件放进 `dist/`。

然后用 release 模式编译 Rust。跟开发模式的区别是开了优化，编译慢但跑起来快、体积小。

最后把界面文件和 Rust 程序打进一个 exe，再套一层 NSIS 安装程序。

产物在这两个位置：

`src-tauri/target/release/bundle/nsis/` —— 安装包

`src-tauri/target/release/agent-mail-notifier.exe` —— 免安装版，可以直接改名当便携版用

项目故意没有启用自动更新，发新版要手动把这两个文件传到 GitHub Releases。

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
