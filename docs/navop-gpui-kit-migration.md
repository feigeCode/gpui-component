# Navop 从 `navop-gpui-ce` 迁移到 gpui-kit

本文记录 Navop 放弃 `navop-gpui-ce` fork、直接接入上游 `gpui-kit`（`gpui-component` `main`）的迁移方案。

- 分支：`port-ce-generic-to-main`（基于 `main`，已合入可向上游提 PR 的通用能力）
- 目标：Navop 应用只依赖 `gpui-kit`，不再维护 gpui-component 的 CE fork

---

## 一、迁移后白捡的能力（main 已包含，CE 里这些是上游同步来的）

CE 分支里这些能力在 `main` 上本来就有，迁移后无需再自己维护：

- 导航/撤销：`NavStack`、`UndoHistory`、`History` 与导航拆分
- 编辑器：多光标、CRLF 光标边界、列选择宽度
- `Icon::data` / `IconSource`（SVG 字节）
- Select/List 的 `on_dismiss`、committed values、list 测高
- fps headline 采样重写
- shell 运行时主体（`scope::with_current`、materialize、quickjs、host_modules、dock_api 等）

---

## 二、本分支已迁移到 main 的通用能力（向上游提 PR）

这些是 CE 中**通用、且 gpui-kit 本身不具备**的能力，已从 `navop-gpui-ce` 移植到 `main` 分支 `port-ce-generic-to-main`，按功能拆成独立 PR：

### 1. 高亮器动态 WASM 语法解析器
- `LanguageRegistry::register_parser_factory` / `parser()`
- 运行期注册 `Fn() -> Result<(Parser, Language)>`，解析时优先于静态编译的 grammar
- 文件：`crates/component/src/highlighter/registry.rs`、`input_adapter.rs`、`highlighter.rs`
- 测试：`dynamic_language_uses_registered_parser_factory`

### 2. 编辑器装饰扩展
- 类型：`GutterMarker`、`GutterMarkerRenderer`、`RangeDecoration`、`RangeDecorationStyle`、`InlineWidget`
- 内部存储：`EditorAnnotations`（document_revision、completion_epoch、markers/decoration/widgets）
- 渲染：gutter marker 车道、range decoration（Fill/Frame）、inline widget
- 文件：`crates/base/src/input/{editor/decorations.rs, base/{kind.rs, element.rs, state.rs}, editor/mod.rs, mod.rs}` + `crates/component/src/input/editor.rs`
- 新增事件：`InputEvent::GutterMarkerMouseDown`
- 测试：`geometric_decorations_and_widgets_follow_utf8_edits`、`extension_ranges_and_offsets_clip_to_utf8_boundaries`、`frame_outline_is_continuous_across_different_line_widths`

### 3. LSP 补全防过期 + 手动刷新
- `completion_epoch` / `document_revision` 追踪，异步补全返回时校验上下文是否仍有效
- 公开方法 `refresh_completion_popup`（元数据异步就绪后重触发补全）
- 文件：`crates/base/src/input/editor/lsp/completions.rs`
- 测试：`completion_context_rejects_every_stale_dimension`

### 4. shell `with_current` 重导出
- `pub use scope::{ScopePhase, with_current, with_current_app};`
- 文件：`crates/shell/src/lib.rs`

### 5. Icon 彩色渲染（`IconColorMode`）
- `IconColorMode`（`Mono`/`Color`）枚举 + `IconNamed::color_mode()` 默认方法（默认 `Mono`），定义在 `gpui_kit_assets`（`crates/assets/src/icon.rs`），`gpui_component` re-export。
- `Icon` 增加 `color_mode` 字段 + `color()` / `mono()` / `color_mode()` 方法；`Icon::build` 自动继承 `IconNamed::color_mode()`。
- 渲染分支：`Mono` 走原有 `svg()` + `text_color`（alpha-mask 单色），`Color` 走 `img()` 光栅化以保留 SVG 内置色（品牌/产品 logo 的 `fill="#…"` 必须走 `img`，`svg()` 会丢色）。
- 文件：`crates/component/src/icon.rs`、`crates/assets/src/icon.rs`、`crates/assets/src/lib.rs`
- 测试：`gpui-component` 现有 icon 测试全过（18 个）。

> 这是第四节第 2 项里原属 Navop 层的 `IconColorMode` / color-mono 模式，已通用化进 main；Navop 侧无需再自己实现。

### 6. shell contained view 嵌入（`load_view` / `ViewLoadOptions` / `LoadedScriptView`）
- `ViewLoadOptions`（root/entry/policy/write_type_declarations）+ `LoadedScriptView`（`view()`/`is_unloaded()`/`unload()`/`Drop` 生命周期：卸载时 `retire` view、`release_application_generation`、`cancel_policy_tasks`、`revoke_host_modules`）。
- `ShellRuntime::load_view(options, window, cx) -> Result<LoadedScriptView>`：以宿主自定义 policy 加载一个可嵌入的 `Entity<ScriptView>`（非窗口根 view）。
- 支撑改动：`load_app` 拆出 `load_app_with_options`（`write_type_declarations` 参数）；`Policy::revoke_host_modules()`；`lib.rs` re-export。
- 文件：`crates/shell/src/plugin.rs`、`engine/quickjs/mod.rs`、`policy.rs`、`lib.rs`
- 测试：移植 CE 的 `embedded_view_unload_retires_view_and_revokes_modules`，全量 696 测试通过。

### 7. shell 可取消异步宿主调用（`HostAsyncTask` / `cancellable_async_function`）
- `HostAsyncTask`（`future + Option<Arc<dyn Fn() + Send + Sync>>` cancel）：`new`/`without_cancel`/`cancel`/`into_parts`。
- `HostModule::cancellable_async_function`；`async_function` 改用 `HostAsyncTask::without_cancel`；`begin`/`dispatch_async` 返回类型改为 `HostAsyncTask`。
- 驱动：`engine/quickjs/host_modules.rs` 的 `host_async_call` 用 `into_parts` 拆 cancel；`scheduler::awaiting` 加 `cancel` 参数传给 `TaskState::with_cancellation`（`TaskState` 的 `cancellation`/`cancel_work` main 已有）。
- 文件：`crates/shell/src/host_modules.rs`、`engine/quickjs/{host_modules.rs, scheduler.rs}`、`lib.rs`
- 测试：`cancellable_async_function_exposes_its_cancel_action`。

### 8. shell `init_embedded`
- `pub fn init_embedded(cx: &mut App) { style::init(); }`：供「已由 `gpui_component::init` 初始化 gpui-base」的宿主调用，避免重复 `gpui_base::init`。
- 文件：`crates/shell/src/lib.rs`

### 尚未移植、可单独提 PR 的 shell 能力（建议后续）
- typings 的 element-method 重载 + `Element` 联合类型（注意 CE 把 `gpui` 模块重命名为 `gpui-kit`，上游应保留 `gpui` 模块名，只取重载机制）

---

## 三、应用层（Navop）需要改造的内容

### 3.1 包名 / 路径 / import
| CE fork | gpui-kit (main) |
|---|---|
| `gpui_ce_components*` v0.2.0 | `gpui-component` / `gpui-kit` v0.6.x |
| `crates/ui` | `crates/component` |
| `crates/macros` | `crates/component-macros` |
| `use gpui_component::…` | `use gpui_kit::component::…` / `use gpui_kit::*` |
| 依赖 `gpui`（git `gpui-ce`） | 依赖 `gpui-kit`（自带 `gpui-pre` 快照） |

### 3.2 依赖
- 去掉 `gpui-ce` / `gpui_ce_*` 的 git 依赖，改为 crates.io 的 `gpui-kit`。
- 应用 `Cargo.toml` 只声明 `gpui-kit`，通过 `gpui_kit::platform` / `gpui_kit::base` / `gpui_kit::component` / `gpui_kit::assets` 使用。

### 3.3 入口初始化
- 保持 `gpui_component::init(cx)` 语义，但命名改为 kit 侧（见 main 的 `gpui_kit::component` 或对应 init）。

### 3.4 语义化令牌 vs 具体数值
- CE 的 `theme/geometry.rs`（spacing/radius/control/layout/tree/border/shadow/opacity/motion/overlay/resize 令牌）没有进上游，Navop 需要在自己 crate 里重建这套几何令牌层（见第四节）。

### 3.5 分拆 PR 建议
1. 先提交第二节的 1–4 到上游（已在本分支）。
2. Navop 应用层迁移单独 PR。
3. 通用化收益高的能力（动态解析器、装饰扩展、补全防过期）优先提，减少长期 fork 面。

---

## 四、需要迁移到应用层的能力（Navop 专属，不要进上游）

这些是 CE 里的 Navop 专属实现，迁移到 gpui-kit 后应下沉到 Navop 自己的 crate：

1. **Theme 几何令牌** `theme/geometry.rs`（约 616 行）：Navop 的间距/圆角/控件/布局/树/边框/阴影/透明度/动效/浮层/拖拽尺寸体系，作为 Navop 应用的 theme 层。

2. **Icon 体系扩展**（`icon.rs` 约 406 行）：`IconSize`、`file_path`、`FunctionalIcon` / `BrandIcon` / `ObjectIcon`。配套 `icon_named` proc-macro 的 SVG 内联颜色检测（`svg_uses_intrinsic_color`）。

   > `IconColorMode` + `color()` / `mono()` 已通用化进 main（见第二节第 5 项），**只有** `IconSize`、`file_path`、`FunctionalIcon`/`BrandIcon`/`ObjectIcon` 及颜色检测宏仍属 Navop 层。

3. **~150 个 Navop SVG 图标**（数据库、网络设备、各发行版/云厂商/数据库 logo 等）：`crates/assets/assets/icons/` 下新增的图标是 Navop 资产。

   > **已落地**：图标已复制到 Navop 新建的 `crates/one-assets`（见第六节），不再依赖 CE fork 的 assets 目录。

4. **`gen-navop-typings`** 工具：生成 `gpui-kit.d.ts` 的 Navop 专用脚本，作为 Navop 应用侧工具。

5. **Navop 编辑器集成 / LSP 补全扩展**：CE 里未通用化的部分（数据库对象补全、特定 decorations 用法等）留在 Navop 应用层。

6. **shell 运行时定制**：`with_current` 已进上游；剩余的 contained shell 嵌入、`gpui`→`gpui-kit` 模块重命名等按需留在 Navop 层或单独提 PR。

---

## 五、GPUI 层补丁风险（本仓库之外，迁移前必须验证）

CE 的 `gpui-ce`（git `feigeCode/gpui-ce`）带有三个 gpui 层补丁，切到 crates.io 的 `gpui-pre` 后**必须确认是否已上游化到 Zed**：

1. macOS x86_64 `BOOL` 修复
2. Linux XIM 修复
3. macOS IME 组合范围（`NSNotFound` replacement range）

若 gpui-pre 未包含这些，Navop 的中文 IME、Linux 输入法会退化。验证方式：对照 Zed 主线对应提交，或在 gpui-kit 上跑 Navop 的中文 IME 场景。

---

## 六、Icon 迁移现状与剩余工作

### 6.1 已完成

**main 侧（本仓库，第二节第 5 项）**：`IconColorMode` + `Icon` 的 color/mono 渲染分支，编译与 18 个测试通过。

**Navop 侧（`crates/one-assets`，新建 crate）**：

- `assets/icons/`：249 个 SVG（从 CE fork 复制，含品牌/DB/发行版 logo）。
- `build.rs`：扫描 SVG 生成 `IconName` 枚举，`pascal_case` 转变体名 + `svg_uses_intrinsic_color` 检测 color_mode，产出 `OUT_DIR/icon_name.rs`，其中 `impl gpui_component::IconNamed for IconName`（含 `color_mode()`）。
- `src/lib.rs`：
  - `IconName` 的 `view()` / `color()` / `mono()` 固有方法（委托给 `gpui_component::Icon`）。
  - `From<IconName> for AnyElement` + `RenderOnce`，使 `.icon(IconName::X)` / `.child(IconName::X)` 可用。
  - 首字母缩写别名（`AI`、`MongoDB`、`PostgreSQLColor`、`MySQLColor`、`SQLiteColor` 等 18 个），与 CE fork 的 `IconName` 拼写保持一致。
  - `Assets`（`rust_embed::RustEmbed`）+ `AssetSource`，运行时按 `icons/xxx.svg` 提供 SVG 字节。
- 测试：3 个（color_mode 检测 ground truth、缩写别名、path 与 asset source 一致）通过。

> 关键点：navop 的 `IconName` 枚举是**编译期生成**的（249 个变体），color_mode 由 SVG 内容自动推断。`Icon::new(IconName::X)` 通过 `impl<T: IconNamed> From<T> for Icon` 自动带上 color_mode，无需改 navop 的调用代码。

### 6.2 剩余工作（Navop 侧，切到 main 后）—— 已全部完成

1. **import 改写（约 134 文件）** ✅：`use gpui_component::IconName` → `use one_assets::IconName`。`IconNamed` 保留 `gpui_component::IconNamed`（main 里带 `color_mode()`）。

2. **`IconSize` 归位** ✅：34 个文件改到 `one_ui::IconSize`，删除 `one_ui` 里 `From<IconSize> for gpui_component::IconSize`。

3. **`FunctionalIcon` / `ObjectIcon`** ✅：调用处改为 `IconName::X.mono()` / `.color()`。

4. **`file_path()`** ✅：main 的 `Icon` 最终不加 `file_path`（改用 `Icon::data()`）。Navop 侧：`driver_icon_from_file_path` 用 `fs::read` + `Icon::data()`；`ssh_form_window` 用 `img(path)` 直接渲染上传图标。

5. **彩色 icon 塞进 gpui_component 组件** ✅：无需额外处理——main 的 `Icon::build` 会从 `IconNamed::color_mode()` 继承颜色模式，`From<T: IconNamed>` 自动携带，彩色 icon 进任何组件（Button/PopupMenuItem）都正确。

6. **入口注册资产源** ✅：`AppAssets` 增加 `navop_icons: one_assets::Assets`，在 `driver` → `navop_icons` → `builtin` 链中回退（`one_assets::Assets` 提供 249 个 Navop 专属图标）。

7. **依赖切换** ✅：`gpui-component` / `gpui-component-assets` 等从 `feigeCode/gpui-component` fork 切到本地路径 `../../../gpui-component`（`gpui-component-assets` 已映射到 `gpui-kit-assets`）。

### 6.3 校验

- `cargo test -p one-assets`：3 个测试通过。
- `cargo test -p gpui-component --lib icon`：18 个测试通过。
- color_mode 检测已用 CE fork 的 ground-truth 断言（`MongoDB`/`Redis`/`Database`/`Terminal`/`Vnc`/`Procedure`/`FolderFunctions`/`StatusConnectedLocked` = Color；`RdpLine`/`VncLine`/`Monitor`/`Paste` = Mono）逐一验证，与 fork 行为一致。

---

## 七、Shell 迁移现状与剩余阻塞点

### 7.1 已完成（第二节第 6–8 项）

`load_view` / `ViewLoadOptions` / `LoadedScriptView`（contained shell view 嵌入）、`HostAsyncTask` / `cancellable_async_function`（可取消异步宿主调用）、`init_embedded` 均已移植到 main，含回归测试，697 测试通过。

Navop 用到的 `gpui_shell` 公开符号在 main 已全部齐备：`ShellRuntime`、`ScriptView`、`HostModule`、`HostArguments`、`HostError`/`HostObject`/`HostValue`/`HostResult`、`HostAsyncTask`、`Capabilities`/`ExecuteGrant`、`policy::Policy`、`with_current_app`、`FrozenComponentRegistry`、`LoadedScriptView`/`ViewLoadOptions`、`init_embedded`、`gpui_component_shell::components`。

### 7.2 剩余阻塞点

**gpui-shell 层：已清零。** 唯一的「尚未移植」项是 typings 的 element-method 重载 + `Element` 联合类型，它是类型声明生成能力，不影响 Navop 编译。

**跨仓库（不在本仓库内）：**

- **GPUI 层补丁（第五节）**：`gpui-ce` → `gpui-pre` 的三个补丁（macOS x86_64 BOOL、Linux XIM、macOS IME 组合范围）需确认是否已上游化到 Zed，否则中文 IME / Linux 输入法退化。

**Navop 侧改造（第六节 6.2，应用层）：已全部完成。**

- icon import 改写、`IconSize` 归位、`FunctionalIcon`/`ObjectIcon`/`file_path`、入口资产源注册、依赖切换、Theme 几何令牌重建——均已落地，workspace 全量 `cargo check` 通过（EXIT=0）。

---

## 八、过渡方案：用 path patch 提前接上动态纹理

上游链路（zed PR → gpui-pre 发版 → gpui-kit 升级 → navop 恢复）走完之前，Navop 先用 `[patch.crates-io]` 把 `gpui-pre-*` 换成**本地 zed 快照**，这样动态纹理不必等发版。

工具：`navop/script/patch-local-gpui-pre.py`（用脚本头部的 docstring 说明原理）。

```bash
# 从本地 zed（默认取同级 ../zed 的当前分支）出一版 gpui-pre 并改好 Navop 的 patch
script/patch-local-gpui-pre.py --update-lock
```

它做三件事：

1. 调 `gpui-component/script/bump-gpui.ts <版本> --zed <本地 zed> --stage-only`——就是第八节说的官方快照流水线，产出 25 个 `gpui-pre-*` crate（版本 `0.3.99`，满足两侧 `^0.3.1` 要求）。
2. 把快照**复制到所有 checkout 之外**（`<gpui-component>/../.gpui-pre/workspace`）。
3. 重写 Navop 根 `Cargo.toml` 里标记之间的 `[patch.crates-io]` 块，并（`--update-lock`）跑 `cargo update -p ...` 把 `Cargo.lock` 挪到快照上。

三个必须知道的点：

- **不能直接 patch zed 源码树**：zed 的包名是 `gpui`/`util`/…、版本 0.2.2，永远不满足 `gpui-pre = "0.3.1"`。patch 要求版本匹配，所以必须先走改名+定版本的快照流水线。
- **只有 `[patch]` 不够**：Cargo 会保留 lock 里已满足要求的旧版本并忽略 patch（报 `was not used in the crate graph`）。必须显式 `cargo update -p gpui-pre ...`。
- **快照不能放在任何 workspace 内**：留在 `gpui-component/target/` 下会被归给 gpui-component 的 workspace 解析 `workspace = true`，那里没有 `accesskit` 等键。必须复制出去。

首轮结果：`cargo check --workspace --locked` EXIT=0，`cargo test -p remote_desktop_view --lib` 241 passed；`remote_desktop_view` 的动态纹理实现已从 `eda5371ae` 反向还原（`view.rs` 5 / `render.rs` 89 / `render_contract_tests.rs` 20 / `surface.rs` 43 行，与降级提交逐文件镜像）。

`Cargo.lock` 会同时出现 22 个「仅依赖边」变化（`windows-sys`/`itertools` 等重复版本的重新选择）——这是快照从 zed@6916400 前进到本次分支基线的必然结果，不是污染。

> 该 patch 块、`Cargo.lock` 与还原的源码**都不提交**：patch 路径只在本机存在，提交会破坏 CI；而还原的动态纹理代码又依赖该 patch，二者同进同退。上游 gpui-pre 发版后执行 `script/patch-local-gpui-pre.py --remove` 并回退 lock 即可。

---

## 九、发生产怎么提交

**核心约束**：官方 `gpui-pre-0.3.4`（截至本分支基线，0.3 系列全部子 crate）**零文件包含 `DynamicTexture`**。换言之，navop 的 patch 块、还原代码与 lock 23 条 gpui-pre 路径必须**同进同退**：

- 去掉 patch → navop 编译失败（动态纹理 API 不存在）
- 提交还原代码但留 patch → CI 一编就挂
- patch 路径 commit 进仓库 → 路径只在本机存在，CI/同事 checkout 一编就挂

只有一条发生产路径：**等官方 gpui-pre 发版包含动态纹理 → navop 一并去掉 patch 并提交还原代码**。

### 路径 A：走上游（正式、生产合规）

强依赖链路：

```
zed PR (cae01216cb 动态纹理)
  ↓ 合入 zed-industries/zed
gpui-component 升级 gpui-pre（维护者跑 bump-gpui.ts <新版本> --zed <新 zed>）
  ↓ 发到 crates.io（gpui-pre >= 0.3.5）
navop 去掉 patch + 提交还原代码 + 提 PR
```

**Step 1 — 推 zed 动态纹理到上游**

```bash
cd zed
git push -u <fork-remote> dynamic-texture      # feigeCode/zed 或私人 fork
gh pr create --repo zed-industries/zed \
  --base main --head <fork>:dynamic-texture \
  --title "feat(gpui): add dynamic texture update support" \
  --body-file <(git log -1 --format=%B HEAD)
```

评审预案：Metal/wgpu/DirectX 三个后端新增路径都是大块改动，zed 评审可能要求拆 PR。可预告方案：单独 PR 一个 trait + atlas key（动态纹理基础），后续 PR 按后端拆。

**Step 2 — 等合并并升级 gpui-component**

合并后：

```bash
cd zed && git rebase main && cargo test -p gpui -p gpui_apple -p gpui_wgpu --lib
cd ../gpui-component
bun script/bump-gpui.ts 0.3.5 --zed ../zed      # 实际命令以维护者约定为准
```

把 `port-ce-generic-to-main` 的 7 个迁移提交按用户偏好**拆小**（`Spinner::animation_id` 偏 navop 应收回，其余通用）：

- PR-A：`Dialog::alert` / `Dialog::confirm` / `dialog-state-changed` 事件
- PR-B：`Button::glyph_size` + `DatePickerState open accessors`
- PR-C：`shell: init_embedded` + `shell: cancellable async host functions`
- （已合掉）`Icon::file_path` 已 `Icon::data` 替代

**Step 3 — navop 去掉 patch 并提 PR**

```bash
cd navop
python3 script/patch-local-gpui-pre.py --remove   # 删 [patch.crates-io]
cargo update -p gpui-pre                            # 让 lock 跟随上游
cargo check --workspace --locked
cargo test -p remote_desktop_view --lib            # 241 tests still green

git diff --stat
# 预期只有：
#   M crates/remote_desktop_view/src/view.rs
#   M crates/remote_desktop_view/src/view/render.rs
#   M crates/remote_desktop_view/src/view/render_contract_tests.rs
#   M crates/remote_desktop_view/src/view/surface.rs
#   M Cargo.toml      (patch 块被 --remove 删干净；Cargo.lock 会有少量漂移但只跟 gpui-pre 系列相关)
#   ?? script/patch-local-gpui-pre.py                (辅助脚本，建议永久保留)

git add -A
git commit -m "feat(remote_desktop_view): restore dynamic texture implementation

gpui-pre now exposes the dynamic-texture API. Revert the RenderImage
fallback and let the surface publish dirty rects each frame:

- surface.rs uses DynamicTexture directly; backing framebuffer becomes
  the upload source for paint_dynamic_texture.
- render.rs calls update_dynamic_texture / paint_dynamic_texture in
  place of paint_image, and drops the placeholder handles via
  drop_dynamic_texture.
- view.rs and the contract tests assert the dynamic-texture path."

gh pr create --base dev --head sync-gpui-kit \
  --title "feat(remote_desktop_view): restore dynamic texture via gpui-pre" \
  --body "Depends on gpui-pre >= 0.3.5 (DynamicTexture API)."
```

### 路径 B：内部用，不走上游（短中期过渡）

如果 zed 上游 PR 卡住、navop 又必须用动态纹理跑生产（演示/灰度），可行但**不能合并到 dev 分支**：

- 把 `navop-workspace/.gpui-pre/workspace/` 推到 navop 仓库的独立 branch（如 `internal/gpui-kit-snapshot`）
- navop 仓库内访问 git 相对路径 `../internal/gpui-kit-snapshot`
- 需在 navop 仓库加 `[workspace.exclude]` 避免嵌套冲突

**缺点**：每次 zed 分支更新都要重做一次快照；CI 要把 internal branch 拉下来；这本质上是维护第三套依赖。生产环境**不建议**走此路径。

### 实际节奏建议

按优先级：

1. **现在就做**：把 zed 分支推到 fork（10 分钟，避免后面忘记）。这一步不依赖任何人。
2. **同时**：在 gpui-component 仓库开 issue 提醒维护者，新 zed 合并后要跑 bump-gpui.ts 出新版本。这是链路阻塞点。
3. **等 zed 合并 + gpui-pre 重发**（不可控，慢则数周），再走路径 A 的 Step 3。
4. **navop 当前工作区先不 commit**：保留 patch + 还原代码 + 脚本的「待发状态」，避免出现「提交了一半」的中间态污染历史。

### 路径 B2：自维护 git fork（推荐用于内部/长期）

不需要等官方，按自己节奏发布。`gpui-pre` 的 crate 名 + 版本要求不变，只是来源从「本机 path 目录」换成「自有 git 仓库」。

仓库布局：

```
feigeCode/zed                       # 长期维护 dynamic-texture 分支（已就绪）
feigeCode/gpui-pre                  # 把 zed 转成 gpui-pre 命名空间后的产物
```

**与 path patch 的对比**：

- **path patch（path-to-path）**：本机 `<gpui-component>/../.gpui-pre/workspace/` 直接被 patch 命中。开发快，但绑死 build host。
- **git fork（patch-to-git）**：保留 `[patch.crates-io]`，把每条 `path = "..."` 改写成 `git = "..." + tag = "..."`。CI / 同事 / 部署机都能解析。

**首次发布**：

```bash
# 1) 准备好 fork 仓库（一次性，GitHub 网页创建 feigeCode/gpui-pre，private）

# 2) 发布当前快照到 fork（自动跑 staging、init、commit、tag、push）
script/publish-gpui-pre-fork.py \
    --fork-url "git@github.com:feigeCode/gpui-pre.git" \
    --tag "fork-0.3.99" \
    --init-only                          # 先 init+commit，inspect 后再 push
# 检查 .gpui-pre/publish 目录里的 commit 内容
# OK 后：
script/publish-gpui-pre-fork.py \
    --fork-url "git@github.com:feigeCode/gpui-pre.git" \
    --tag "fork-0.3.99"                  # 这次 push

# 3) navop 切换到 git 依赖（一次性；之后只在 zed 改完才需要重新发布）
script/migrate-to-git-fork.py \
    --fork-url "https://github.com/feigeCode/gpui-pre.git" \
    --tag "fork-0.3.99"
cargo check --workspace --locked
cargo test -p remote_desktop_view --lib  # 仍应 241 passed
```

**之后每次 zed dynamic-texture 分支变化**：

```bash
cd /path/to/zed && git pull                  # 或 merge main / cherry-pick
cd /path/to/navop
script/patch-local-gpui-pre.py --no-stage    # 刷 staging（仅本地）
script/publish-gpui-pre-fork.py \
    --fork-url "git@github.com:feigeCode/gpui-pre.git" \
    --tag "fork-0.3.100"                     # bump tag
script/migrate-to-git-fork.py --tag "fork-0.3.100"
cargo test -p remote_desktop_view --lib
git add -A && git commit -m "bump: gpui-pre to fork-0.3.100"
```

**切回官方**（上游 gpui-pre 含 DynamicTexture 后）：

```bash
script/migrate-to-git-fork.py --path          # 先回到 path patch（验证 lock 仍干净）
script/patch-local-gpui-pre.py --remove       # 删整个 patch 块
cargo update -p gpui-pre
cargo check --workspace --locked              # 应解析到 crates.io 的官方版本
git diff --stat
# 删那 4 个 navop 侧还原的源文件（已被 dev 接受、现在依赖官方 API）
```

**两个脚本的语义**：

- `script/publish-gpui-pre-fork.py`：把 stage 出的 workspace 复制到 `<gpui-component>/../.gpui-pre/publish/`，在那里 `git init` + `commit` + `tag` + `push`。参数化 fork URL、tag、branch，支持 `--dry-run` / `--init-only` / `--no-stage`。
- `script/migrate-to-git-fork.py`：只编辑 `[patch.crates-io]` 块（`path` ↔ `git + tag`）。不调 cargo，cargo update 留给用户控制时机；默认会跑 `cargo update -p gpui-pre-...` 让 lock 跟上来。

**为什么不能直接给 feigeCode/zed 打 patch**：zed 的 crate 名是 `gpui`/`util`/...、版本 0.2.2，navop 依赖 `gpui-pre`/`gpui-pre-util`/...、版本 `^0.3.1`。`[patch.crates-io]` 只能改**来源**（path / git / crates.io），不能改**包名/版本**——所以必须先经 `bump-gpui.ts` 做改名+定版本，产物**单独**建仓库。

**已落地（2026-09-10）**：

- `feigeCode/gpui-pre`（2026-09-10 已转 public）已创建，`main` 分支 + tag `fork-0.3.99`，含 25 个 `gpui-pre-*` crate（快照来自 zed `dynamic-texture` @ `52b2927a1b` + `cae01216cb`）。
- navop 的 `[patch.crates-io]` 已切换为 git 来源：`gpui-pre-* = { git = "https://github.com/feigeCode/gpui-pre.git", tag = "fork-0.3.99" }`，24 个包进图（`gpui-pre-reqwest-client` 不在图中被 cargo 忽略）。仓库转 public 后 HTTPS 匿名可拉，SSH key 不再是 build host 的前提。
- 验证：`cargo check --workspace --locked` EXIT=0；`cargo test -p remote_desktop_view --lib --locked` 241 passed。

**踩过的坑（重要）**：

1. **URL 必须是规范 URL**：Cargo 不接受 SCP 风格的 `git@github.com:feigeCode/gpui-pre.git`（`relative URL without a base`），必须写完整的 `https://github.com/feigeCode/gpui-pre.git` 或 `ssh://git@github.com/...`。git 命令本身两种都认，但 Cargo 的 URL 解析更严格。
2. **凭据**：仓库现在是 public，HTTPS 匿名拉取即可，任何 build host / CI 无需配置。push（发新 tag）仍需凭据：HTTPS + token 或 SSH。
3. **`-p` 与 graph 的关系**：patch 块 25 条但图中 24 个——不在图里的包（如 `gpui-pre-reqwest-client`）patch 会被 cargo 报 `was not used` 警告，保留它们是为了防止将来启用相关 feature 时混源。

**当前状态与提交建议**：

navop worktree 的全部改动（`Cargo.toml` patch 块 git 化、`Cargo.lock` git 源条目、4 个 `remote_desktop_view` 恢复文件、3 个脚本）已提交——patch URL 是远程 git，不再绑死本机路径。`feigeCode/gpui-pre` 已转 public，所有 build host 匿名 HTTPS 即可拉取。
