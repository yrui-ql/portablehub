# 主题格式

主题是一个 JSON 文件，放在程序目录的 `Themes\` 下，文件名与 `id` 一致（例如 `my-theme.json`）。
同名主题会覆盖内置主题；内置主题有 `pixel-dark`（默认）、`pixel-paper`、`pixel-slate`。

```json
{
  "schemaVersion": 1,
  "id": "my-theme",
  "name": "我的主题",
  "author": "me",
  "vars": {
    "--ph-bg": "#101018",
    "--ph-accent": "#ff8800"
  },
  "css": ".ph-card { border-style: dashed; }"
}
```

## 可用变量

| 变量 | 作用 |
|---|---|
| `--ph-bg` | 主背景 |
| `--ph-bg-alt` | 次级背景（标题栏、输入框） |
| `--ph-panel` | 卡片/面板背景 |
| `--ph-panel-hover` | 悬停背景 |
| `--ph-border` | 普通边框 |
| `--ph-border-light` | 高亮边框 |
| `--ph-accent` | 主强调色 |
| `--ph-accent-2` | 次强调色（自启标记） |
| `--ph-danger` | 危险/错误色 |
| `--ph-text` | 主文字 |
| `--ph-text-muted` | 次要文字 |
| `--ph-shadow` | 硬阴影颜色 |
| `--ph-shadow-offset` | 阴影偏移（像素风用 `4px`） |
| `--ph-radius` | 圆角（像素风用 `0px`） |
| `--ph-border-width` | 边框宽度 |
| `--ph-font` | 字体族 |
| `--ph-font-size` | 字号 |
| `--ph-image-rendering` | 图标渲染（`pixelated` / `auto`） |

## 稳定选择器

自制主题可以覆盖这些类的样式，但不要依赖其他内部结构：

`.ph-titlebar` `.ph-title` `.ph-wbtn` `.ph-toolbar` `.ph-search` `.ph-btn`
`.ph-grid` `.ph-card` `.ph-icon` `.ph-name` `.ph-badge` `.ph-menu` `.ph-menu-item`
`.ph-settings` `.ph-section` `.ph-row` `.ph-input` `.ph-select` `.ph-toast` `.ph-empty`

## 限制

- `css` 最大 64KB
- 禁止 `@import`、`url(http…)`、`url(https…)`、`javascript:`、`expression(`
- `id` 只允许小写字母、数字、`-`、`_`
- 变量必须以 `--ph-` 开头
- 主题损坏时自动回退到 `pixel-dark`，不会阻塞启动
