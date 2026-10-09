# Antigravity App translation / Antigravity App 汉化

Development builds expose only **Chinese interface** under **Settings → Experimental Features**. Quick setup, individual client preferences and the permission-rule JSON editor have been removed. Existing Antigravity preferences are retained; removing these controls does not reset them. The published 4.10.0 package still has its original features.

## Using translation

Open the standalone **Antigravity App** on Windows or macOS, then turn on Chinese interface. Keep AntiGravity Switch running while using it. Translation covers built-in sidebars, settings and nested pages, controls, hints, menus, and known labels on history, automation and customization pages. Unknown labels retain their original wording. Conversation/code bodies, user input, project names, plugin Readmes, embedded third-party pages and operating-system menus are excluded.

**Only Antigravity App is affected.** This feature does not configure the App or agy CLI, and does not translate Antigravity IDE, Codex, Claude Code or Pi Agent.

Turn off the switch to restore the original text. You can turn it off even after closing Antigravity. A 15-second lease also restores text when AntiGravity Switch stops renewing the connection. New windows and pages are discovered while it runs.

## Compatibility

The adapter uses the App's existing local debugging connection after checking its installed package identity, executable, process and complete listener ownership. It does not launch or restart the App, replace its bundle, change client preferences, or install a Hook.

- macOS: the standalone Apps 2.21.1 and 2.22.0 have received live acceptance.
- Windows: the native standalone App adapter follows the official 2.21.1 package layout, verifies the current Windows user and parent language-server process, and reads TCP ownership through Windows APIs. Windows WSL mode is currently unavailable because its server ownership cannot be verified by this adapter. Native Windows API and build checks are distinct from a live signed-in App test; a Windows App session has not been tested locally.
- Linux and Antigravity IDE: this integration is unavailable.

UI scopes use semantic controls and page structure rather than sidebar column positions. Synthetic fixtures cover alternate sidebar structures. Older/future versions are not promised complete translation. Unsupported or unverifiable connections remain unavailable. The dictionary includes MIT-licensed entries from [EasyAntigravity](https://github.com/DSDS-CMHL/EasyAntigravity); attribution and license are bundled with the implementation.

## 简体中文

开发版本的 **设置 → 实验功能** 只保留“界面汉化”。懒人配置、单项配置和权限规则 JSON 编辑已移除；此前修改的 Antigravity 设置会保留，不会自动恢复默认值。已发布的 4.10.0 安装包仍保留原有功能。

先打开 Windows 或 macOS 的独立 Antigravity App，再打开汉化开关。侧边栏、设置及内置嵌套页面会随切页翻译；关闭后恢复原文，使用期间需保持 AntiGravity Switch 开启。停止续期后，页面会在 15 秒内恢复。

**汉化仅对 Antigravity App 生效**，不调整 App 或 agy CLI 的配置，也不影响 Antigravity IDE、Codex、Claude Code 或 Pi Agent。聊天、代码、项目名称、插件正文、第三方嵌入页面和系统菜单保持原文。

Mac 2.21.1 和 2.22.0 已做实际 App 验证。Windows 原生模式按官方 2.21.1 安装包接入；Windows API 与构建检查不等于登录后的实际 App 验收，本机尚未做后者。Windows WSL 模式、Linux 和 IDE 暂不支持。旧版侧边栏仅有模拟布局验证，尚未逐版本实测。
