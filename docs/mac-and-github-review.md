# Mac 端与 GitHub 复查（2026-10-05）

## 当前交付范围

工作分支为 `feat/settings-category-sidebar`，对应 [草稿 PR #22](https://github.com/anglee0323/agy-switch/pull/22)。该分支已经包含多轮 APP、CLI、账号身份与菜单栏改动，PR 不再只是设置分类调整。旧描述中的六分类和实验汉化已过时；当前设置保留常规、配额、智能换号三类，实验汉化已删除。

菜单栏直接展示账号列表、各自 Gemini / Claude-GPT 的 5 小时与周额度，支持逐行查看和明确切换；顶部聚合范围可以在常规设置中选择。聚合是已知数据的等权平均剩余百分比，不能称为 Token 总量。共享池去重，未知、过期、禁用和受限数据不冒充可用额度。当前标记依据运行中独立 App 的已验证身份；核验失败不会任意选择第一个账号。

Mac 菜单栏改为 `NSMenu`，按照 CodexBar 的系统字体、细进度条和标准菜单项组织内容，背景、圆角、阴影、屏幕适配及收起都由 AppKit 管理。实机验收期间尝试直接替换根视图的玻璃效果触发了主线程 foreign exception / SIGABRT（23:53 的报告）；后续透明背景也不满足可读性。两种接入均已撤除，不再创建 Mac 网页面板或替换 Tao/Wry 视图。

复查还修复了智能换号设置自动保存的竞态：后端写入按用户编辑顺序排队，旧响应不能覆盖新草稿；失败后仍可继续保存。

## 验证与边界

- 前端 production build 成功；全套 Playwright 17 项通过（菜单栏 5、设置 9、智能换号 3）。后续 fixture 修正后，受影响的桌面偏好浏览器用例再次通过。
- Rust 全套库测试 110 通过、2 忽略；包含实际 Mac 菜单投影的 5 项新增验证和桌面模块 8 项。
- 额度展示 27、聚合 11、设置组件契约 10、桌面偏好 6、发布契约 8 项通过。
- 已安装 Mac 可执行文件的 CLI smoke 14 项通过：隔离合成数据、无 GUI、无网络、无真实凭据修改。CLI 启动隔离 6 项和 native harness self-test 通过。
- 真实只读检查：Tools 当前记录与运行中 App 均为账号二；账号列表 6 项，额度同时报告 5 小时与周窗口。禁用账号保持不可切换。
- 未在正在工作的官方 App 上触发真实登录切换/重启；该路径由已有后端测试和合成 UI 测试覆盖。登录系统启动、其他显示器及 Windows/Linux 原生 GUI 不属于本机已完成验收。
- 本地 bundle 为 4.7.8 arm64，签名完整性验证通过。未发布新 Release；Gatekeeper 与 notarization 未重新验收。
- 最终菜单实现提交为 `e977a854`。已替换桌面“最新打磨版”的 release 可执行文件；其 CLI 14 项再次通过。用户在真机点击菜单栏图标，确认能看到原生菜单；自动截图只能捕获主窗口，未把它冒充菜单的视觉证明。此次重启验收后未发现新的崩溃报告。完整换号、所有原生子菜单按钮与多显示器仍未逐项实机验收。

## 三组外部 Issue / PR

审阅固定于下列 head，三者目前均无 GitHub status checks。执行了源码比较和费用函数的隔离探针，没有运行外部安装器、切换真实账号、合并或发送审阅评论。结论均为修改后再合并。

### #11 / #12：环形轮换与任务完成提示

[Issue #11](https://github.com/anglee0323/agy-switch/issues/11) / [PR #12](https://github.com/anglee0323/agy-switch/pull/12)，head `c55b4a17720e68267b0391375b9e3c3788485379`。

**接入多端版本的适配限制。** `src/components/autoSwitch/AutoSwitch.tsx:66` 调用 `switchAccount(status.target_account_id!)`，没有传递目标程序。重新核对该 PR 固定 head，发现其 `Target` 枚举只有 `App`，因此默认 App 目标在原 PR 内一致；先前将其描述为原 PR 已经会切错 IDE 的 P1 故障不准确，现予更正。若移植到当前支持 App / AppCli / IDE / VS Code 的版本，需要明确传递目标，并重新验证 pending ID、配置和候选条件，不能直接照搬按钮调用。

环形搜索与优先级模式是可取的，但新增测试主要复刻遍历逻辑，应补生产 coordinator 的 A→B→C→A、无候选、禁用/受限、待切换期间配置变化验证。当前分支已更新目标枚举和 5 小时额度决策，不宜直接搬回旧版整套 coordinator。

**需求完成情况：部分完成。** 轮换策略与手动立即重启按钮已实现；任务/turn 完成检测、完成后的提示、可取消的 5 秒静默倒计时、全部耗尽后的最早重置倒计时没有实现。进程正在运行不等于正在生成，更不等于任务已经完成。

**与当前分支的区别。** 当前生产 coordinator 从 `candidate_account_ids` 列表头开始寻找第一个达标账号，属于优先顺序，并非环形轮换；#12 的 RoundRobin 在均衡使用候选账号上是可取的补充。当前分支已有多端目标和等待任务判断，但任务判断基于 transcript 文件最近活动与末行状态的启发式推断，不能视为可靠的任务完成信号。建议保留当前执行机制，按需要增加可选的环形候选顺序。

### #14 / #15：应用内更新

[Issue #14](https://github.com/anglee0323/agy-switch/issues/14) / [PR #15](https://github.com/anglee0323/agy-switch/pull/15)，head `5603f7bcebbd27f6a293190ad9322fee3e824d87`。

**P1：下载路径和执行来源未建立后端边界。** `src-tauri/src/modules/updater.rs:164–205` 接受前端任意 `download_url` 与 `asset_name`，后者直接拼到临时目录；绝对路径或 `../` 可以逃出该目录。下载后没有校验发行资产 hash/签名；Windows 的 243–254 行直接执行文件并退出应用。需要后端从可信仓库 Release 元数据选择资产、限制文件名、使用独占临时文件、校验长度/hash/签名，并对并发下载加锁。不能依赖正常 UI 只传合法名称。

**P2：架构不匹配时仍提供错误包。** Mac 的 74–81 行在指定架构缺失时回退到任意 macOS 压缩包；例如 Intel 主机可能下载 arm64 包。Windows/Linux 同样只靠后缀选第一个资产。应严格匹配发布 manifest 的平台与架构，缺失时明确显示无兼容安装包。

**P2：按钮承诺与平台行为不符。** 文案为“立即下载并重启更新”，Mac 的 260–269 行只在 Finder 中显示文件，不会安装或重启。这符合 Issue 正文给 Mac/Linux 的手动打开安装包方案，但应使用平台对应文案，避免把下载成功描述成已更新。

**需求完成情况：功能主体存在，安全验收不满足。** 启动检查已经排除菜单栏窗口；不能把它误报为每个窗口都检查。需要签名/校验失败、恶意文件名、架构缺失、并发写入及安装失败的测试后再引入。

### #17 / #18：按模型费用及环形图

[Issue #17](https://github.com/anglee0323/agy-switch/issues/17) / [PR #18](https://github.com/anglee0323/agy-switch/pull/18)，head `2958c2ae4780c7f60ceb804efcc1d8c5b768415c`。

**P2：新规则把 Claude 不同系列按 Sonnet 价格计算。** `src/pages/Dashboard.tsx:120` 将原本只匹配 Sonnet 4.6 的规则扩展到 Sonnet/Opus/Haiku，仍统一为输入 $3、输出 $15、缓存读取 $0.30 / MTok。提取该 PR 的实际函数执行，Opus 4.6 与 Haiku 4.5 都命中此规则。[Anthropic 官方价格](https://platform.claude.com/docs/en/about-claude/pricing)分别为 Opus 4.6 的 $5/$25/$0.50 与 Haiku 4.5 的 $1/$5/$0.10；成本金额和饼图占比会同时出错。应按精确模型 ID、价格来源与有效日期维护规则，未知模型保留未知。

**P2：缺失价格被图表呈现为零费用。** 825–857 行过滤 `costUsd > 0`；有真实用量但所有模型都未定价时，图中央显示 `$0.00 / 无计费`，列表显示“暂无费用记录”。总费用卡悬停已有未定价提示，但图表本身仍容易误导。需要区分无用量、真实零费用、未定价与部分估算，明确百分比分母只是已定价费用。

**需求完成情况：图表、悬停、费用列基本齐全，价格准确性不满足。** 可在保留现有首页布局的前提下移植其费用明细功能，先统一 KPI/表格/饼图的计算并验证各模型价格、未知和混合数据。后端的 input_tokens 与 cached_tokens 是分开的，此处没有重复计算缓存的证据。

## origin 分支清单

清理前共 16 个远程分支（不含符号 HEAD）。2026-10-05 按用户明确要求，已删除其中对应已合并 PR 的 13 个远程分支；清理后仅保留主线、当前工作分支和关闭未合并的 PR #2 分支。

删除前逐个核验分支 head 与已合并 PR 的 head 一致，且其 merge commit 已进入 main；删除使用原子推送及精确提交号检查，避免误删核验期间发生新增提交的分支。删除后通过 GitHub API 再次确认剩余分支列表。本地主线、工作分支、代码、Release、Issue 和 PR 历史均未删除，三个外部 PR 仍开放。

| 分支 | 关联状态 | 建议 |
| --- | --- | --- |
| `main` | 主线 | 保留 |
| `feat/settings-category-sidebar` | PR #22，当前工作 | 保留；更新描述，等新 CI |
| `build/reproducible-cloud-linux` | PR #2，关闭未合并 | 保留待确认；有独立历史提交 |
| `codex/unify-native-agy-sync` | PR #3，已合并 | 已清理远程分支 |
| `feat/app-localization-opt-in` | PR #4，已合并，功能已撤除 | 已清理远程分支 |
| `codex/cli-homebrew` | PR #5，已合并 | 已清理远程分支 |
| `feat/menu-bar-dashboard` | PR #6，已合并 | 已清理远程分支 |
| `feat/safe-quota-switch` | PR #7，已合并 | 已清理远程分支 |
| `codex/windows-release-cli-validation` | PR #8，已合并 | 已清理远程分支 |
| `test/native-gui-acceptance` | PR #9，已合并 | 已清理远程分支 |
| `codex/release-4.7.7-preparation` | PR #10，已合并 | 已清理远程分支 |
| `codex/homebrew-4.7.7` | PR #13，已合并 | 已清理远程分支 |
| `fix/macos-bundle-signature` | PR #16，已合并 | 已清理远程分支 |
| `release/4.7.8-preparation` | PR #19，已合并 | 已清理远程分支 |
| `chore/homebrew-4.7.8` | PR #20，已合并 | 已清理远程分支 |
| `fix/account-dashboard-data-contract` | PR #21，已合并 | 已清理远程分支 |

最近几个修复分支经过 squash 合并，旧 head 不是 main 祖先不代表功能漏合并。三个外部 PR 来自贡献者分支，并不在这 16 个 origin 分支中。

## 自己的 PR #22

旧 CI 的 Mac 发布契约仍查找已删除的汉化文案，Linux UI 测试仍按六分类/手动保存验收；已更新到当前产品行为。旧失败不能作为当前提交通过的证据。PR 保持草稿，新的跨平台 CI 结果单独查看；本机 Mac 通过也不自动证明 Windows/Linux 原生 GUI。

`832b9a5e` 的 Mac、Windows 构建及三端 release CLI / 打包通过，Ubuntu build 的浏览器用例有 8 项失败：7 项旧菜单契约和 1 项聚合设置控件缺失。其对应的新控件、紧凑面板与浏览器用例现已一起提交到 `e977a854`，本地全套 17 项通过；应以新 head 的 CI 结果验收，不能沿用上一提交的成功项。

## 后续功能顺序（用户 2026-10-05 意向）

- #18 可以考虑接入现有首页；先修正模型定价与未知费用状态，让总费用、明细与环形图使用同一份计算结果。
- #15 延后到其他功能收尾之后处理。
- #12 尚未决定引入；当前需要区分候选账号顺序和换号执行机制，不能笼统认定现有方案全面优于环形轮换。

## 菜单栏显示打磨（2026-10-05）

- 顶部改为 AntiGravity tool lite 和应用图标；整体额度右侧先显示可用账号，再显示剩余比例。
- 总览聚合范围与账号区 Gemini / 非 Gemini / 全部按钮独立保存。默认邮箱在前、备注在后，切换/当前按钮前置且使用原生边框；失效、禁用账号默认隐藏。
- 常规偏好增加显示系列、名称格式、账号隐藏、窗口/整体额度/图标显示、按钮位置和颜色阈值。默认 >60% 绿、20–60% 黄、<20% 红；不允许隐藏全部额度窗口或保存反向颜色阈值。部分更新不会覆盖其他菜单偏好，旧常规设置写入也保留新偏好。
- 悬浮详情从禁用灰色菜单项改为正常系统文字的分组视图，使用模型图标、彩色百分比和短重置倒计时，去掉重复的进度条百分比 tooltip。About 和菜单提供仓库链接。
- 验证：生产前端构建、arm64 release 构建通过；Rust 116 通过、2 个辅助用例忽略；浏览器界面 19 通过；额度聚合/颜色 12、既有菜单逻辑 27、设置契约 10、桌面设置竞态 6 通过。
- 桌面“最新打磨版”已替换并重新启动，签名完整性检查通过，Mach-O UUID 与本次 release 相同；安装后的 CLI 14 项通过。实际设置页显示全部新控件，图标开关保存/恢复已验证；保留用户当前 Gemini 聚合范围。没有出现新的 Tools 崩溃报告。
- 自动截图仍只捕获主窗口，无法捕获 AppKit 弹出菜单。因此上述浏览器与主窗口验证不作为原生菜单/悬浮详情视觉验收的证据；最终视觉效果需在实际菜单上查看。

### 菜单栏文案与位置修订（用户截图反馈）

百分比改为整数，不再显示 <1% / >99%；统计仍保留原始精度。删除品牌下重复的“额度总览”，聚合标题改为“总览”，系列/平均剩余靠右，数量改为“5 个账号”这类普通文本。默认切换按钮置于右侧，保留用户已选择的右侧位置与显示系列。删除菜单栏 About 入口，保留 GitHub；用量看板、管理账号、设置去掉省略号，底部入口使用原生 SF Symbols。前端构建、Rust 116 项、菜单栏/设置浏览器 16 项、聚合/颜色 12 项通过。

最新补充：按钮固定右侧，删除按钮位置设置和配置字段；默认显示全部系列，设置保留系列选择。账号改为轻边框/背景分块，邮箱与备注上下两行。原生系列按钮在当前 tracking session 中修改已存在的额度条/文字视图的可见性与宽度，不再关闭或重建 NSMenu；保存失败保留原选择，并在当前菜单显示错误。最新前端构建、Rust 116 项、菜单栏/设置界面 16 项、聚合/颜色 12 项均通过。

包含最新补充的 arm64 release 已部署至桌面“最新打磨版”并启动，签名完整性通过，安装后 CLI 14 项通过。原生菜单连续系列切换的视觉验收已向用户请求确认；自动截图仍不能捕获浮层，不把浏览器用例当作这项验收。

## 2026-10-05 本轮显示一致性修订

菜单账号去掉圆角外框和底色，使用留白与细分隔线。中等额度改为低饱和度琥珀色条，数值使用系统正文色；显示禁用账号时，名称、备注、禁用状态和不可用额度行统一红色，仍不能切换。名称选项使用“邮箱优先／备注优先／仅邮箱”；系列名称明确区分 Gemini 与 Claude/GPT，英文使用 Claude & GPT，保留平均剩余统计口径，取消圆点和顿号分隔。

App 方块视图的周额度移除独立卡片渲染，条视图/方块视图的 5 小时/周额度四处均复用 QuotaItem，统一高度、字体、圆角、进度背景和倒计时布局。真机打开四种组合检查，并恢复原来的方块周视图；设置页已核对安装后的系列与名称选项。保留用户当下设置（包括绿色阈值 70 和显示禁用账号）。

生产前端和最终 arm64 release 构建通过；Rust 116 通过、2 忽略；最新菜单/设置浏览器测试 16 通过，增加显示禁用账号仍不可切换且额度为未知的断言。桌面最新版已更新并启动，签名完整性通过，安装文件 UUID 与最终 release 一致，CLI 14 项通过，未见新的 Tools 崩溃报告。第一次原生测试编译遇到 E0308（条件表达式中的 Retained<NSColor> 引用推断），改为显式局部颜色对象后测试和发布构建通过。自动截图仍只捕获主窗口，未宣称本轮原生浮层视觉和连续切换已经完成自动验收。

### 本轮用户截图后的修正

用户澄清标红只针对额度区域：禁用账号的名称、邮箱、周期标签和状态恢复普通文本样式；主列表和详情展示可用额度 0%，保留红色空额度轨道。所有百分比统一使用系统正文色，黄色额度条恢复 systemYellowColor，去掉棕色；浅色菜单为黑色数字，深色随系统保持可读。禁用展示值仅在视图层覆盖，不写回缓存、不加入总览平均，切换仍禁用。删除账号行和详情时间的 tooltip，保留唯一的账号详情浮层。

菜单内 Gemini/Claude-GPT/全部按钮及菜单内保存/切换机制已移除；“账号区显示系列”只在设置中选择，菜单按保存值渲染。浏览器测试通过真实设置入口选系列，验证菜单不存在选择按钮、选择保留、禁用主列表/详情均为 0%、统计和禁止切换保持一致。

生产前端、最终 arm64 release 构建通过；Rust 116 通过（2 忽略），最新菜单/设置浏览器 16 通过。桌面最新版已替换、签名完整性通过、UUID 与最终构建一致、CLI 14 项通过，并实际启动与核对设置入口；未出现新的 Tools 崩溃报告。为纳入用户最后的文字颜色要求，中途主动停止尚未完成的旧 release 构建并重新构建最终版。未把自动截图无法捕获的原生菜单外观声称为自动视觉验收。

### 看板标题与账号状态视觉修订

菜单栏“总览”改为“额度概览”，“6 个账号”改为左侧“账号额度”、右侧账号总数。菜单账号额度的百分比改为右对齐。禁用操作位改为原生带 `nosign` 图标的禁用小按钮，红色额度轨道使用系统红色不透明填充，0% 保持黑色系统正文。

App 条视图和方块视图的当前账号勾选标记均移至邮箱文字后，保留蓝色原色图标。方块视图的禁用提示保留原有禁用图标和文字。前端生产构建、Mac arm64 发布构建和已安装 App CLI 14 项检查通过；安装二进制 UUID 与最终发布文件一致。按本轮执行约束未运行测试套件。

### 同一菜单内查看账号详情

取消原生账号子菜单，点击账号名称后在当前 NSMenu 内显示已预建的详情条目，通过“返回”恢复概览；品牌标题保留，不关闭重开菜单，不产生旁边独立浮层。切换/当前/禁用操作位统一为 84×26；当前使用系统蓝色圆圈对勾，禁用使用系统红色禁用图标和文字，状态位只读，避免禁用控件的灰化。主列表和详情仍展示禁用可用额度 0%，身份与周期标签保持普通样式。原生和共享面板的有效数据覆盖 tooltip、额度列重复 tooltip 均移除。

生产前端、Mac release 构建通过；Rust 116 通过、2 忽略；菜单/设置浏览器 16 通过。桌面最新版已更新并启动，签名完整性通过，安装 UUID 与最终 release 一致，安装后 CLI 14 通过，没有新的 Tools 崩溃报告。原生菜单快捷键已触发，但自动 AX 仍只返回主窗口，未将浏览器测试当作原生详情导航/外观验收。

### 三块布局与截图反馈修正

菜单改为“今日用量 / 剩余额度 / 账号列表”三块。今日用量使用输入、输出、缓存三段环形图，显示总 Token、请求次数和 API 费用估算；统计复用首页原生本机记录，不归因到某个账号。只读取已有官方价格缓存，打开菜单不联网抓取价格，Native 的用量读取最多等 2 秒。未知或歧义模型不猜价格、不显示成免费，部分计价和缓存价格标明状态；读取失败保留不可用显示。

账号名称恢复普通标签，透明点击区域打开同菜单详情，不显示右箭头。补偿 NSTextField 的左右文本留白，让标题、账号名、邮箱与周期标签共用左边界，百分比、数量与按钮共用右边界；操作位恢复中文 64×26、英文 72×26，状态内容居中，“已禁用”缩为“禁用”。此前箭头和名称按钮带来的灰色及偏移已移除。

初次界面检查暴露新图表挤压短窗口账号区，已调整顶部与底部占用并按实际账号区高度分页。最终浏览器 17 项通过，含 320×400 下无裁切、未知费用不当作零费用、空记录显示零和禁用不激活；Rust 119 项通过、2 忽略，新增估算检查覆盖原生模型别名、未知/歧义价格和部分计价。前端生产构建通过。

最终 arm64 release 已部署至桌面最新版并实际启动；签名完整性通过，安装文件 UUID 与最终 release 一致，安装后 CLI 14 项通过，未出现新的 Tools 崩溃报告。原生菜单仍无法被 AX/自动截图读取，未宣称最终原生浮层视觉与点击返回已自动验收；共享面板截图只用于验证共享布局。

### 菜单响应速度、悬浮与英文一致性

打开菜单不再等待本机用量扫描或运行中账号核验。先读取小型账号 DTO、配置和当日内存快照，再在当前 tracking session 中更新用量与当前账号状态；核验完成前不将保存的账号标成已验证身份。用量读取合并并发请求、30 秒内复用结果，启动预热，首页扫描完成后同步更新缓存；跨本地午夜不复用前一天。根据用户最新反馈，原生菜单和共享面板均移除账号详情页与名称点击入口，额度及重置时间只在列表展示，右侧切换操作保留。减少菜单打开时创建的视图和操作目标。

悬浮时保留进度条，缩短条宽给时钟和简短倒计时让位；百分比右边界、账号行高度和菜单宽度固定，移开后恢复。两列额度之间保留 12 点空隙，第一列百分比不再紧挨第二列进度条，整行左右边界保持对齐。设置提供“悬浮显示／始终显示／不显示”三种重置时间模式，默认悬浮；始终显示固定保留倒计时和缩短的进度条，不显示恢复全宽。旧开关配置兼容、部分写入和普通设置过期写入保护均覆盖。切换按钮使用原生轻微高亮；成功切换不再留下提示行，失败信息在下一次打开时显示一次。顶部环形图使用清蓝、柔黄和薄荷绿，图例用对应的短色条，文字和数字保留普通系统颜色；删除重复 USD 标注。

英文模型选择的全选/取消全选和共享池说明改用当前界面语言。App 周额度的 Shared Pool 保留为普通标签文字，取消独立蓝色胶囊；所有有重置时间的 QuotaItem 统一使用与 5 小时相同的黄色时间，未知时间仍保持 N/A。安装后逐一查看方块 5 小时、方块周额度、条视图周额度、条视图 5 小时，标签和倒计时样式一致。

最终前端生产构建和 Mac arm64 release 构建通过；Rust 121 通过、2 个辅助用例忽略；菜单栏/设置浏览器 19 通过，覆盖无账号详情点击入口、禁用 0%、双列间距与重置时间三种模式；安装后 CLI 14 通过。桌面“最新打磨版”已替换并启动，安装和 release UUID 均为 `BF7CE44A-0198-3E37-B3CE-6FB4149BF4A4`，没有新增 Tools 崩溃报告。初次安装签名失败，报错 `resource fork, Finder information, or similar detritus not allowed`；清理 FinderInfo 后重新签名成功。桌面 FileProvider 会重新附加 FinderInfo，严格属性校验仍报同一错误；普通签名完整性、资源封印、架构和空 entitlements 检查通过，未宣称严格校验或 Gatekeeper/公证验收通过。最终安装版 6 个账号的原生菜单准备时间为首次 89、再次 19 毫秒，运行中身份分别在 322、120 毫秒应用到现有状态位；这些是准备/核验计时，不是屏幕动画计时。真机设置页已读到新的重置时间选项，未改动用户最新选择。自动捕获依然读不到原生菜单浮层，浏览器的悬浮几何断言不作为原生外观验收；Windows/Linux 原生 GUI 也没有在本机验收。

## Integrated PR review — 2026-10-05

The three contributor heads were merged into the Mac polish branch, preserving their history. Main integration is gated on the final source's native Mac acceptance and CI.

- **#18**, `2958c2ae`: homepage model cost column, breakdown and token/cost chart toggle. Replaced substring matching and shared Claude-family fallback prices with exact cached-price matching, consistent with the native menu. Unpriced models remain visible and excluded from the estimated subtotal; unknown costs are not zero. The English cost-detail label is localized.
- **#12**, `c55b4a17`: optional round-robin selection. Existing configurations keep priority order. Added pointer/keyboard drag handles to selected candidate rows and persisted their order through the existing serialized auto-save path. Production coordinator fixtures cover wraparound, priority, absent source and disabled/forbidden candidates. Existing identity, process, pending-request and fresh-quota revalidation remain in place. The original direct restart shortcut was not transplanted over the evolved multi-client coordinator.
- **#15**, `5603f7bc`: background startup check, manual Settings check and localized update dialog. Public metadata must be a stable, newer version with a release URL in this repository. Users can turn startup checks off or dismiss a version; Settings retains the update hint. The integration opens the official release page and does not expose the original arbitrary download/installer command. Download installation and restart are not advertised as implemented.

Verification so far: frontend production build; 27 Chromium UI tests using synthetic IPC; 125 Rust tests with two isolated helper tests ignored; 14 real executable CLI smoke checks. Actual Mac PTY acceptance confirmed arrow navigation, numeric section selection, masked synthetic credential input, Escape cancellation, back and clean exit, without authentication or switching. A standalone `script` harness failed and was kept under ignored audit artifacts; it is not counted as passing acceptance.

The CLI audit removed invented 100% windows and default-model cost estimates. Human statistics now use the cached pricing table and actual per-model records for each range. Unknown prices and partial estimates are explicit. Pricing-cache lookup honours the isolated data-directory override without creating directories during read commands. The Windows CLI currently supports one-line commands; its interactive menu is not yet accepted on Windows. Full visual feature parity is not required: account actions and configuration should share backend operations across GUI, terminal and script commands.

CI regression fixes update the Settings component double for the new update section and the native Linux test to the current `autoSwitch` category and panel-scoped checkbox. These fixture repairs do not establish installed Linux/Windows GUI acceptance. New model-pricing and updater guard tests run in CI.

The earlier 4.7.9 candidate retained `agy-lite` compatibility alongside `agy-switch` (removed below at the user's request), updates the desktop bundle version before signing, and replaces switching copy that guaranteed task completion/context preservation with the actual activity-detection limits. The native Linux form check uses its stable control ID rather than an obsolete English label. Real Brew v4.7.8 installation succeeded with a custom app directory; Gatekeeper assessment rejected it and the old public CLI invocation timed out. Neither is reported as successful launch acceptance.

Final Mac candidate verification: production frontend and arm64 release build passed, 125 Rust tests passed (two helper cases ignored), 27 browser UI checks passed, and the two cost/localization cases were rerun after wording corrections. The installed desktop bundle reports 4.7.9, verifies signature integrity and empty entitlements, matches release UUID `786120CB-E8F5-35BB-AFC1-DAE2F981CD6A`, and passes 15 read-only CLI executable checks including both aliases. Actual App review confirmed the Chinese and English homepage cost views, Settings update section, switching strategy and drag handles. Language was restored to Chinese; candidate order and switching preferences were not changed. Native menu open/close was exercised through its shortcut, but AX still cannot capture its popup; no visual-acceptance claim is inferred from that.

The user requested no legacy Lite compatibility. The final 4.7.9 source and generated cask retain only `agy-switch`; the earlier alias acceptance records describe the superseded candidate. The final executable smoke suite has 14 checks.

Homebrew CI previously used the source package version to regenerate an older published cask, preventing a version bump from being merged before publication. It now validates the published cask version, keeping real archive/checksum comparison intact. The release workflow separately verifies the new source version and generates the new cask from its actual package.

CLI review also found remaining account-table, refresh and status labels describing the recorded selection as an active external identity. All now use Selected/当前选择, and the status section calls it a local record. Read-only CLI identity semantics remain unchanged.

The final Linux native run exposed a route-mount race: the harness clicked Settings then queried its category immediately. Earlier runs passed by timing. Native clicks now wait for a visible, enabled element before using WebDriver, while retaining all screenshot/IPC assertions and the 30-second failure bound. This is a harness readiness fix, not a skipped GUI check.

## Main integration and public 4.7.9

PR #22 merged at `4876f7e7`; contributor PRs #12, #15 and #18 are merged with original history retained. The final installed local Mac app is version 4.7.9, arm64, release UUID `9FE410C8-DD42-3CA4-9C46-D32298865A5B`, with 14 executable CLI checks and final real PTY navigation/masked-input cancellation. Main first-party Chinese/English content was reviewed; OS-owned application menus retain system-provided labels. No complete OS-menu localization claim is made.

The final Linux native CI passed all six captures after the route-readiness fix; Windows GUI remained blocked. Public v4.7.9 assets and their generated Brew cask match all downloaded hashes. Actual Brew upgrade/reinstall preserved twelve existing JSON files and removed the legacy alias. Quarantined Brew CLI launch timed out and Gatekeeper rejected it, separately from valid strict signature integrity in Applications and 14 passing public-ZIP CLI checks. [Public acceptance](release-notes/4.7.9-public-acceptance.md) records the draft publication retry and platform limits.
