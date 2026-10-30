# PortableHub

Windows 10/11 便携软件管理器。Tauri 2 + Svelte 5，单 exe，默认像素风，支持自制主题。

![Command Deck 主界面](docs/assets/command-deck.png)

> 平台边界：构建脚本与运行实现为 Windows 专用；配置格式、主题与文案表跨平台。
> 如需 macOS 构建，新增 `scripts/_mac/` 并在项目管理侧声明，不改共享脚本。

## 它能做什么

- 扫描 `Apps\` 下的绿色软件，以卡片网格展示；也支持从全盘「发现」并导入
- **Command Deck 主界面**：左库导航 + 应用矩阵 + 右侧选中详情；批量标签/分组/分类
- 三套内置主题：像素暗夜（默认）、暖纸陈列、冷灰索引
- 单击选中、双击/Enter 启动，方向键选择，`Ctrl+Alt+P` 呼出，托盘常驻
- 搜索支持名称子串与**拼音首字母**（输入 `wx` 可找到「微信」）
- 右键菜单可编辑应用（名称 / 主程序路径 / 启动参数 / 工作目录）
- 导入结果显示成功/跳过/失败明细，可一键复制
- 逐条勾选开机自启（只写用户 Startup 快捷方式，**不写注册表**）；每次启动 Hub 会检测并补齐未运行的自启项
- 主题文件放在 `Themes\`，换肤即时生效；设置页左侧分区导航
- 配置、缓存、WebView2 数据全部在程序目录内，删目录即卸载

## 目录

```
PortableHub.exe
config.json
Apps\<应用>\...
Themes\<主题>.json
_cache\            # 图标、WebView2 数据、可删
```

## 下载

从 [Releases](https://github.com/yrui-ql/portablehub/releases) 下载 `PortableHub.exe`，
放到任意目录运行即可；首次运行会在同目录生成 `Apps\`、`Themes\` 和配置。

## 构建

前置：Node 22+、Rust（msvc）、MSVC Build Tools、WebView2 Runtime。

```powershell
powershell -ExecutionPolicy Bypass -File scripts/setup-toolchain.ps1 -Check
powershell -ExecutionPolicy Bypass -File scripts/setup-toolchain.ps1
npm install
powershell -ExecutionPolicy Bypass -File scripts/build.ps1
```

产物：`src-tauri/target/release/PortableHub.exe`。

## 验证

```powershell
powershell -ExecutionPolicy Bypass -File scripts/verify.ps1
```

依次跑私密词扫描（含 `src`、`tests`、`e2e`、`scripts`、`docs`）、前端单测、类型检查、
拼音表离线结构校验、浏览器 E2E、Rust 单测、release 构建与无界面自检；任一失败退出码非 0。

单独跑某个前端 E2E 套件：

```powershell
npm run e2e -- e2e/edit.spec.ts
```

`src/lib/pinyin-table.ts` 是生成产物，由 `node scripts/gen-pinyin-initials.mjs` 重放。两种校验语义不同：

- `node scripts/gen-pinyin-initials.mjs --check`：**联网校验**。重新下载 pinyin-data 并与已提交的表逐字比对；
  取源失败（断网、上游不可达）或映射漂移都算失败，退出码非 0。
- `node scripts/gen-pinyin-initials.mjs --check --offline`：**离线结构校验**。不联网，只检查已提交产物的
  结构、参考首字母与体积，输出会注明未与上游比对；`scripts/verify.ps1` 里跑的是这一档。

需要本地副本时用 `--source <pinyin.txt>` 指定源文件（文件不存在即失败）。

## 主题格式

```json
{
  "schemaVersion": 1,
  "id": "my-theme",
  "name": "我的主题",
  "author": "me",
  "vars": { "--ph-bg": "#101018", "--ph-accent": "#ff8800" },
  "css": ".ph-card { border-style: dashed; }"
}
```

可用变量与稳定选择器见 `docs/THEME.md`（待补）。CSS 禁止 `@import` 与外链。

## 隐私

- 无遥测、无账号、不自动上传应用列表或使用记录；
- AI 分类/建议默认关闭，开启并同意后只发送应用名称、路径、大小等元数据，不发送文件内容；
- 发布版二进制移除了本机构建路径（`scripts/rust-paths.ps1` + `scripts/check-release-paths.mjs`）。

## 许可

[MIT](LICENSE)。内置字体 FusionPixel 使用 SIL Open Font License，见
`src/assets/fonts/OFL.txt`；其他依赖的许可见 `package-lock.json` / `Cargo.lock`。
