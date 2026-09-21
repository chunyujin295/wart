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

## 两种生成模式

### FIGlet —— 经典大字

从 `.flf` 字体渲染多行横幅。smushing（字符挤压）、kerning、hardblank 的处理都遵循字体文件自己的头部声明。换行会堆叠成多个独立横幅。

**字体画不出来的字符怎么处理。** `.flf` 本质是一张「字符码 → ASCII 图案」的查找表，条目都是 ASCII——所以一个 Nerd Font 图标**没法被 FIGlet 字体画出来**，表里根本没有它。wart 的做法是把这些字符**用点阵模式那套引擎光栅化**，按大字的行高绘制在大字旁边：

```
            __          __     _____ _______
 ,g@WWW&p   \ \        / /\   |  __ \__   __|
gW} @@p ]$p  \ \  /\  / /  \  | |__) | | |
@`  7W[  `@   \ \/  \/ / /\ \ |  _  /  | |
$_]@@@@@L_@    \  /\  / ____ \| | \ \  | |
{Wp]MMM[gW}     \/  \/_/    \_\_|  \_\ |_|
  ]MWWWM[
```

细节随字体行数缩放：`big`、`Standard` 这类 6 行字体能画出可辨认的图标，`mini` 这种 3 行的就不行了——格子数不够。

### 点阵 —— 光栅化

用真实字体光栅化文字，再把像素映射成字符。要画 Nerd Font 图标和真正的逐字符渐变，靠的是这个模式。

| 字符集 | 每格源像素 | 说明 |
|---|---|---|
| 盲文 | 2×4 | 分辨率最高。点是分离的（盲文本来就是给触觉阅读设计的），所以实心区域看起来是穿孔的 |
| 形状匹配 | 4×8 | 每格挑一个「画出来最像」的可打印 ASCII 字符。适合图像和图标 |
| 四分之一方块 | 2×4 | 每格四个子块，靠字符自身的空隙提供透明，**不需要背景色** |
| 半方块 | 1×2 | 每格两个可独立着色的像素。需要终端支持背景色 |
| 线画 | 1×2 | 按边缘方向选 `/ \ \| _ -` |
| 方块 | 1×2 | `░▒▓█` 五档深浅 |
| ASCII 渐变 | 1×2 | `" .:-=+*#%@"`，六种里最粗的 |

**形状匹配**把 95 个可打印 ASCII 字符逐个光栅化，每个归约成「八个子区域的墨迹分布」向量；图像每一格也归约成同样的向量，取最近的。方法来自 Alex Harri 的 [*ASCII characters are not pixels*](https://alexharri.com/blog/ascii-rendering)。密度梯度问的是「这格多暗」，形状匹配问的是「这格长什么样」——后者才是对的问题。

## 图标样式

大字旁边的图标（以及任何 FIGlet 字体画不出的字符）怎么画。图标按 **1.5 倍大字高度**绘制，略微高出，读起来像一个刻意的徽标。

| `--icon-style` | 用什么画 | 每格子单元 | 说明 |
|---|---|---|---|
| `shape`（默认） | 可打印 ASCII，按形状选 | 1 | ASCII 艺术质感，和大字同一媒介，而且可辨认 |
| `braille` | 2×4 点 | **8** | 形状分辨率最高、最锐利，但点阵质感明显不是大字的媒介 |
| `blocks` | `░▒▓█` | 1 | 实心剪影。抖动纹理让它比「单子单元」听起来更有细节 |
| `quadrant` | `▘▝▖▗▚▞▛▜▙▟` | **4** | 比半方块细一倍，且不需要背景色 |
| `halfblock` | `▀ ▄ █`，两半各一色 | 2 | **单色图形里最粗的一种**，见下 |
| `lineart` | `/ \ \| _ -` | 1 | 笔画词汇和大字一致，但笔画每格携带的信息远少于点阵，需要约 18 列才看得清 |

**关于半方块。** 它「最细腻」的名声说的是**颜色**（每格两个独立着色的像素），不是形状。图标是单色图形，比的是形状分辨率，而每个半方块字符只有 **2** 个子单元，盲文有 **8**——四倍差距。更糟的是渐变通常是横向的，同一格上下两半颜色几乎相同，`▀` 的前景/背景之分随之消失，它唯一的颜色优势也用不上。只在渲染**真正的多色图像**、且终端支持背景色时才值得用。

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

| 格式 | 扩展名 | 用途 |
|---|---|---|---|
| ANSI 文本 | `.ansi` | 带转义序列，`fastfetch --file` / 终端 `cat`。注意扩展名不是 `.txt`——**满是转义序列的文件本来就不是纯文本** |
| 纯文本 | `.txt` | 只有字符、没有任何转义序列。给会把 `ESC[38;2…` 当字面量打出来的地方：commit message、README、聊天窗口、源码注释 |
| fastfetch 配置 | `.jsonc` | 指向 ANSI 文件的配置，含那些不好查的设置 |
| Neovim Lua | `.lua` | dashboard-nvim / snacks.nvim / alpha.nvim 的高亮组 |
| PNG | `.png` | 分享、README 配图 |
| HTML / SVG | `.html` / `.svg` | 浏览器、矢量输出 |

同一个作品两种文本格式的体量差别很直观——带边框的 `WART`：

```
纯文本    782 字节，   0 处转义序列
ANSI     5847 字节， 339 处转义序列
```

`--format ansi` 配 `--no-color` 也能得到不带颜色的输出，但那是"上色之后又擦掉"；`--format plain` 是直接不产生颜色，而且不会因为参数写错而漏出转义序列。

### 给 fastfetch 用

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

改图标时重新生成这两个文件即可：`.ico` 供 exe 用，256px 的 `.png` 供运行时用（原图 1254px，全尺寸解码出来是 6MB，而它从来不会被画得比几十像素大）。

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

177 个测试，覆盖 FIGlet 流程、盲文位运算、渐变采样、颜色量化、形状匹配、六种导出器、图标搜索排序、翻译表完整性，以及字号与像素网格的倍数关系。
