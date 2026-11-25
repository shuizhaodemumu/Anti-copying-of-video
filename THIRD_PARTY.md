# 项目来源与第三方开源致谢

## 项目来源与 Fork 关系

Anti-copying of video 是从「加密内容在实体屏幕上播放后，手机翻拍时会看到什么」这个问题发展出的独立实验。相关项目为同一作者的 Encrypt your digital assets。两个项目均采用 MIT 许可证。本仓库独立建库，没有 GitHub Fork 上游。60 Hz 两相显示、九宫格排布、文字样页和字幕样片的实现均在本仓库中。[相关项目与版本](https://github.com/shuizhaodemumu/Encrypt-your-digital-assets/releases/tag/windows-online-v2.7.1) · [该版本的 MIT 许可证](https://github.com/shuizhaodemumu/Encrypt-your-digital-assets/blob/98071917c122d982abd4e031ad932f49ce4dc058/LICENSE) · [本项目许可证](LICENSE)

## 播放器使用的开源项目

| 项目 | 在 0.4.1 中的用途 | 许可证 |
| --- | --- | --- |
| [gtk4-rs](https://github.com/gtk-rs/gtk4-rs) | 直接依赖 `gtk4 0.9.7`，使用 GTK 4.8 API，构建窗口、接收帧时钟回调、显示纹理 | MIT |
| [gtk-rs-core](https://github.com/gtk-rs/gtk-rs-core) | GTK Rust 绑定所用的底层 Rust 封装，包括 GLib 和 GIO | MIT |
| [GTK](https://github.com/GNOME/gtk#licensing-terms)、[GLib](https://github.com/GNOME/glib) | Fedora 提供的原生共享库。可执行文件链接 GTK 4、GLib、GObject 和 GIO | GTK 主体及 GLib 为 LGPL-2.1-or-later，GTK 内部个别组件另有条款 |
| [image](https://github.com/image-rs/image) | 直接依赖 `image 0.25.10`，关闭默认功能，仅开启 `jpeg`。读取静态页面与短片逐帧 JPEG，并处理单通道字形遮罩 | MIT OR Apache-2.0 |
| [zune-image](https://github.com/etemesi254/zune-image) | `image` 使用的 JPEG 解码器，涉及 `zune-jpeg 0.5.15` 和 `zune-core 0.5.3` | MIT OR Apache-2.0 OR Zlib |

## 演示素材、字体及制作工具

文字页《渡口日志》、河谷场景、中文字幕和五字遮罩由本项目的 Python 脚本绘制。仓库自带生成结果，运行应用或重新编译 Rust 程序都可直接使用它们。[页面与视频生成脚本](tools/generate_assets.py) · [字形遮罩生成脚本](tools/generate_latent_mask.py)

| 项目 | 用途 | 许可证 |
| --- | --- | --- |
| [Pillow](https://github.com/python-pillow/Pillow/blob/main/LICENSE) | 绘制、缩放、排字和 JPEG 编码 | MIT-CMU，具体条款见上游 LICENSE |
| [Fontconfig](https://gitlab.freedesktop.org/fontconfig/fontconfig) | 素材脚本通过系统的 `fc-match` 查询字体文件与字形集合索引 | Fontconfig 的宽松授权，见上游 COPYING |
| [Noto CJK](https://github.com/notofonts/noto-cjk) | Noto Sans CJK SC 绘制中文正文、字幕和暗字，Noto Serif CJK SC 绘制主标题 | SIL Open Font License 1.1 |
| [Noto Sans Mono](https://github.com/notofonts/latin-greek-cyrillic) | 绘制核对码和场景中的数字 | SIL Open Font License 1.1 |

在发布环境中重新运行两个脚本得到的三份素材，与 0.4.1 归档中的文件逐字节一致。素材已把字形绘制为像素，源码包和 RPM 均不含字体文件。窗口状态栏的 CSS 首选 IBM Plex Mono，系统也可使用自己的等宽回退字体。自行重生成素材时，Fontconfig 的实际匹配结果取决于所安装的字体。[IBM Plex](https://github.com/IBM/plex) · [界面字体设置](src/photo60.css)

单独发布的演示 MP4 从程序纹理生成，使用 Pillow 加入文字说明，再由 FFmpeg 调用 libx264 压制为 H.264。编码器和字体文件均未随 MP4 或本项目的 RPM 分发。[FFmpeg 与组件许可证](https://ffmpeg.org/legal.html) · [x264 项目](https://www.videolan.org/developers/x264.html)

## 完整 Rust 依赖与许可证

Fedora x86_64 的正常依赖和构建依赖合计 68 条第三方包版本记录，同一 crate 的不同版本分别计数。安装 RPM 后，`/usr/share/licenses/photo60-test/crate-notices/` 下有逐包的上游许可文件和 `packages.tsv`。打包脚本按照目标平台的 Cargo 元数据收集 70 条第三方记录，其中还包括两条未出现在下列实际依赖树中的可选记录。[Cargo.lock](Cargo.lock) · [打包时的收集脚本](packaging/crate-notices.py)

以下命令可在仓库根目录复核该目标平台的正常依赖、构建依赖及版本。

```sh
cargo tree --locked --target x86_64-unknown-linux-gnu -e normal,build
```

<details>
<summary>展开 Fedora x86_64 的 68 条第三方包版本记录，按上游项目归类</summary>

| 上游 | Crate 与版本 | 上游声明的许可证 |
| --- | --- | --- |
| [moxcms](https://github.com/awxkee/moxcms) | `moxcms 0.8.1` | `BSD-3-Clause OR Apache-2.0` |
| [pxfm](https://github.com/awxkee/pxfm) | `pxfm 0.1.30` | `BSD-3-Clause OR Apache-2.0` |
| [bitflags](https://github.com/bitflags/bitflags) | `bitflags 2.13.2` | `MIT OR Apache-2.0` |
| [proc-macro-crate](https://github.com/bkchr/proc-macro-crate) | `proc-macro-crate 3.5.0` | `MIT OR Apache-2.0` |
| [memchr](https://github.com/BurntSushi/memchr) | `memchr 2.8.3` | `Unlicense OR MIT` |
| [target-lexicon](https://github.com/bytecodealliance/target-lexicon) | `target-lexicon 0.13.5` | `Apache-2.0 WITH LLVM-exception` |
| [autocfg](https://github.com/cuviper/autocfg) | `autocfg 1.5.1` | `Apache-2.0 OR MIT` |
| [rust-field-offset](https://github.com/Diggsey/rust-field-offset) | `field-offset 0.3.6` | `MIT OR Apache-2.0` |
| [rustc-version-rs](https://github.com/djc/rustc-version-rs) | `rustc_version 0.4.1` | `MIT OR Apache-2.0` |
| [proc-macro2](https://github.com/dtolnay/proc-macro2) | `proc-macro2 1.0.107` | `MIT OR Apache-2.0` |
| [quote](https://github.com/dtolnay/quote) | `quote 1.0.47` | `MIT OR Apache-2.0` |
| [semver](https://github.com/dtolnay/semver) | `semver 1.0.28` | `MIT OR Apache-2.0` |
| [syn](https://github.com/dtolnay/syn) | `syn 2.0.119`, `syn 3.0.10` | `MIT OR Apache-2.0` |
| [unicode-ident](https://github.com/dtolnay/unicode-ident) | `unicode-ident 1.0.27` | `(MIT OR Apache-2.0) AND Unicode-3.0` |
| [cfg-expr](https://github.com/EmbarkStudios/cfg-expr) | `cfg-expr 0.20.10` | `MIT OR Apache-2.0` |
| [zune-image](https://github.com/etemesi254/zune-image) | `zune-core 0.5.3`, `zune-jpeg 0.5.15` | `MIT OR Apache-2.0 OR Zlib` |
| [system-deps](https://github.com/gdesmott/system-deps) | `system-deps 7.0.8` | `MIT OR Apache-2.0` |
| [memoffset](https://github.com/Gilnaa/memoffset) | `memoffset 0.9.1` | `MIT` |
| [gtk-rs-core](https://github.com/gtk-rs/gtk-rs-core) | `cairo-rs 0.20.12`, `cairo-sys-rs 0.20.10`, `gdk-pixbuf 0.20.10`, `gdk-pixbuf-sys 0.20.10`, `gio 0.20.12`, `gio-sys 0.20.10`, `glib 0.20.12`, `glib-macros 0.20.12`, `glib-sys 0.20.10`, `gobject-sys 0.20.10`, `graphene-rs 0.20.10`, `graphene-sys 0.20.10`, `pango 0.20.12`, `pango-sys 0.20.10` | `MIT` |
| [gtk4-rs](https://github.com/gtk-rs/gtk4-rs) | `gdk4 0.9.6`, `gdk4-sys 0.9.6`, `gsk4 0.9.6`, `gsk4-sys 0.9.6`, `gtk4 0.9.7`, `gtk4-macros 0.9.5`, `gtk4-sys 0.9.6` | `MIT` |
| [byteorder-lite](https://github.com/image-rs/byteorder-lite) | `byteorder-lite 0.1.0` | `Unlicense OR MIT` |
| [image](https://github.com/image-rs/image) | `image 0.25.10` | `MIT OR Apache-2.0` |
| [equivalent](https://github.com/indexmap-rs/equivalent) | `equivalent 1.0.3` | `Apache-2.0 OR MIT` |
| [indexmap](https://github.com/indexmap-rs/indexmap) | `indexmap 2.14.2` | `Apache-2.0 OR MIT` |
| [bytemuck](https://github.com/Lokathor/bytemuck) | `bytemuck 1.25.2` | `Zlib OR Apache-2.0 OR MIT` |
| [futures-rs](https://github.com/rust-lang/futures-rs) | `futures-channel 0.3.34`, `futures-core 0.3.34`, `futures-executor 0.3.34`, `futures-io 0.3.34`, `futures-macro 0.3.34`, `futures-task 0.3.34`, `futures-util 0.3.34` | `MIT OR Apache-2.0` |
| [hashbrown](https://github.com/rust-lang/hashbrown) | `hashbrown 0.17.1` | `MIT OR Apache-2.0` |
| [libc](https://github.com/rust-lang/libc) | `libc 0.2.191` | `MIT OR Apache-2.0` |
| [pkg-config-rs](https://github.com/rust-lang/pkg-config-rs) | `pkg-config 0.3.34` | `MIT OR Apache-2.0` |
| [num-traits](https://github.com/rust-num/num-traits) | `num-traits 0.2.19` | `MIT OR Apache-2.0` |
| [smallvec](https://github.com/smallvec/smallvec) | `smallvec 1.16.6` | `MIT OR Apache-2.0` |
| [pin-project-lite](https://github.com/taiki-e/pin-project-lite) | `pin-project-lite 0.2.17` | `Apache-2.0 OR MIT` |
| [slab](https://github.com/tokio-rs/slab) | `slab 0.4.12` | `MIT` |
| [toml](https://github.com/toml-rs/toml) | `serde_spanned 1.1.2`, `toml 1.1.8+spec-1.1.0`, `toml_datetime 1.1.2+spec-1.1.0`, `toml_edit 0.25.17+spec-1.1.0`, `toml_parser 1.1.5+spec-1.1.0` | `MIT OR Apache-2.0` |
| [winnow](https://github.com/winnow-rs/winnow) | `winnow 1.0.4` | `MIT` |
| [heck](https://github.com/withoutboats/heck) | `heck 0.5.0` | `MIT OR Apache-2.0` |
| [version-compare](https://gitlab.com/timvisee/version-compare) | `version-compare 0.2.1` | `MIT` |

</details>

元数据收集还包含 `serde_core 1.0.229`、`toml_writer 1.1.3+spec-1.1.0`。锁文件另外记录了 `serde_derive` 和 Windows 目标包；这些条目允许 Cargo 解析其他平台或功能组合。
