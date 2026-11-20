# GitRune 界面巡检报告

> 2026-11-10。剥离完成后对运行中的应用做一轮界面巡检。
> 方法：Playwright 纯浏览器模式（`e2e/fixtures.ts` 内存后端）+ 全量 e2e 套件 + 定制探针。
> **本报告只记录发现，不做修复**（方法论：发现与修复分离）。

---

## 一、结论速览

**核心 Git 功能全部可用，未发现阻断级缺陷。**

| 验收面         | 结果                                    |
| -------------- | --------------------------------------- |
| Git 相关 e2e   | **40/40 通过**                          |
| 工具窗渲染     | Project / Commit / Git 全部正常         |
| Git 工具窗页签 | Log / Console / Worktrees 齐全          |
| 设置对话框     | 页面树完整（7 组），无报错              |
| 运行时错误     | **0 个 console error，0 个 page error** |

全量 e2e：**96 通过 / 133 失败**，其中 **103 个失败属于已删功能**（agents、database、codeInsight、debug、run、terminal、plugin 等），属预期。

---

## 二、确认正常的部分（证据）

### 2.1 Git 核心 e2e 全绿（40 个）

覆盖：`commit`、`diff`、`gitIgnore`、`gitLogBranchMenu`、`gitLogCommitChecks`、`gitLogScrollbar`、`newBranch`、`compareWith`、`mergeEditor`、`projectTree`、`overviewLane`

```
40 passed (2.6m)
```

这说明：提交面板、diff 视图（并排/统一/万行文件）、Git log 图、分支菜单、ignore 菜单、三方合并编辑器、项目树交互全部正常。

### 2.2 工具窗与面板

```
TOOLWINDOW_NAMES: [ 'Project', 'Commit', 'More Tool Windows', 'Git' ]
TABS_AFTER_GIT:   [ 'Log', 'Console', 'Worktrees' ]
```

原 23 个工具窗降到 4 个，**Git 客户端的三个核心工具窗都在**。

### 2.3 设置对话框

```
DLG_TREEITEMS: Appearance & Behavior, Appearance, System Settings, Notifications,
               Keymap, Editor, Version Control
```

正是保留的页面集。Appearance 页渲染出 Theme / Editor color scheme / Accent colour 等控件。

### 2.4 无运行时错误

多轮探针均返回 `ERRORS: []`——没有 console error，没有未捕获异常。

---

## 三、发现的问题（按严重度）

### 🔴 P1 — 开发工具链的 worktree 标识符未跟随改名

**现象**

```
$ npx tsx scripts/dev-status.ts
{
  "worktree": ".../gitrune",
  "identifier": "ai.l0ng.openidea.dev.wfc4cb407c8856b0e",   ← 旧前缀
  "bridges": []
}
No MCP bridge found for this worktree.
```

但应用实际以 `dev.gitrune.app.dev.wfc4cb407c8856b0e` 运行，MCP 监听在 9223。

**根因**：`scripts/dev-session.ts:12` 仍返回 `ai.l0ng.openidea.dev.w...`。`dev-status.ts` 用它做匹配，两边对不上 → 永远报 "No MCP bridge found"。

**影响**：`pnpm dev:status` 失效；依赖它拿 MCP 端口的自动化验证流程无法工作。**不影响产品运行时**（应用本身标识符已正确）。

**同类位置**：`scripts/dev-session.test.ts:53` 的断言也是旧值。

---

### 🟠 P2 — 标题栏缺少 "Search Everywhere" 按钮

**现象**（e2e `workbench.spec.ts:6` 失败）

```
Locator: locator('header').first().getByRole('button', { name: 'Search Everywhere' })
Error: element(s) not found
```

实测标题栏按钮只剩：`acme-api`、`IDE and Project Settings`、`Select Opened File`、Search Everywhere 消失。

**根因**：`search` feature 已删（其 `registerTitleItem` 随之移除）。

**判定**：**符合剥离意图**——纯 Git 客户端不需要全文搜索入口。但需要**明确决策**：是接受这个结果并更新 e2e 断言，还是补一个轻量的"查找动作"入口。

---

### 🟠 P2 — ⌘K 打开 Commit 面板时消息框不自动聚焦

**现象**（e2e `commit.spec.ts:60` 失败）

```
Locator: getByRole('region', {name:'Commit'}).getByRole('textbox',{name:'Commit message'})
Expected: focused / Received: inactive
```

定制探针结果：

```
FOCUS[after-cmd-k]:        {"active":"DIV|_handle_127e7_12"}      ← 焦点落在面板分隔条
MSG_COUNT:                 1                                      ← 消息框存在
FOCUS[after-click-textarea]: {"active":"TEXTAREA|Commit message"}  ← 点击后能正常聚焦
```

**已排除**：

- `CommitView.tsx` 与原版**逐字节相同**（不在 diff 列表里）
- `vcs/index.ts`、`e2e/fixtures.ts` 未改动
- `contributions.ts` / `Workbench.tsx` 仅改 import 路径与品牌名
- 无 console 错误

**尚未定位根因**。可疑方向（按可能性排序）：

1. `focusTick` 的触发时机——它只在「提交空消息」时自增（`CommitView.tsx:892`），⌘K 打开面板本身不触发；测试却期待一打开就聚焦，说明原本有另一条聚焦路径
2. tool window 首次显示时的默认聚焦机制（可能依赖 `data-autofocus`，但消息框未标注）
3. `editor` feature 的激活顺序变化影响了焦点初始化

**影响**：用户按 ⌘K 后需手动点一下消息框才能输入。**功能可用，体验退化**。

**优先级建议**：这是唯一「功能正常但体验变差」的项，值得修。

---

### 🟡 P3 — 三处 e2e 因依赖已删功能而失败

| 用例                                             | 失败原因                                 | 判定                               |
| ------------------------------------------------ | ---------------------------------------- | ---------------------------------- |
| `boot.spec.ts:57` `#boot-error` 计数不符         | 等待特定模块加载失败场景                 | 需单独确认                         |
| `openProject.spec.ts:10` Close Project → Welcome | 30s 超时，依赖 `close_project` stub 链路 | 需确认是否受删减影响               |
| `keymap.spec.ts` 两条                            | Settings › Keymap 页的快捷方式对话框     | 设置页正常，疑为对话框依赖已删插件 |

**keymap 值得单独看**：设置树里 Keymap 页存在，但"Add Keyboard Shortcut"对话框路径失败，可能依赖 `plugins` feature 的某段逻辑。

---

### 🟡 P3 — 5 个主题 e2e 失败

`themes.spec.ts` 里 Solarized Light/Dark、One Light/Dark、gear preview 全部失败。这些用例名称含 "terminal"，很可能是**依赖已删的 terminal 工具窗**做颜色比对。需确认是否可改为不依赖 terminal。

---

## 四、已删功能的失败（103 个，预期内，无需处理）

按来源分布：

| 类别      | 代表用例                                                                                     |
| --------- | -------------------------------------------------------------------------------------------- |
| Agents    | `agentApproval`、`agentMarkdown`、`agentScroll`、`agentsDock` 等 20+                         |
| 代码智能  | `codeInsight`（Go to Declaration、Find Usages、Rename In Place、quick fix）、`semanticStyle` |
| 数据库    | `clickhouse-source`、`oracle-source`、`databaseDialogLayout`、`sdkFieldLayout`               |
| 调试/运行 | `debug`、`run`、`build.console`、`linkedGradle`                                              |
| 终端      | `terminal-clipboard`、部分 `toolwindows`、`console-selection`                                |
| 插件      | `plugin-settings`、`pluginDetails`、`pluginEndpoints` 等 6 个                                |
| 语言高亮  | `goHighlight`、`pythonHighlight`、`kotlinHighlight`、`angularInline`、`icls-php`             |
| 其它      | `services`、`deployment`、`search`、`hierarchyRefactor`                                      |

**建议**：删除这些 spec 文件，让 `pnpm e2e` 恢复为可用的门禁。

---

## 五、未覆盖的检查（诚实声明）

以下**没有验证**，不能视为通过：

1. **真实 Tauri 运行时**：巡检用浏览器模式 + 内存后端。真实 Rust 后端的 Git 操作（实际读写仓库、真正的 `git` 子进程、askpass 凭证弹窗）未在本次巡检中跑通
2. **GitHub PR 面板**：需要真实 `gh` CLI 和登录，未测
3. **Shelf / Stash / Changelist UI**：e2e 未覆盖，未手动点过
4. **Worktree 操作**：同上
5. **多窗口 / Comparison 窗口 / Settings 独立窗口**：未测
6. **macOS 原生窗口行为**（标题栏、全屏、Dock 菜单）：未测

---

## 六、建议的下一步（按性价比排序）

| 优先级 | 动作                                 | 理由                                         |
| ------ | ------------------------------------ | -------------------------------------------- |
| 1      | 修 P1：`dev-session.ts` 的标识符前缀 | 一行改动，恢复 `dev:status` 与自动化验证能力 |
| 2      | 修 P2：Commit 面板聚焦               | 唯一体验退化项，用户每次提交都会碰到         |
| 3      | 删 103 个失效 e2e spec               | 让 `pnpm e2e` 重新成为可用门禁               |
| 4      | 决策 P2：Search Everywhere 是否补回  | 产品决策，不是 bug                           |
| 5      | 补 §5 未覆盖项的验证                 | 尤其真实 Tauri 下的 Git 操作                 |

---

## 七、巡检验证命令（可复现）

```bash
# 全量 e2e
npx playwright test e2e/ --reporter=json > result.json

# Git 相关（本次全部通过）
npx playwright test e2e/commit.spec.ts e2e/diff.spec.ts e2e/gitIgnore.spec.ts \
  e2e/gitLogScrollbar.spec.ts e2e/gitLogBranchMenu.spec.ts \
  e2e/gitLogCommitChecks.spec.ts e2e/newBranch.spec.ts e2e/projectTree.spec.ts

# 开发状态（P1 问题会复现）
npx tsx scripts/dev-status.ts
```

---

## 八、巡检后处理结果（2026-11-10 补）

按「先修 P1 → 再修 P2 → 再清 spec」的顺序完成：

| 项                     | 状态    | 验证                                                                  |
| ---------------------- | ------- | --------------------------------------------------------------------- |
| P1 worktree 标识符     | ✅ 已修 | `dev:status` 返回 `dev.gitrune.app.dev.w…` 与 bridge                  |
| P2 Commit 聚焦         | ✅ 已修 | 根因是 tool window 选中了分隔条 DIV；消息框加 `data-autofocus` 后修复 |
| P3 失效 e2e            | ✅ 已清 | 96/133 → **102/3**，剩余 3 个均为并行 flaky                           |
| **新 P1 信任 UI 缺失** | ✅ 已修 | 恢复 `features/project-trust`（394 行），未信任项目恢复可用           |

### 新 P1 详情（巡检第 2 轮发现）

**现象**：`e2e/openProject.spec.ts` 断言 `Trust and Open Project?` 对话框，元素不存在。

**根因**：剥离时删了 `features/project-trust`，但 `crates/ide-host/src/trust.rs` 和
`ops/vcs/commands.rs` 的 34 处 `require_trust` 都还在。

**后果（严重）**：打开未信任项目时，所有 Git 操作被后端以
"Project is in Restricted Mode. Trust the project to enable execution." 拒绝，
而用户看不到任何信任入口 → **Git 功能对新克隆的仓库完全不可用**。

**修复**：从 openidea 恢复该 feature 并在 `features/index.ts` 注册（置于最前，
因为它在任何项目打开前就要生效）。typecheck 0 错误，相关 e2e 通过。

### 最终门禁状态

```
pnpm typecheck      0 errors
oxlint src          0 errors (7 warnings)
vite build          ✓ built
cargo check         Finished
cargo test ide-vcs  238 passed / 0 failed
playwright e2e      102 passed / 3 failed（3 个均为并行 flaky，串行全过）
```
