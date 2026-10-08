# Antigravity experimental settings / Antigravity 实验功能

Open **Settings → Experimental Features** with the macOS standalone Antigravity App running. This first beta has been checked against Antigravity App 2.21.1. Windows, Linux and the Antigravity IDE are not supported by this settings integration yet; unavailable clients cannot be configured through these controls.

## Quick setup

The recommended preset makes Antigravity ready to use, and exposes the same options that are available in its native settings. **Shared preferences apply to both Antigravity App and Google's `agy` CLI**, because they use `~/.gemini/config/config.json`. Sleep prevention and background operation belong to the App. Codex, Claude Code and Pi Agent are unaffected. Google's `agy` is distinct from this application's `agy-switch` management command.

| Preference | Recommended value |
| --- | --- |
| Execution permissions | Turbo |
| Plan review | Always proceed |
| Outside-project file access | Allow |
| Web reading | Allow |
| Browser JavaScript | Always proceed |
| Follow-up messages | Send immediately, interrupting the current invocation |
| Conversation width | Wide |
| Intermediate thinking | Show |
| Extra AI Credits | Off |
| Prevent sleep / keep running after closing windows | On |

**Recommended / Custom** is a preset selector. Applying Recommended writes the defaults once. Editing a preference, including through Antigravity itself, changes the presentation to **Customized**. Selecting Custom keeps current values; it does not undo them. Selecting Recommended again reapplies the preset.

Writes use Antigravity's own partial-settings API and confirm the saved values afterward. Existing plugins, hooks, rules and permission-migration markers are preserved. Recommended setup retains existing permission lists and adds force-push confirmation and root-directory deletion denial. Partial failures are reported and current values are reread; setup is not claimed to be atomic across the App's two preference stores.

**Edit permission rules JSON** edits only the native `allow`, `ask`, and `deny` string lists. Examples of native rule syntax:

```json
{
  "allow": ["read_url(https://example.invalid/*)", "execute_url(https://example.invalid/*)"],
  "ask": ["command(git push *--force*)"],
  "deny": ["command(rm -rf /)"]
}
```

`read_url` controls reading a web page; `execute_url` controls browser actions. File rules use `read_file(...)` and `write_file(...)`. Rule targets and browser site lists are user input. The editor neither logs nor copies the rest of the client configuration.

## App translation

Translation applies only to the macOS standalone **Antigravity App**. Keep AntiGravity Switch running while it is enabled. It translates built-in sidebars, settings and nested permission pages, controls, hints, menus and known labels on history, automation and customization pages. Unknown new labels retain their original wording. Conversation/code bodies, user input, project names, plugin Readmes and embedded third-party pages are excluded. The operating system's native menu bar is not translated.

The integration uses the App's existing local debugging connection after checking its executable, process and listener ownership. It does not replace the App bundle or install a Hook. Disabling restores original text. A 15-second lease also restores it if AntiGravity Switch stops renewing the connection. New windows/pages are discovered while the feature runs.

UI scopes are identified through semantic controls and page structure rather than sidebar column positions. Synthetic fixtures cover alternate sidebar structures. Only 2.21.1 received live client acceptance: this does not promise complete translation on every older or future version. The dictionary includes MIT-licensed entries from [EasyAntigravity](https://github.com/DSDS-CMHL/EasyAntigravity); source attribution and license are bundled with the implementation.

## 简体中文

在 macOS 上打开 Antigravity App，然后进入 **设置 → 实验功能**。本版已在 Antigravity App 2.21.1 验证。

**懒人配置同时配置 Antigravity App 和 agy CLI**；防止休眠、关闭窗口后后台运行是 App 设置。Codex、Claude Code、Pi Agent 不受影响。推荐值见上表，全部都可以在 Antigravity 原生设置中修改。追问“立即发送”会中断当前执行，“排队”会等待当前轮次结束。

修改任意选项后仅显示“已自定义”。切到“自定义”保留当前值，再选“推荐配置”会重新应用默认值。权限列表可通过 JSON 手动编辑，网页读取用 `read_url(...)`，浏览器操作用 `execute_url(...)`。只更新选定字段，保留现有规则、插件、Hooks 和迁移标记；失败时显示实际状态。

**汉化仅作用于 Antigravity App**。内置侧边栏、设置及嵌套页面会随切页更新；关闭后恢复原文，使用期间需保持 AntiGravity Switch 开启。聊天、代码、项目名称、插件正文、第三方嵌入页面和系统菜单保持原文。新旧布局按控件与页面结构匹配；旧版仅有模拟布局验证，尚未逐版本实测。当前设置集成不支持 Windows、Linux 或 IDE。
