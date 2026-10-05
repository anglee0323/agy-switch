# agy-switch

<img src="branding/app-icon.svg" width="88" height="88" alt="agy-switch 应用图标">

[English](README.md) · [下载](https://github.com/anglee0323/agy-switch/releases/latest) · [命令行指南](docs/cli.md)

在桌面应用或终端中管理 Antigravity 账号、查看剩余额度和本机用量。授权与额度查询直接连接 Google，账号数据和用量记录保存在本机。

## 选择使用方式

| 平台 | 桌面应用 | 快捷入口 | 命令行 |
| --- | --- | --- | --- |
| macOS · Apple Silicon | 应用程序包 | 原生菜单栏 | Homebrew 安装自带 `agy-switch` |
| Windows · x64 | 按用户安装 | 托盘看板 | 安装程序自带 `agy-switch.exe`，也可单独下载 |
| Linux · x64 | Debian 安装包，以 Ubuntu 22.04 为构建基线 | 桌面支持托盘时可用 | deb 自带 `agy-switch`，也可单独下载 |

桌面应用和命令行共用账号数据。Linux 同时提供桌面应用和无需显示环境的终端操作；目前 Linux 命令行仍依赖 GTK3/WebKitGTK 运行库，并非静态服务器程序。暂不提供 Intel Mac、Windows ARM 和 Linux ARM 安装包。

## 安装

**品牌更新开发中：** 4.9.0 源码已使用新名称和图标，尚未打包或发布。公开最新版仍为 4.8.1，应用和压缩包保留原来的 Tools Lite 名称。下方安装名称对应下一版；[历史下载](https://github.com/anglee0323/agy-switch/releases/tag/v4.8.1)及校验值保持原样。

从 **[GitHub Releases](https://github.com/anglee0323/agy-switch/releases/latest)** 下载对应平台的文件。桌面和命令行下载包附有 SHA-256 校验文件，更新专用包附有签名；`release-manifest.json` 记录全部附件及对应源码提交。

### macOS

```sh
brew tap anglee0323/agy-switch https://github.com/anglee0323/agy-switch.git
brew install --cask anglee0323/agy-switch/agy-switch
```

这会同时安装应用和 `agy-switch`。手动安装时，解压 `agy-switch-<版本>-macos-arm64.zip`，将应用移入 Applications。升级和验证结果见 [Homebrew 指南](docs/homebrew.md)。

### Windows

运行 `agy-switch-<版本>-windows-x64-setup.exe`。安装程序提供中文和英文，安装到当前用户目录，并在桌面程序旁附带命令行程序。

只使用终端时，解压 `agy-switch-<版本>-windows-x64.zip`，在该目录打开 PowerShell：

```powershell
.\agy-switch.exe
.\agy-switch.exe accounts list --json
```

PowerShell 会等待这个控制台程序执行，并保留退出码。安装位置、运行环境和故障排查见 [Windows 指南](docs/windows.md)。

### Linux

```sh
sudo apt install ./agy-switch-<版本>-linux-amd64.deb
agy-switch-desktop       # 桌面应用
agy-switch              # 终端看板
```

只使用终端时，可以下载 `agy-switch-<版本>-linux-amd64.tar.gz`。[Linux 指南](docs/linux.md)介绍运行库、无显示环境的安装、系统凭据服务及桌面兼容性。

**安装包信任：** 当前 Mac 安装包尚无 Developer ID 签名和公证，Windows 安装包尚无 Authenticode 签名，系统信任检查可能阻止运行。Homebrew 不会绕过这些检查。实际测到的 Mac 启动限制见[4.8.1 公开安装包验收记录](docs/4.8.1-public-acceptance.md)；文件完整性验证和系统信任验证是两项独立检查。

## 开始使用

1. 打开应用，通过 Google 授权、刷新令牌或本机导入添加账号。终端看板也支持授权和遮罩输入令牌。
2. 刷新额度，再选择要使用的账号。切换可能关闭并重新打开 Antigravity，请先保存工作，并在客户端确认账号。
3. 在首页或菜单栏、托盘中查看用量。如果需要自动选择备用账号，可在设置中启用智能切换。

`agy-switch` 是管理命令，不带参数运行可打开终端看板；Google 的 `agy` 用来执行 Antigravity 任务。切换账号会同步已初始化的原生 `agy` 会话；已经运行的任务可能仍保留之前的凭据。

## 功能

| 板块 | 功能 |
| --- | --- |
| 账号 | 额度与重置时间、列表和卡片视图、备注、启用或禁用、排序及批量操作 |
| 用量 | 每日和近期用量、输入输出与缓存构成、各模型费用估算及分布图 |
| 快捷看板 | 今日用量、整体剩余额度、各账号额度及明确的切换按钮 |
| 智能切换 | 优先级或轮询、拖动候选账号排序、额度阈值及活动检查，默认关闭 |
| 更新 | 检查版本、签名校验下载、安装进度及支持平台上的重启。[平台限制](docs/software-updates.md) |
| 外观 | 简体中文和英文、浅色和深色主题、模型选择及快捷看板偏好 |

三端快捷看板采用相同的三个信息区块。Mac 使用系统原生菜单，Windows 和 Linux 使用不透明的紧凑窗口。在设置中选择显示 Gemini、Claude/GPT 或两组系列，并调整账号名称、不可用账号的显示方式，以及重置时间的悬浮显示、始终显示或隐藏。额度条默认在大于 60% 时为绿色、20% 至 60% 时为黄色、低于 20% 时为红色；百分比使用正常文字颜色。

整体额度是有效账号数据的等权平均值，并显示可用账号数，不代表可相加的总令牌量。缺失或过期的额度会显示为未知。费用来自已知模型价格的估算，不是 Antigravity 账单；无法计价的模型会明确标识。智能切换不会迁移正在运行的任务，也不能保证所有任务已经结束。[切换机制](docs/low-quota-switching.md) · [看板数据说明](docs/menu-bar-dashboard.md)

## 终端工作流

```sh
agy-switch                       # 交互看板
agy-switch accounts list
agy-switch quota                 # 缓存额度
agy-switch stats                 # 本机用量和费用估算
agy-switch refresh               # 联网刷新额度
agy-switch switch user@example.com
agy-switch current --json         # agy-switch 保存的选择
```

上下方向键选择，回车或右方向键进入，Esc 或左方向键返回；数字快捷键也可使用。交互式账号管理支持备注、启用或禁用、确认删除及添加账号。脚本命令支持 JSON 和明确的退出码，只读命令使用本机缓存，不会启动桌面应用。

当前源码还新增了 CLI 策略设置、账号及候选排序和更新检查，暂未打包发布。桌面偏好、更新安装和后台策略执行仍由桌面应用提供。[命令、切换边界及退出码](docs/cli.md)

## 截图与验证

![Windows 用量看板](docs/screenshots/4.8.0/windows-dashboard-light.png)

Windows 原生 WebView2 用量看板。

![Linux 快捷看板](docs/screenshots/4.8.0/linux-quick-dashboard-light.png)

Linux 原生 WebKitGTK 快捷看板。两张截图来自记录完整的 4.8.0 CI 调试构建，使用合成示例数据，不含系统窗框。[截图来源](docs/screenshots/4.8.0/README.md)

原生窗口、终端和安装包分别验证。构建成功不等于已经验证真实授权切换、登录启动、所有显示器上的托盘定位，或所有 Linux 桌面环境。[各平台验收](docs/native-gui-acceptance.md)

## 数据与隐私

账号和偏好默认保存在 `~/.antigravity_tools`，可用 `ABV_DATA_DIR` 指定其他目录。导入的凭据属于敏感本地文件。用量来自 Antigravity 本机数据库和归档，应用不会上传对话内容。Google 授权、刷新令牌和额度查询需要联网；价格同步和可选的版本检查也会联网。

本项目不提供凭据中转或代理服务。切换会修改目标客户端使用的凭据位置，也可能报告部分更新；失败后请先确认客户端状态，再重试。[命令行行为及共享切换锁](docs/cli.md)

## 构建与贡献

使用 Node.js 22+、Rust stable，并安装平台指南中列出的依赖。

```sh
npm ci
npm run tauri dev
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
npx playwright test
```

Mac 使用 `npm run tauri build` 构建；Windows 使用 `./scripts/build-windows.ps1`；Linux 使用 `./scripts/build-linux-deb.sh --native` 或 `--docker`。发布流程生成桌面安装包和命令行下载包，验证校验值，只发布已打标签的 main 分支源码。[发布检查清单](docs/release-checklist.md)

基于 [lbjlaq/Antigravity-Manager](https://github.com/lbjlaq/Antigravity-Manager) 衍生。本分支聚焦本机账号、额度和用量，提供独立的命令行、设置和快捷看板。采用 [CC BY-NC-SA 4.0](LICENSE) 许可。
