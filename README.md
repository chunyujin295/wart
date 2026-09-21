# wart

<p align="center">
  <img src="./doc/img/logo.png" alt="icon" width="200">
</p>


把文本转成 ASCII 艺术字，支持 Nerd Font 符号和渐变配色。为 Neovim dashboard 和 fastfetch 的 logo 而做。

同一个二进制两种用法：**不带参数启动 GUI，带参数走命令行**。

```sh
wart                                          # GUI
wart --text HELLO --font slant --gradient "#ff5f5f,#ffd75f" --out logo.txt
```

界面默认中文，右上角可切英文；配色可切浅色/深色（首次启动跟随系统）。

## Windows 下构建

```powershell
.\build.ps1                    # 跑测试、release 构建、打包到 dist\
.\build.ps1 -Mode quick -Run   # 快速重建并启动
.\build.ps1 -Example           # 额外生成一批样例到 dist\samples\
```

`dist\` 里是 exe、许可证，以及一个空的 `figlet\` 目录用来放自定义字体。

需要 Rust 1.85+，无系统依赖——字体、FIGlet 字库、图标元数据都编进了二进制。

换应用图标（exe / 任务栏 / 界面左上角那个）见[图标](#图标)。

## 发布

推 tag 即发布，其余交给 GitHub Actions（`.github/workflows/release.yml`）：

```sh
# 先把 Cargo.toml 里的 version 改成要发的号，提交
git tag v0.2.0 && git push origin v0.2.0
```

工作流会先跑测试、核对 tag 与 `Cargo.toml` 的 version 一致（不一致直接失败，免得发出去的二进制 `--version` 和 release 对不上），然后构建四个平台并附上校验和：

| 平台 | 产物 |
|---|---|
| `windows-x64` | `wart-vX.Y.Z-windows-x64.zip` |
| `linux-x64` | `wart-vX.Y.Z-linux-x64.tar.gz`（在 22.04 上构建，glibc 要求低，能在更多发行版上跑） |
| `macos-arm64` / `macos-x64` | `wart-vX.Y.Z-macos-*.tar.gz` |

每个包里的内容和 `dist\` 一致：二进制、README、许可证、空的 `figlet\` 目录。

**不想先打 tag 试一把**：在 Actions 页手动跑 `Release`（`workflow_dispatch`）——同样构建四个平台并上传产物，只是不发布 release、也不需要 tag。

两点注意：macOS 的二进制没有签名，下载后首次运行需要 `xattr -d com.apple.quarantine wart`（或右键打开）；Linux 包需要 glibc 2.35+。

`ci.yml` 是日常那道关：push 到 `main` 和 PR 会在 Ubuntu 与 Windows 上跑 `cargo test`，和 `build.ps1` 本地跑的是同一条命令。

## 两种生成模式

### FIGlet —— 经典大字

从 `.flf` 字体渲染多行横幅。smushing（字符挤压）、kerning、hardblank 的处理都遵循字体文件自己的头部声明。换行会堆叠成多个独立横幅。

**字体画不出来的字符怎么处理。** `.flf` 本质是一张「字符码 → ASCII 图案」的查找表，条目都是 ASCII——所以一个 Nerd Font 图标**没法被 FIGlet 字体画出来**，表里根本没有它。wart 的做法是把这些字符**用点阵模式那套引擎光栅化**，按大字高度的一点五倍画在大字旁边（更精确的尺寸规则见[图标样式](#图标样式)）：

```
╭────────────────────────────────────╮
│                                    │
│ ██╗    ██╗███████╗██╗      ██████╗ │
│ ██║    ██║██╔════╝██║     ██╔════╝ │
│ ██║ █╗ ██║█████╗  ██║     ██║      │
│ ██║███╗██║██╔══╝  ██║     ██║      │
│ ╚███╔███╔╝███████╗███████╗╚██████╗ │
│  ╚══╝╚══╝ ╚══════╝╚══════╝ ╚═════╝ │
│                                    │
│                                    │
│                                    │
│ ██╗   ██╗   ██╗    ⠀⠀⢀⣠⣤⣤⣄⣀        │
│ ╚██╗ ██╔╝   ██║    ⠀⣴⠟⠉⠀⠀⠀⠙⢿⡄      │
│  ╚████╔╝    ██║    ⣼⠁⠀⠀⠔⠈⠀⠀⠘⡇      │
│   ╚██╔╝██   ██║    ⣏⠀⠀⠀⢆⠀⠀⠀⡰⠃      │
│    ██║ ╚█████╔╝    ⢹⡄⠀⠀⠈⠑⠒⠉        │
│    ╚═╝  ╚════╝     ⠀⠹⣄             │
│                    ⠀⠀⠀⠑⠂           │
│                                    │
╰────────────────────────────────────╯

```

细节随字体行数缩放：字体越高，图标越大、越清楚。短字体（`Term` 只有 1 行、`Bigfig` 3 行）里的图标按 10 行的下限绘制，会比大字高出一截，这是刻意的——比缩到看不清要好。

### 点阵 —— 光栅化

用真实字体光栅化文字，再把像素映射成字符。要画 Nerd Font 图标和真正的逐字符渐变，靠的是这个模式。

| 字符集 | 每格源像素 | 说明 |
|---|---|---|
| 盲文 | 2×4 | 分辨率最高。点是分离的（盲文本来就是给触觉阅读设计的），所以实心区域看起来是穿孔的 |
| 形状匹配 | 4×8 | 每格挑一个「画出来最像」的可打印 ASCII 字符。适合图像和图标 |
| 四分之一方块 | 2×4 | 每格四个子块，靠字符自身的空隙提供透明，**不需要背景色** |
| 半方块 | 1×2 | 每格两个可独立着色的像素。需要终端支持背景色 |
| 线画 | 1×2 | 按边缘方向选 `/ \ \| _ -` |
| 方块 | 1×2 | `█` 实心剪影：一格不是满就是空 |
| ASCII 渐变 | 1×2 | `" .:-=+*#%@"`，六种里最粗的 |

**形状匹配**把 95 个可打印 ASCII 字符逐个光栅化，每个归约成「八个子区域的墨迹分布」向量；图像每一格也归约成同样的向量，取最近的。方法来自 Alex Harri 的 [*ASCII characters are not pixels*](https://alexharri.com/blog/ascii-rendering)。密度梯度问的是「这格多暗」，形状匹配问的是「这格长什么样」——后者才是对的问题。

## 图标样式

大字旁边的图标（以及任何 FIGlet 字体画不出的字符）怎么画。图标按 **1.5 倍大字高度**绘制，略微高出，读起来像一个刻意的徽标；**至少 10 行**——六行以内的字体都会被抬到这条线，宁可让图标压过大字，也不缩成看不清的糊块。这个高度是**图形本身**的高度，不含字体的行距空白，所以 6 行大字给图标 10 行、约 20 列。图宽上限是行数的四倍：像 em dash 这种几十比一的扁字形，按高度放大出来会是一条贯通终端的横杠，会被宽度截住，缩成一根短横。

| `--icon-style` | 用什么画 | 每格子单元 | 说明 |
|---|---|---|---|
| `shape`（默认） | 可打印 ASCII，按形状选 | 1 | ASCII 艺术质感，和大字同一媒介，而且可辨认 |
| `braille` | 2×4 点 | **8** | 形状分辨率最高、最锐利，但点阵质感明显不是大字的媒介 |
| `blocks` | `█` | 1 | 实心剪影。一格只有「满」和「空」两种，笔画一律画成实心一行 |
| `quadrant` | `▘▝▖▗▚▞▛▜▙▟` | **4** | 比半方块细一倍，且不需要背景色 |
| `halfblock` | `▀ ▄ █`，两半各一色 | 2 | **单色图形里最粗的一种**，见下 |
| `lineart` | `/ \ \| _ -` | 1 | 笔画词汇和大字一致，但笔画每格携带的信息远少于点阵，需要约 18 列才看得清 |

**关于半方块。** 它「最细腻」的名声说的是**颜色**（每格两个独立着色的像素），不是形状。图标是单色图形，比的是形状分辨率，而每个半方块字符只有 **2** 个子单元，盲文有 **8**——四倍差距。更糟的是渐变通常是横向的，同一格上下两半颜色几乎相同，`▀` 的前景/背景之分随之消失，它唯一的颜色优势也用不上。只在渲染**真正的多色图像**、且终端支持背景色时才值得用。上下两半颜色相同时（纯色，或横向渐变——同一格上下两半的颜色差通常在一两个色阶以内，肉眼看不出），它现在直接画成 `█`——既不必依赖背景色，笔画也不会只剩半格高。

**细线为什么曾经消失。** 方块曾经是 `░▒▓█` 五档深浅，按格子的**平均**覆盖率取档。可这个尺寸下一条线常常比格子还细，平均值于是把「线穿过这里」报成了「这里有一点点灰」，画出来是 `░`——等于没画。现在这四个按阈值判定的字符集（盲文、方块、半方块、四分之一方块）都改用**格内最强像素**判定：只要有像素达到阈值（`--threshold`，默认 0.5）就算有墨，笔画就按字符集本身的分辨率画实。

**为什么没有无缝的 2×4。** Unicode 13 的 *Symbols for Legacy Computing* 区正好有这个东西——sextants（2×3，`U+1FB00`–`U+1FB3B`，63 种组合齐全）和 octants（2×4，`U+1FB70`–`U+1FB8B`）。但扫描了本机全部字体加内置字体共 **410 个，没有一个包含其中任何一个码点**，发出去会全是豆腐块。所以无缝方案止步于四分之一方块。

## 边框

```sh
wart --text WART --font big --frame rounded --frame-padding 1 --out logo.txt
```

| 样式 | 字符 |
|---|---|
| 细线 | `┌ ─ ┐ │ └ ┘` |
| 圆角 | `╭ ─ ╮ │ ╰ ╯` |
| 粗线 | `┏ ━ ┓ ┃ ┗ ┛` |
| 双线 | `╔ ═ ╗ ║ ╚ ╝` |
| Powerline | 两侧圆角带。唯一一款真·Nerd Font 专属，其余是 Unicode 标准字符 |

边框在**上色之前**套用，所以渐变会自然从左边框贯穿到右边框，而不是给边框单独配一个颜色。

## 配色

纯色、线性（带角度）、径向、彩虹。颜色在工作图上**一次性烘焙**，所以 ANSI、Lua、HTML、SVG、PNG 和界面预览六者是同一个结果，而不是六个实现碰巧一致。

## 导出

| 格式           | 扩展名           | 用途                                                         |
| -------------- | ---------------- | ------------------------------------------------------------ |
| ANSI 文本      | `.ansi`          | 带转义序列，供 `fastfetch --file` 或终端 `cat` 使用。注意扩展名不要写成 `.txt`——含大量转义序列的文件本质上已不是纯文本 |
| 纯文本         | `.txt`           | 只含字符、无任何转义序列。适用于会把 `ESC[38;2…` 当字面量直接打印出来的场合：commit message、README、聊天窗口、源码注释 |
| fastfetch 配置 | `.jsonc`         | 指向 ANSI 文件的配置，包含一些不易查阅的设置项               |
| Neovim Lua     | `.lua`           | dashboard-nvim / snacks.nvim / alpha.nvim 的高亮组定义       |
| PNG            | `.png`           | 用于分享、README 配图                                        |
| HTML / SVG     | `.html` / `.svg` | 浏览器预览、矢量输出                                         |

同一个作品两种文本格式的体量差别很直观——带边框的 `WART`：

```
纯文本    782 字节，   0 处转义序列
ANSI     5847 字节， 339 处转义序列
```

`--format ansi` 配 `--no-color` 也能得到不带颜色的输出，但那是"上色之后又擦掉"；`--format plain` 是直接不产生颜色，而且不会因为参数写错而漏出转义序列。

### 给 fastfetch 用

![image-20260921115927772](https://map--depot.oss-cn-hangzhou.aliyuncs.com/image/image-20260921115927772.png)

导出一个 fastfetch 配置时，**它指向的那个 ANSI 文件会一起写出来**（同名、同目录），不用你手动导出两次：

```sh
wart --text WART --format fastfetch --out ~/logo.jsonc
# 同时写出 ~/logo.jsonc 和 ~/logo.ansi（配置指向的那个文件）
```

> ### ⚠️ `logo.source` 必须是绝对路径
>
> fastfetch 解析相对路径时，参照的是**它的当前工作目录**，**不是配置文件所在的目录**。所以从别的目录敲 `fastfetch`，相对路径就会失效——现象是 logo 变成一片 `/////` 之类的乱码，而不是报错。
>
> ```jsonc
> // ✗ 只有"恰好从该目录运行"时才有效
> "source": "logo.ansi"
>
> // ✓ 从任何目录运行都有效
> "source": "C:/Users/你/logo.ansi"
> ```
>
> **导出的配置里已经写的是绝对路径**，直接用就没问题。只有两种情况需要你手动改：
>
> - 把 `logo` 块手工合并进别的配置文件时（相对路径不会随之调整）
> - 移动了 logo 文件本身
>
> 路径用正斜杠 `/`——反斜杠在 JSON 字符串里是转义符，`\C`、`\w` 都不是合法转义，fastfetch 会因为这一条拒绝加载**整个配置**。Windows 同样接受正斜杠。

#### 放哪里

fastfetch 按这个顺序找配置：

| 平台 | 路径 |
|---|---|
| 通用 | `~/.config/fastfetch/config.jsonc` |
| Windows | `%APPDATA%\fastfetch\config.jsonc`、`%LOCALAPPDATA%\fastfetch\config.jsonc`、`C:\ProgramData\fastfetch\config.jsonc` |

把导出的文件改名成 `config.jsonc` 放进去，直接 `fastfetch` 就会用它。

#### 两种用法

**一、整份直接用**（推荐先这么试，确认效果）：

```sh
fastfetch --config ~/logo.jsonc
```

**二、合并进你现有的配置。** 如果你已经在用 fastfetch，把导出文件里的 `logo` 块整段复制进你 `config.jsonc` 的顶层即可，其余部分不动：

```jsonc
{
  "$schema": "...",

  // ← 从导出的文件里复制这一段
  "logo": {
    "type": "file",
    "source": "C:/Users/你/logo.ansi",
    "position": "left",
    "padding": { "top": 1, "left": 2, "right": 6 }
  },

  // 你原本的模块列表保持不动
  "modules": [ "title", "os", "kernel", "shell" ]
}
```

合并时注意 `source`：复制过去的就是绝对路径，照用即可；但如果你改了 logo 的位置，记得同步改这里。

#### 改什么

导出的配置里每一项的作用，以及实测可用的取值：

| 设置 | 作用 | 取值 |
|---|---|---|
| `logo.type` | logo 的来源类型 | `file` 按文本读并解析里面的 ANSI 转义（wart 要的就是这个）；`file-raw` 原样输出不解析 |
| `logo.source` | logo 文件路径 | **必须绝对路径**，原因见上面的警告框。用正斜杠，不要用反斜杠 |
| `logo.position` | logo 摆在信息栏哪一侧 | `left`、`right`、`top`（实测只有这三个合法） |
| `logo.padding.right` | logo 和信息栏之间的间距 | 数字。**嫌挤就调大这个**，另一个办法是去掉 `"right"` 那一项让它用默认值 |
| `logo.padding.top` | 顶部空行 | 数字。想让 logo 和上方终端内容拉开距离就调大 |
| `display.disableLinewrap` | 禁止 fastfetch 把过宽的 logo 折行 | `true`/`false`。logo 很宽时保持 `true` |
| `display.separator` | 「键: 值」中间的分隔符 | 任意字符串，比如 `" → "` |
| `modules` | 显示哪些信息、什么顺序 | 见下 |

#### 加减模块

`modules` 里每一项可以只是一个名字，也可以是一个对象（对象才能带参数）：

```jsonc
"modules": [
  "os",
  "kernel",

  // 对象形式：把标题行的图标换掉、上个色
  { "type": "title", "key": "  ", "keyColor": "green" },

  // 显示 CPU 温度
  { "type": "cpu", "temp": true },

  // 加一行自定义文字。注意 format 就是要打印的内容本身
  { "type": "custom", "key": "终端", "format": "WezTerm" },

  "memory",
  "uptime"
]
```

可用的模块名：`fastfetch --list-modules`。想看某个模块有哪些参数、或者要一份带全部默认值的完整配置：

```sh
fastfetch --gen-config-full full.jsonc
```

**一个容易踩的坑**：模块对象声明了 `additionalProperties: false`，所以**多写一个键会让整个模块被静默丢弃**——不报错，只是不显示。比如自定义行写成 `{ "type": "custom", "key": "终端", "format": "WezTerm", "text": "..." }` 就不会出现，因为 `custom` 模块没有 `text` 这个键（`format` 本身就是内容）。模块不显示时，先对照 `--gen-config-full` 的输出检查键名。

#### 常见调整

- **logo 太靠边或者太挤**：先动 `padding.right`（间距），再看 `position`（换一侧）
- **图标不显示，变成方块或问号**：终端没装 Nerd Font。wart 特意用的是 Nerd Font 的 **Mono** 变体（图标与字母等宽），换成别的变体可能还会让宽度对不齐
- **颜色不对/发灰**：终端没开真彩色。确认 `COLORTERM=truecolor`，Windows Terminal / WezTerm / Alacritty 默认都支持
- **LOGO 显示和预览不一样**：如果作品用了背景色（半方块模式，或开了「背景」），导出文件头部会有注释提醒——fastfetch 把 logo 当文本画，不一定渲染背景色。这时改用只设置前景色的样式重新导出

## 图标

应用图标来自 `doc/img/logo.png`，用在三个地方：

| 位置 | 怎么实现的 |
|---|---|
| exe 本身（资源管理器、开始菜单、固定到任务栏） | `assets/icon/icon.ico` 作为 **PE 资源**在构建时链入（`build.rs` + `embed-resource`）。Windows 只能从这里读，运行时加载不了 |
| 窗口标题栏 / 任务栏 | `ViewportBuilder::with_icon`，启动时解码 `assets/icon/icon.png` |
| 界面左上角标题旁 | 启动时上传成纹理，之后每帧绘制 |

`.ico` 是**多尺寸**的（16/24/32/48/64/128/256），Windows 会挑最接近的那个——比让它缩放一张大图清晰。

### 换 logo

1. **换图。** 把新图覆盖到 `doc/img/logo.png`。透明底最省事；如果拿到的是带纯色底的导出图，下一步加 `--key` 抠掉。

   脚本**不对图形本身做任何处理**：白边、投影、描边这类效果属于 artwork，做进母版里——那里能在大尺寸下判断，改起来是重画而不是调参数。母版里有什么，图标里就是什么。

2. **生成。**

   ```sh
   python tools/make-icons.py        # 写出 assets/icon/icon.png 和 icon.ico
   ```

   或者构建时顺带跑：`.\build.ps1 -Icons`，它在 `cargo build` 之前刷新图标，链进 exe 的就是新的。需要 Python 和 Pillow（`pip install pillow`）——只在换 logo 时用得上，不是构建依赖，所以没编进 cargo。

3. **看效果。**

   ```sh
   python tools/make-icons.py --preview target/icon.png   # 各尺寸 × 浅底/深底对照
   ```

   exe 的图标要**重新构建**才会变（它是 PE 资源，运行时读不到）；窗口/任务栏图标和界面左上角那个直接读 `icon.png`，构建后启动就是新的。资源管理器有时缓存旧图标，换个目录看图或重启 `explorer` 即可。

4. **提交** `assets/icon/icon.png` 和 `assets/icon/icon.ico`。脚本不会改 `doc/img/logo.png`。

### 生成参数

都有默认值，不改也能跑：

| 参数 | 默认 | 作用 |
|---|---|---|
| `--source` | `doc/img/logo.png` | 母版图，可以指向别处 |
| `--padding F` | `0.04` | 图形四周留白，占边长的比例 |
| `--key COLOR` | 无 | 先把某个纯色背景抠掉，如 `--key '#ffffff'` |
| `--key-tolerance` | `8` | 抠背景的容差 |
| `--sizes` | `16,24,32,48,64,128,256` | ICO 里包含哪些尺寸 |
| `--preview PATH` | 无 | 额外输出一张对照图 |

### 脚本做和不做的

**只取景，不处理。** 裁到墨迹、居中、按尺寸缩放，仅此而已——不碰颜色和轮廓。当前母版自带白边和投影，那是画进去的，`assets/icon/*` 直接继承。

**先裁到墨迹、再居中。** 母版的留白常常不对称，直接缩放会让图标偏一边。

**每个尺寸都从母版重采样**，而不是把 256px 那张缩下去，所以 16px 的图标不会有二次缩放的模糊。

**`--key` 走四角泛洪**，不是全局换色：图形内部与背景同色的高光（眼睛、亮部）不会被误伤。

## 字体

328 款 FIGlet 字体在构建时从 `assets/figlet/` 编进二进制，见 `build.rs`。字体集与 [patorjk 的 TAAG](https://patorjk.com/software/taag/) 对齐，所以你在那边挑的字体这里同名就有（`ANSI Shadow`、`Graffiti`、`Standard`……）。字体名大小写不敏感，`--font standard` 能找到 `Standard`。

GUI 里的字体下拉框可搜索；命令行 `wart --list-fonts` 会把内置和自定义的列出来。想加自己的字体，把 `.flf` 放到 exe 旁边的 `figlet\` 目录。

### 授权

- **`assets/figlet/`** —— 328 款字体来自 [patorjk/figlet.js](https://github.com/patorjk/figlet.js)，也就是 TAAG 背后的引擎。figlet.js 是 **MIT**（`assets/licenses/figlet-js-MIT.txt`）。各字体文件是第三方投稿，comment 块里标了作者，但**多数没有自己的授权声明**；这批字体可追溯到 FIGlet 发行版，而 Debian 把该发行版整体判定为 BSD-3-Clause。详情和「需要特殊解析的字体清单」见 [`assets/figlet/PROVENANCE.md`](assets/figlet/PROVENANCE.md)。
- **`assets/fonts/fusion-pixel-12px-monospaced-zh_hans.otf`** —— 缝合像素字体，**OFL-1.1**（`assets/licenses/fusion-pixel-OFL.txt`）。一个字体同时覆盖拉丁和简体中文；egui 0.36 通过 `skrifa` 光栅化，能处理它的 CFF 轮廓。
- **`assets/fonts/JetBrainsMonoNerdFontMono-Regular.ttf`** —— 来自 [ryanoasis/nerd-fonts](https://github.com/ryanoasis/nerd-fonts) v3.5.1。JetBrains Mono 是 **OFL-1.1**（`assets/licenses/JetBrainsMono-OFL.txt`），Nerd Fonts 的补丁脚本是 MIT（`assets/licenses/nerd-fonts-MIT.txt`）。特意用 **Mono** 变体：它的图标字形与字母**等宽**，所以图标列能和字母列对齐，也不会出现「图标双宽」导致的宽度计算错误。

[cmatsuoka/figlet](https://github.com/cmatsuoka/figlet) 的 **BSD-3-Clause 正典字体**也记录在 `assets/licenses/figlet-BSD3.txt`。如果你需要授权毫无争议的子集，那 18 款是有明确 BSD-3 授权的。

## 界面字体

界面用**缝合像素字体 12px 简中版**。它是像素字体，所以**所有界面字号都是它 12px 设计网格的整数倍**（定义在 `ui::theme`）——非整数倍会让轮廓偏离像素网格、渲染发虚。

作品预览**故意不用**这个字体，仍用 Nerd Font：私用区图标字形和统一字宽才是那里的重点。

系统 CJK 字体仍会作为兜底查找，因为像素字体覆盖汉字但不覆盖所有符号。

## 测试

```sh
cargo test
```

200 个测试，覆盖 FIGlet 流程、盲文位运算、渐变采样、颜色量化、形状匹配、六种导出器、图标搜索排序、翻译表完整性、图标尺寸与墨迹包围盒的对齐，以及字号与像素网格的倍数关系。
