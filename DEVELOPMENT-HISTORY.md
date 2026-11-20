# GitRune 开发经验总结

> 本文从 29 个真实提交中提炼，记录**为什么这样做**、**踩过哪些坑**、**哪些判断被证明是错的**。
> 代码会变，这些经验不会过期。
>
> 写作时间：2026-11-13。项目从此重开 Git 历史，本文是唯一的历史档案。

---

## 0. 一句话结论

**GitRune 是把 OpenIDEA 的代码开发能力整体剥离后剩下的纯 Git 客户端。**
真正难的不是删除，而是**判断哪些东西看起来独立、实际被依赖**。

| 指标         | OpenIDEA | GitRune      | 降幅 |
| ------------ | -------- | ------------ | ---- |
| Rust crate   | 38       | **11**       | −71% |
| Rust 行数    | ~252,000 | **65,246**   | −74% |
| 前端 feature | 42       | **7**        | −83% |
| 前端 TS 行数 | ~380,000 | **164,063**  | −57% |
| 编译产物     | —        | 36 MB `.app` | —    |

保留的核心：`ide-vcs` **15,611 行、238 个测试，一行未改**。

---

## 1. 最贵的三个教训

### 教训 1：`cargo check` 通过 ≠ 代码是好的

**这是本项目最大的坑，代价最高。**

删减完成后，`cargo check -p gitrune` 一直是绿的。我们据此认为后端没问题。
后来跑 `cargo test -p ide-host`，发现：

> **60 个测试文件从未编译过**（自删减起）。

它们还在用已删除的 `HostConfig.languages` / `syntax` 字段，还在调用 `ide_language`、
`ide_build`、`ide_remote` 等已经不存在的 crate。

**根因**：`cargo check` 默认**不编译测试代码**。

**正确做法**：

```bash
# 只 check 是不够的
cargo check -p gitrune

# 必须显式检查测试目标
cargo check -p gitrune --all-targets
# 或直接跑
cargo test -p <crate> --no-run
```

**这条应该写进任何 Rust 项目的 CI。**

---

### 教训 2：删掉「前端功能」时，后端的校验不会跟着消失

**这是唯一一个让应用「看似能用、实际不能用」的 bug。**

`features/project-trust`（394 行）被当作普通前端功能删除了。但：

- `crates/ide-host/src/trust.rs` 还在
- `ops/vcs/commands.rs` 里 **34 处 `require_trust` 调用**还在

结果：打开一个未信任的仓库，**所有 Git 命令都被拒绝**，提示 "Project is in Restricted Mode"，
而用户**找不到任何可以信任它的入口**。

> 一个全新 clone 的仓库，完全无法使用。

**怎么发现的**：重写 e2e 时，`openProject.spec.ts` 断言了 "Trust and Open Project?" 对话框，
而这个对话框已经不存在了。

**规律**：

> **前端是「入口」，后端是「门禁」。删掉入口，门禁还在，用户就被锁在门外。**

删除任何功能前，必须用后端符号反查：

```bash
# 删 feature 之前，先查后端是否还在校验它
grep -rn "require_trust\|requireLicense\|requireAuth" crates/ --include=*.rs
```

---

### 教训 3：重写函数时，容易丢掉「不在视线里」的分支

重写 `open_project`（524 行 → 82 行）时，丢掉了 watcher 里的 VCS 重扫分支。

后果：**在项目下新建 worktree、或往里 clone 仓库，Git 视图不会更新**，
必须重开项目才能看到。

`vcs_path_changed` 这个函数还在，但**没有调用者**了——这是最典型的信号：

> **一个函数只被测试引用，说明产品代码把它忘了。**

**发现方式**：`crates/ide-host/tests/vcs_worktrees.rs` 的
`watcher_picks_up_a_worktree_added_below_the_project` 测试失败。

**正确做法**：重写大函数时，先列出原函数调用的所有副作用，逐条确认新实现覆盖了。

---

## 2. 架构上真正重要的三个决定

### 决定 1：`ide-vcs` 一行不改

**这是整个项目的基石。**

`crates/ide-vcs` 是 Git 引擎：`gix` 读，`git` 子进程写。**15,611 行，238 个测试。**

删减全程没有碰它，原因：

- 它是这个项目为什么存在的理由
- 它有完整的测试保护（这是敢动其他一切的底气）
- 它**不依赖任何被删的 crate**

**验证**：删减前后 `cargo test -p ide-vcs` 都是 238 passed。

**关键前提**：它的依赖只有 `gix / which / regex-lite / libc` —— **与 Tauri 完全解耦**。
这意味着它未来可以被任何宿主复用（见第 6 节）。

---

### 决定 2：保留 `ide-ssh` 的凭证链，即使「看起来像远程开发功能」

`ide-ssh` 里有三块东西，长得像「远程开发」：

| 文件           | 行数 | 真实作用             |
| -------------- | ---- | -------------------- |
| `git_creds.rs` | —    | Git HTTPS 凭证       |
| `askpass.rs`   | —    | `git` 的密码提示路由 |
| `secrets.rs`   | —    | 系统钥匙串读写       |

**合计 1,196 行。** 如果把整个 crate 当成「远程开发」删掉，
**clone / pull / push 的 HTTPS 认证全部失效**。

**教训**：

> 不要按「模块名字」判断去留，要按「调用链」判断。

**这条已写入 `gitrune/AGENTS.md` 作为不可回退的规则。**

---

### 决定 3：`VirtualDocument` 搬到 `ide-core`

`VirtualDocument` 原本在 `ide-lsp` 里。删掉 `ide-lsp` 后，它的使用者（编辑器）就断了。

**处理方式**：不是重写，而是**把它移到 `ide-core`**（更低层的公共 crate）。

**规律**：

> 删除一个 crate 时，先看它里面有没有「不属于它」的公共类型。
> 有的话，**下沉到公共层**，而不是跟着一起删。

---

## 3. 前端剥离的具体手法

### 手法 1：三个 shim 模块兜住「已删能力的调用点」

删掉 `editor/lsp`（18,936 行）、`editor/refactor`（5,419 行）等之后，
`editor/core` 里还有大量调用点。

**没有**去改这些调用点（太多），而是新建：

| 文件                | 作用                         |
| ------------------- | ---------------------------- |
| `lspShims.ts`       | 语言服务器相关调用的 no-op   |
| `structureShims.ts` | tree-sitter 结构分析的空实现 |
| `syntaxShims.ts`    | 语法高亮的空 Extension       |

```ts
// syntaxShims.ts —— 语法高亮直接返回空，编辑器不报错
export const setSyntaxHighlights = StateEffect.define<SyntaxHighlights>();
export const syntaxHighlightField: Extension = [];
```

**收益**：一次改动消除了几百个 TS 错误，且不用碰 `editor/core` 的内部逻辑。

**代价**：这些是「技术债」，但**是明确的、可搜索的债**（文件名就叫 shim）。

---

### 手法 2：`ipc.ts` 用宽松类型兜住已删命令

`src/bindings.ts` 是自动生成的（`pnpm gen:bindings`），不能手改。
但它导出的类型对应着已删除的后端命令。

**做法**：在 `src/platform/ipc.ts` 里手工声明宽松的替身：

```ts
// 已删命令的类型：字段都可选，调用能编译
export interface LanguageInfo_Serialize {
  extensions?: string[];
  filenames?: string[];
  // ...
}

// commands 加索引签名：调用不存在的命令能编译，运行时 reject
export const commands = { /* ... */ } as Record<string, AnyCommand> & typeof realCommands;
```

**关键**：`commands` 的索引签名让**已删命令的调用能通过编译**，运行时才失败。
这是**故意的**——目的是让编译通过，而不是让它们能工作。

**踩过的坑**：一度把 `commands` 写成 `any`，导致 375 个错误
（`call()` 的泛型推断全废）。**正确做法是交叉类型**，不是 `any`。

---

### 手法 3：`noUncheckedIndexedAccess` 是很多 TS2722 的根因

`tsconfig.json` 开了 `noUncheckedIndexedAccess: true`，意味着：

```ts
const x = map[key];   // x 的类型是 T | undefined，不是 T
x.foo();              // TS2722: 可能为 undefined
```

删减过程中大量「莫名其妙」的类型错误都来自这里。
**遇到 `cannot be called / possibly undefined` 时先想到它。**

---

## 4. 界面巡检的四个真实 bug

这轮巡检用 Playwright 走查运行中的应用（浏览器模式 + 内存后端），
找出四个 bug，**全部是「静态对比看不出来」的类型**。

### P1-A：`dev:status` 找不到 bridge

`scripts/dev-session.ts` 构造的 worktree id 是 `ai.l0ng.openidea.dev.w...`，
而 Rust 侧已经改成 `dev.gitrune.app.dev.w...`。

**`dev-status.ts` 按字符串匹配，所以永远找不到。**

**规律**：

> 改名时，**运行时拼接的字符串**（不是常量）最容易漏。
> 编译器检查不到它们。

排查命令：

```bash
grep -rn "ai\.l0ng\|openidea\.dev\|openidea\." --include=*.ts --include=*.rs .
```

---

### P2：⌘K 打开 Commit 面板，光标不在输入框

**现象**：按 ⌘K，Commit 面板出现，但光标落在**面板分隔条**上。

**排查过程**（值得记录，因为走了弯路）：

1. 对比 `CommitView.tsx` 与上游 —— **字节级相同**，所以不是删减引入的
2. 静态看代码找不出问题
3. **实际探测运行中的应用**，发现：
   - textarea 可见、可用、可聚焦
   - `focus()` 调用有效
   - 所以**不是能力问题，是选择顺序问题**

**根因**：`ToolWindowView` 的聚焦逻辑是：

```ts
// 优先 [data-autofocus]，否则选第一个「非按钮、非链接」的可聚焦元素
```

Commit 面板有 15 个可聚焦元素：

- 前 8 个是工具栏按钮（被排除）
- **第 9 个是面板分隔条的 DIV**（被选中了）
- textarea 排在第 11 位，永远轮不到

**修复**：给 textarea 加 `data-autofocus`。

**教训**：

> **「能力正常但行为不对」时，去查选择顺序，而不是查能力。**

---

### P1-B：项目信任对话框（见教训 2）

---

### 新发现：仓库重扫丢失（见教训 3）

---

## 5. 打包阶段的三个坑

**打包才会暴露，开发时完全看不到。**

### 坑 1：自动更新指向了上游

`tauri.conf.json` 里的 updater 端点是：

```json
"endpoints": ["https://github.com/l0ng-ai/openidea/releases/latest/download/latest.json"]
```

**后果**：GitRune 0.1.39 看到 openidea 0.1.41，**提示用户升级**——
升上去就把 GitRune 换成了 openidea。

**这也是我犯的错**：为了消除签名报错，我把 `pubkey` 删了，
结果 `tauri-plugin-updater` 要求这个字段必须存在，**启动直接 panic**：

```
PluginInitialization("updater", "missing field `pubkey`")
```

**正确做法**：保留插件 + **生成自己的密钥对**，只清空 `endpoints`。

```bash
npx tauri signer generate -w .keys/gitrune.key
# 公钥写进 plugins.updater.pubkey，endpoints 留空
# .keys/ 必须 gitignore
```

**规律**：

> 消除一个报错时，先确认你消除的是「症状」还是「原因」。
> 我删 pubkey 消除的是症状，代价是应用启动不了。

---

### 坑 2：CLI 启动器指向不存在的文件

bundle 里有 `Contents/Resources/bin/openidea`：

```sh
exec "$contents/MacOS/openidea" --cli "$@"    # ← 实际文件叫 gitrune
```

**后果**：菜单里「Install Command」装出来的命令**永远无法运行**。

需要全链路改名：

- `tauri.macos.conf.json` 的 bundle files
- `ide-cli/src/install.rs` 的 `LAUNCHER` 常量
- socket 名（`openidea-{:016x}`）
- 菜单文案 + 双语词条

---

### 坑 3：签名不完整 → Gatekeeper 拒绝启动

Rust 的 linker 会自动做 ad-hoc 签名，**但不封装资源**：

```
Signature=adhoc, linker-signed
Sealed Resources=none          ← 没有封装 icon.icns
```

Gatekeeper 报错：

```
code has no resources but signature indicates they must be present
```

**修复**：打包后手动完整签名。

```bash
codesign --force --deep --sign - GitRune.app
# 验证
codesign --verify --deep --strict GitRune.app
```

签名后应该是：

```
Sealed Resources version=2 rules=13 files=2
valid on disk
satisfies its Designated Requirement
```

---

### 坑 4：文件关联抢了 70 种扩展名的默认打开方式

`fileAssociations` 里有 `public.source-code` 和 `.rs/.py/.ts/.kt/.go...`。

**一个 Git 客户端不该成为这些文件的默认编辑器。** 已只保留「文件夹」。

---

## 6. 关于 Rune 的评估（重要判断）

用户后来考虑用 [Rune](https://github.com/anomalyco/rune)（Rust 原生 UI 框架）重写 GitRune。

**实测数据**（这是判断的基础，不是猜的）：

```
startup: first IPC 2822 ms after process start
  main                22 ms
  setup at           606 ms     ← Rust 后端就绪
  webview           2216 ms     ← 78% 花在 WebView
```

| 阶段                | 耗时          | 占比    |
| ------------------- | ------------- | ------- |
| Rust `main`         | 22 ms         | 0.8%    |
| 后端 setup 完成     | 606 ms        | 21%     |
| **WebView + React** | **~2,216 ms** | **78%** |

**结论**：**后端不是瓶颈，WebView 才是。** Rune 恰好能解决这个（它没有 WebView）。

**但代价必须说清**：

- 前端 164,063 行全部作废
- 164k 行里的坑（Cmd+K 焦点、worktree 重扫、10k 行虚拟滚动）要重踩
- **Rune 没有语法高亮**（`rune-light` 是光照渲染，不是 highlighting）
- Rune 的文本编辑 1,005 行 vs CodeMirror 50,000+ 行
- Rune 是 **pre-release**，pin rev 使用

**唯一真正对口的组件**：`rune-kit::diff`（3,272 行）

- 支持**行级注释**（`Annotation` + `on_comment`）—— 这是 GitRune 目前没有的（PR review 需要）
- `myers.rs` + `model.rs` 是纯逻辑，**不依赖 Rune 渲染**，可以移植

**建议**（如果将来重走这条路）：

> 先做 1-2 天的原型：用 `ide-vcs` + `rune-kit::diff` 写一个能打开真实仓库的最小客户端，
> 实测三个数字：冷启动、10k 行 diff 打开时间、中文输入法。
> **数据出来再决定，不要凭 demo 视频决策。**

**关键前提**：`ide-vcs` 只依赖 `gix/which/regex-lite/libc`，**与 Tauri 零耦合**——
这意味着重写前端不需要动后端。这是当初删减时无意中留下的最大资产。

---

## 7. 工程习惯

### 门禁（每次提交前）

```bash
npx tsc --noEmit          # 0 errors
npx oxlint src e2e        # 0 errors
npx vite build            # PASS
cargo build -p gitrune    # 0 warnings
cargo test -p ide-host    # 72 passed
cargo test -p ide-vcs     # 238 passed
npx vitest run src/features/vcs/__tests__   # 760 passed
npx playwright test e2e/  # 104 passed (1 flaky 可接受)
```

### e2e 的两个真实 flaky（不是产品 bug）

**1. 10k 行 diff 的 word 高亮**
文件虚拟化，F7 跳转后只有可见块被挂载。立刻读 DOM 得到空数组。
**修法**：`expect.poll` 等装饰渲染（20s），并且**unified 模式不能等 `a` 侧**（它不存在）。

**2. boot 启动定时器**
`page.clock.install()` + 30s deadline + `runFor(25_000)` 余量太小，
并行下 Playwright 自己的轮询把假时钟推过 deadline。
**修法**：降到 `10_000`，并给该用例 `retries: 2`。

---

## 8. 踩过的工具坑

| 坑                     | 现象                                  | 解决                                                     |
| ---------------------- | ------------------------------------- | -------------------------------------------------------- |
| `ron v0.12.3` 下载失败 | 代理可达但 crates.io 不通             | 根因是 `keyring-core/sample` feature，改用 `mock::Store` |
| Playwright 缺浏览器    | 8 个 e2e 假失败                       | `npx playwright install chromium`                        |
| GUI 命令挂死           | 启动 GUI 后忘了 `&`，前台等待 20 分钟 | **长驻进程必须 `run_in_background: true`**               |
| SVG 注释里的 `--`      | 浏览器显示错误页                      | XML 注释不能含 `--`（`git log --graph` 踩过）            |
| `pnpm test -- <path>`  | 忽略过滤，跑全量（几分钟）            | 用 `pnpm vitest run <path>`                              |
| lint-staged 阻断提交   | unused imports 导致 commit 失败       | 提交前先跑 oxlint                                        |

---

## 9. 如果重来一次，会怎么做

### 顺序应该反过来

**错误顺序**（实际走的）：

```
删后端 → 删前端 → 修类型错误 → 跑测试 → 发现测试从没编译
```

**正确顺序**：

```
1. 先跑一次完整测试，建立基线（cargo test --all-targets）
2. 删一个模块
3. 立刻跑测试
4. 重复
```

**理由**：测试基线是**唯一能告诉你「什么被弄坏了」的东西**。
删减完成后再建基线，等于摸黑走完全程。

### 应该先做的事

**删任何 feature 之前，先做后端符号反查**：

```bash
# 假设要删 feature "code-insight"
grep -rn "code_insight\|CodeInsight" crates/ --include=*.rs
grep -rn "require_\|authorize" crates/ide-host/src/ops/ --include=*.rs
```

如果后端还有校验逻辑，**前端入口不能删**。

### 改名时应该用清单

运行时字符串是编译器检查不到的：

```
□ dev identifier 前缀（dev.rs）
□ worktree id 拼接（dev-session.ts）
□ 数据目录路径
□ 钥匙串 service 名（secrets.rs）
□ socket 名（socket.rs）
□ CLI 启动器里的 exec 路径
□ 菜单文案
□ 双语词条
□ SVG 注释 / 文档
□ updater 端点
```

---

## 10. 现在项目是什么样

### 保留的 11 个 crate

| crate           | 作用                 | 改动                   |
| --------------- | -------------------- | ---------------------- |
| `ide-vcs`       | **Git 引擎**         | **零改动，238 测试**   |
| `ide-github`    | PR（走 `gh` CLI）    | 无                     |
| `ide-host`      | 项目/窗口/ops        | 重写 `Project`、`Host` |
| `ide-workspace` | 设置、文档、本地历史 | 无                     |
| `ide-vfs`       | 文件系统             | 无                     |
| `ide-ssh`       | **Git HTTPS 凭证链** | 不可删                 |
| `ide-core`      | 公共类型             | 接收 `VirtualDocument` |
| `ide-cli`       | CLI 启动器           | 改名                   |
| `ide-text`      | 文本坐标             | 无                     |
| `ide-fonts`     | 字体                 | 无                     |
| `ide-clipboard` | 剪贴板               | 无                     |

### 保留的 7 个 feature

```
app  github  project-tree  project-trust  settings  vcs  welcome
```

### 产品身份

- 名称：**GitRune**
- 标识：`dev.gitrune.app`
- 配置目录：`.gitrune/`（**向后兼容 `.openidea/`**）
- 图标：`design/gitrune-icon.svg`（GR 字母 + commit graph）
- 签名：`.keys/gitrune.key`（**丢失则无法推更新**）

---

## 11. 未完成/未验证

| 项                         | 状态                                            |
| -------------------------- | ----------------------------------------------- |
| GitHub PR 面板             | **未实测**（需 `gh` 登录）                      |
| 真实 Tauri 运行时 Git 操作 | 部分验证（changelist 写入成功）                 |
| 辅助窗口（比较/设置）      | 未测                                            |
| macOS 原生窗口行为         | 未测                                            |
| `textCatalog.test.ts`      | 9 个测试失败（词条残留已删功能的文案）          |
| 启动耗时                   | 2.8s，**78% 在 WebView**（Rune 可解，但代价大） |

---

## 附：这份文档的由来

本文从 29 个提交的提交信息、真实 bug 排查过程、实测数据中提炼。

**Git 历史已被重置**，所以本文是：

- 唯一的开发过程记录
- 决策依据的存档
- 下次重来的检查清单

**写作原则**：

1. 只写**验证过**的事实（带路径、数字）
2. 明确区分「判断」和「实测」
3. **记录错误判断**，不只是成功（比如「删 pubkey 消除签名报错」的教训）
