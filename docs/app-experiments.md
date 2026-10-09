# Antigravity App translation / Antigravity App 汉化

Development builds expose only **Chinese interface** under **Settings → Experimental Features**. Quick setup, individual client preferences and the permission-rule JSON editor have been removed. Existing Antigravity preferences are retained; removing these controls does not reset them. The published 4.10.0 package still has its original features.

## Using translation

Open the standalone **Antigravity App** on Windows or macOS, then turn on Chinese interface. Keep AntiGravity Switch running while using it. Translation covers built-in sidebars, settings and nested pages, controls, hints, menus, and known labels on history, automation and customization pages. Unknown labels retain their original wording. Conversation/code bodies, user input, project names, plugin Readmes, embedded third-party pages and operating-system menus are excluded.

**Only Antigravity App is affected.** This feature does not configure the App or agy CLI, and does not translate Antigravity IDE, Codex, Claude Code or Pi Agent.

**Installation and source files are not modified.** A translation script runs inside verified App pages through the existing local connection, observes UI changes and replaces known interface labels. It does not change permissions or accounts, or upload conversation content. Runtime injection still executes code in App pages and is not a guarantee of zero risk.

Turn off the switch to restore the original text. You can turn it off even after closing Antigravity. A 15-second lease also restores text when AntiGravity Switch stops renewing the connection. New windows and pages are discovered while it runs.

## CLI controls

Development builds also expose a primary **Experimental Features → Chinese interface** terminal menu, matching the desktop's **Settings → Experimental Features → Chinese interface** hierarchy. Both interfaces share `app_experiments.json` in Switch's data directory.

```sh
agy-switch experiments show --json
agy-switch experiments translation on
agy-switch experiments translation off
agy-switch experiments run
```

`show` only reads the saved switch; its JSON reports `runtime_checked: false`. `translation on|off` saves the switch without launching or connecting to the App. A running Switch desktop process observes changes on its next tick. For CLI-only use, turn it on and keep `experiments run` in the foreground; Ctrl+C stops that runner and returns to the terminal. It does not start a detached service. The switch remains saved; if no other Switch process renews it, the last lease restores the original text within 15 seconds. The terminal menu offers the same foreground runner. These commands control App translation; they do not translate Google's agy CLI.

## Platform compatibility

The adapter uses the App's existing local debugging connection after checking its installed package identity, executable, process and complete listener ownership. It does not launch or restart the App, replace its bundle, change client preferences, or install a Hook.

- macOS: the standalone Apps 2.21.1 and 2.22.0 have received live acceptance.
- CLI: synthetic executable/PTY checks cover the shared switch and Ctrl+C returning to the terminal menu. Live macOS acceptance can check coexistence when another Switch process is already renewing the App; it does not establish isolated lease expiry in that environment. Lease expiry has separate browser coverage.
- Windows: the native standalone App adapter follows the official 2.21.1 package layout, verifies the current Windows user and parent language-server process, and reads TCP ownership through Windows APIs. Windows WSL mode is currently unavailable because its server ownership cannot be verified by this adapter. Native Windows API and build checks are distinct from a live signed-in App test; a Windows App session has not been tested locally.
- Linux and Antigravity IDE: this integration is unavailable.

UI scopes use semantic controls and page structure rather than sidebar column positions. Synthetic fixtures cover alternate sidebar structures. Older/future versions are not promised complete translation. Unsupported or unverifiable connections remain unavailable. The dictionary includes MIT-licensed entries from [EasyAntigravity](https://github.com/DSDS-CMHL/EasyAntigravity); attribution and license are bundled with the implementation.

## 简体中文

开发版本的 **设置 → 实验功能** 只保留“界面汉化”。懒人配置、单项配置和权限规则 JSON 编辑已移除；此前修改的 Antigravity 设置会保留，不会自动恢复默认值。已发布的 4.10.0 安装包仍保留原有功能。

先打开 Windows 或 macOS 的独立 Antigravity App，再打开汉化开关。侧边栏、设置及内置嵌套页面会随切页翻译；关闭后恢复原文，使用期间需保持 AntiGravity Switch 开启。停止续期后，页面会在 15 秒内恢复。

**汉化仅对 Antigravity App 生效**，不调整 App 或 agy CLI 的配置，也不影响 Antigravity IDE、Codex、Claude Code 或 Pi Agent。聊天、代码、项目名称、插件正文、第三方嵌入页面和系统菜单保持原文。

**不修改 Antigravity 的安装文件或源文件**，通过 App 现有本机连接在页面中注入翻译脚本，观察界面变化并替换已知标签。不修改权限或账号设置，不上传聊天内容。运行时注入仍会在 App 页面中执行代码，因此不能承诺完全没有风险。

桌面入口为 **设置 → 实验功能 → 界面汉化**；终端主菜单入口为 **实验功能 → 界面汉化**。两端共用同一个开关。上面的 `show` 命令只读保存状态；`translation on|off` 只保存开关，不启动或连接 App。仅用 CLI 时，开启后运行 `agy-switch experiments run` 并保持前台运行，Ctrl+C 停止续期。它不会启动后台服务，也不会把保存的开关改为关闭；没有其他 Switch 进程续期时，最后一次续期的 15 秒租期到期后恢复原文。

Mac 2.21.1 和 2.22.0 已做实际 App 验证。Windows 原生模式按官方 2.21.1 安装包接入；Windows API 与构建检查不等于登录后的实际 App 验收，本机尚未做后者。Windows WSL 模式、Linux 和 IDE 暂不支持。旧版侧边栏仅有模拟布局验证，尚未逐版本实测。
