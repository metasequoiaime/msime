# Agent Note: Android 瘦包用 RELR 重定位和无损 WebP 图标压回预警线以下

Status: implemented

## Problem

GitHub 渠道的 full 和拼音瘦包在 2026-11-04 的 nightly（`24852caa38`）里是 133,264,819 字节，越过 `build-apk.sh` 132,000,000 字节的预警线，离 135,000,000 字节的硬上限只剩约 1.7 MB（#10810）。这一轮的增量是自然增长：启动器图标加了圆形版本、前景图加大，PNG 合计多了约 581 KB；`libmsime_host_api.so` 多了约 236 KB。维护者决定不提高上限，先瘦身，把体积压回预警线以下（[#10810 的决定](https://github.com/metasequoiaime/msime/issues/10810#issuecomment-6650507635)），上限照旧挡整组按需资源回到包里（见 [develop 红灯自动开单和 APK 体积预警](../process/2026-10-28-ci-red-tracking.md)）。

按 APK 条目量过，最大的几项是未压缩存放的 `libmsime_host_api.so`（37.9 MB）、压缩后的词库、未压缩存放的 dex（18.8 MB）和 `resources.arsc`。原生库已经去掉了调试信息和符号表，词库的 deflate 已经比 zlib 9 级还紧。原生库里有一块是纯粹的格式开销：五万三千多条 `R_AARCH64_RELATIVE` 按 RELA 每条 24 字节存，`.rela.dyn` 占 1.27 MB。启动器图标是 100 张 PNG，合计约 770 KB。

## Decision

两处改动，都不改变行为和画面：

- `build-native.sh` 链接 `libmsime_host_api.so` 时加 `-Wl,--pack-dyn-relocs=relr -Wl,--use-android-relr-tags`，相对重定位压成 RELR 位图。`.rela.dyn` 从 1,276,824 字节降到 2,928 字节，新增的 `.relr.dyn` 只有 15,960 字节，库小 1,261,464 字节。加载时系统链接器照样把它们全部重定位，代码和数据的地址不变。
- 启动器图标改存无损 WebP：`generate_app_icons.py` 先用 rsvg-convert 出 PNG，再用 `cwebp -lossless -exact -z 9` 重编码，删掉旧 PNG。100 张图从 771,623 字节降到 314,164 字节。

同一提交、同一台机器（Mac Studio，开发密钥签名）上 full 瘦包从 133,268,915 字节降到 131,597,843 字节，少 1,671,072 字节，比预警线低约 402 KB。

### RELR 只能用 Android 专用标签

标准的 `DT_RELR`（0x24）要 API 30 的链接器才认。Android 9 和 10（API 28、29）的链接器只认 API 28 起支持的 `DT_ANDROID_RELR`（0x6fffe000），用标准标签的库在这两个版本上加载失败，键盘直接起不来，而 minSdk 是 28。所以必须带 `--use-android-relr-tags`。`verify-native.sh` 检查 `libmsime_host_api.so` 有 `DT_ANDROID_RELR`、没有标准的 `DT_RELR`，Gradle 合并 jniLibs 前也跑这一步，所以漏了参数或换成标准标签都打不出包。

只用 `relr`，不用 `android+relr`。后者再把剩下的非相对重定位压成 Android APS2 格式，这个库里只剩四百来条（`ABS64`、`GLOB_DAT`、`JUMP_SLOT`），省不了几 KB，却多叠一种格式。只用 RELR 时其余重定位照旧是普通 RELA。

`libmsime_android.so` 只有 124 条相对重定位，没有改；`libc++_shared.so` 和 ML Kit 的 `libdigitalink.so` 是预编译的。

### 图标逐像素相同

100 张 WebP 解码后的 RGBA 与原来的 PNG 逐字节相同（`dwebp` 与 ImageMagick 解出的原始像素做哈希比较）。本机 librsvg 2.63.2 重跑生成脚本，结果也和仓库里原来的 PNG 逐像素相同，说明生成流程没有漂移。在 API 28 和 35 的模拟器上切换五套配色截取启动器图标，改前改后每格只有五十到一百多个像素差 1/255，来自 PNG 和 WebP 两条解码路径的取整，肉眼看不出。

## Alternatives considered

- **提高上限和预警线**：改两个数字最省事。维护者明确不采用，上限挡的是整组按需资源回到包里。
- **打开 R8 缩减 dex**：dex 有 18.8 MB，`isMinifyEnabled = false`，宿主自己的类约 3,900 个，其余一万四千多个是 Material、Play 服务、ML Kit、Credential Manager 等库，单靠缩减库代码就可能省下十 MB 级别，是最大的一项。这次不做：宿主经反射按名字取 `BuildConfig`、`FileProvider`、Fragment 和 JNI 回调类，C++ 按名字回调 Java，每一条都要写 keep 规则，还要在设备上把用到库代码的每条路径重新验收一遍；这次的余量用不着冒这个风险。值得单独开 issue 做。
- **dex 或原生库改成压缩存放**：APK 能小十几 MB，但安装时要解压到 `/data`，装好后多占同样大小的空间，冷启动也慢。这是拿装机后的体积和速度换下载体积，不做。
- **链接器 `--icf=safe`**：在 Studio 上只重链宿主库试过，体积一个字节没变，lld 一个函数也没有合并（推测是 Rust 的目标文件不带地址重要性表，没有核实）。`--icf=all` 会合并 C++（静态链进来的 ONNX Runtime）依赖地址唯一性的函数，不安全。
- **去掉 v1 签名**：`MANIFEST.MF` 和 `.SF` 压缩后约 115 KB，minSdk 28 的设备只验 v2/v3。不确定第三方分发渠道是否还查 v1，收益又小，这次不动。
- **把用不到的普通图标位图删掉**：minSdk 28 时自适应图标总是优先，`app_icon_<style>.png` 和 `_round.png` 在设备上不会被画出来，`res/SOURCE.md` 说明它们是给浏览仓库的人看的。改成 WebP 后它们合计只有百来 KB，留着。

## Consequences

- 收益：瘦包回到预警线以下约 400 KB，离硬上限约 3.4 MB；每个版本的 APK 都同样小了约 1.67 MB（非瘦包也用同一个原生库和同一批图标）。
- 代价：余量不大，按最近几次 nightly 每天几百 KB 的自然增长，几周内会再次越过预警线；到时候要么做上面的 R8，要么把更多数据改成资源包。
- 重新生成图标要装 webp（`brew install librsvg webp`）。
- 验证：API 28（`smoke.sh --core`）和 API 35（完整 `smoke.sh`）模拟器装包验收，五套图标在两个级别上改前改后截图对比，`verify-native.sh` 的新检查用 nightly 原来的库、标准标签的库和 Android 标签的库各跑一次，前两个失败、第三个通过。
