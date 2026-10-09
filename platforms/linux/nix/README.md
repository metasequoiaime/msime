# Linux 宿主的 Nix 构建

仓库根目录的 `flake.nix` 只把这里的构建接到 flake 的各个输出上（它必须放在根目录，因为 Cargo workspace 和 `rust-toolchain.toml` 都在那里）。怎么在 NixOS 上用，见 [../README.md](../README.md) 的「Nix 与 NixOS」。

```sh
nix build .#msime-fcitx5     # 插件、Host API、命令行入口与设置窗口，构建中跑 ctest
nix build .#msime-ibus       # 同上，但不带 Fcitx5 插件，只跑装后检查
nix flake check              # 各个包、经 overlay 的构建和 NixOS 虚拟机测试（需要 KVM）
nix develop                  # 钉住的 Rust 工具链，CMake、Fcitx5、Tauri 的开发依赖
nix fmt                      # nixfmt
```

## flake 的输出

| 输出 | 内容 |
|---|---|
| `packages.<system>.msime-fcitx5`（默认） | Fcitx5 插件、IBus engine、`msime-linux-setup` 等命令、`msime-mcp`、provider 脚本与用户单元、设置窗口；`passthru.ibusEngine` 是交给 `i18n.inputMethod.ibus.engines` 的引擎包 |
| `packages.<system>.msime-ibus` | 同一份构建关掉 Fcitx5 插件（`enableFcitx5 = false`），只用 IBus 时取它，不依赖 `fcitx5` |
| `packages.<system>.msime-host-api` | 插件链接的 `libmsime_host_api.so` |
| `packages.<system>.msime-mcp` | MCP 服务程序 `msime-mcp`，由 `msime-fcitx5` 装进自己的前缀 |
| `packages.<system>.msime-desktop` | 设置窗口的 Tauri 二进制，由 `msime-fcitx5` 装进自己的前缀 |
| `packages.<system>.msime-handwriting-model` | 离线手写模型（Zinnia）与它的许可证 |
| `packages.<system>.msime-voice-runtime` | 本地语音识别用的 sherpa-onnx 与 ONNX Runtime 预编译库 |
| `packages.<system>.msime-resources` | 词库（默认不随包，见下） |
| `overlays.default` | 同一组包，按使用方的 nixpkgs 构建 |
| `nixosModules.default` | `programs.msime` |

`<system>` 是 `x86_64-linux` 与 `aarch64-linux`。插件被加载进 `fcitx5` 进程，应当和系统上的 Fcitx5 出自同一份 nixpkgs：直接取 `packages` 用的是本 flake 锁定的那份，经 overlay 或模块时用的是使用方自己的。

## 文件

| 文件 | 作用 |
|---|---|
| `default.nix` | 给定一套 nixpkgs，返回上面这些包和开发 shell；flake 的 `packages` 与 overlay 共用它 |
| `host-api.nix` | `msime-host-api`，crane 构建 |
| `mcp.nix` | `msime-mcp`，crane 构建 |
| `desktop.nix` | `msime-desktop`：前端加 crane 构建的 Tauri 外壳 |
| `rust-notices.nix` | 几个 Rust 产物静态链接的 crate 的许可证声明 |
| `pnpm-lock.nix` | 按 `pnpm-lock.yaml` 逐个下载前端依赖，改写锁文件供 pnpm 离线安装 |
| `fcitx5.nix` | `msime-fcitx5` 与 `msime-ibus`：`platforms/linux` 的 CMake 构建 |
| `locked-artifacts.nix` | 按 `resources/*.lock.json` 这类锁下载并核对 artifact，给词库和手写模型用 |
| `voice-runtime.nix` | `msime-voice-runtime`，取 `resources/voice-runtime.lock.json` 里本机架构的库 |
| `module.nix` | NixOS 模块 `programs.msime` |
| `module-test.nix` | 模块的虚拟机测试 |

## Rust：`msime-host-api`、`msime-mcp` 与 `msime-desktop`

编译器按 `rust-toolchain.toml` 取自 rust-overlay，不用 nixpkgs 自带的 rustc，与其他平台和本地门禁用同一个版本。三个包都用 crane：先 `buildDepsOnly` 编依赖，再编 workspace 里的 crate。依赖这一步只跑 `cargo build`（`cargoCheckCommand = "true"`，`doCheck = false`）：crane 默认还会先 `cargo check --all-targets` 再 `cargo test --no-run`，按 dev-dependencies 另编一套依赖，而这里只用得到 build 的产物。Rust 单测由 `scripts/verify-local.sh` 负责。

几个包的源码都只放 Cargo 用得到的部分（`default.nix` 里的 `cargoSources`），别的平台改动不触发重编。`msime-host-api` 和 `msime-mcp` 只取 `apps/desktop/src-tauri` 的 Cargo 清单和 `.rs`；`msime-desktop` 另外要图标、Tauri 配置与权限声明，不要 `gen/` 下的 Android 与 Xcode 工程。

`msime-desktop` 的步骤与 `platforms/linux/package-container.sh` 相同：

1. 前端：`pnpm --filter @msime/desktop exec vite build`。包的 `build` 脚本还会先跑 `tsc --noEmit`（连同测试），它不影响产物，交给 `verify-local.sh`；所以源码里也不放 `apps/desktop/tests`，改测试不必重编。
2. 把前端放到 `apps/desktop/dist`，`cargo build -p msime-desktop --features tauri/custom-protocol`。没有 `tauri/custom-protocol` 得到的是加载 `devUrl` 的开发版。`TAURI_CONFIG` 把应用报告的版本设为 `platforms/linux/version.txt`，应用内的更新检查才是同类相比。

前端构建要 nixpkgs 的 `pnpm_11`、`nodejs_24` 和 `writableTmpDirAsHomeHook`。经 overlay 或模块时用的是使用方的 nixpkgs，它没有这几个包时 `default.nix` 不带设置窗口（`settingsWindow` 为 `null`，并给出一条求值警告），而不是让原本能用的 `programs.msime.enable` 求值失败。overlay 里照样有 `msime-desktop`：overlay 的属性名不能取决于 `final` 里有什么，否则无限递归；它是惰性的，只有直接取用它时才失败。

### 第三方许可证声明

`THIRD_PARTY_NOTICES.txt` 指向 `rust-crates-NOTICES.txt` 和 `frontend-npm-NOTICES.txt`，与 deb、rpm 一样用 `platforms/linux/collect-notices.py` 生成，再经 `MSIME_RUST_NOTICES`、`MSIME_FRONTEND_NOTICES` 交给 CMake：

- `rust-notices.nix` 在 crane 按 `Cargo.lock` 准备的 vendor 目录上跑 `collect-notices.py cargo`，crate 与 `package-container.sh` 相同（`msime-host-api`、`msime-mcp-server`，带设置窗口时加 `msime-desktop:tauri/custom-protocol`）。
- 前端的 `pnpm install` 之后跑 `collect-notices.py npm`，结果放在前端的 `notices` 输出里，经 `msime-desktop` 的 `passthru.frontendNotices` 交给 `msime-fcitx5`。

CMake 只在打开 `MSIME_ENABLE_PACKAGING` 时强制要求这两份，Nix 构建不开它，漏传只有一条警告，所以装后检查核对它们装进了 `share/doc/msime-client`。

## 前端依赖：`pnpm-lock.nix`

不用 nixpkgs 的 `fetchPnpmDeps`：它把整个依赖目录当成一个固定输出，锁文件每变一次都得有人手动更新哈希，上游没有 Nix 的 CI 来提醒。这里改为锁文件驱动，与词库、手写模型的做法一致，不另记一份哈希：

1. 求值时逐行读 `pnpm-lock.yaml` 的 `packages:` 段，取每个包的 `integrity` 和 `os`、`cpu`、`libc` 限制。只是按 lockfile v9 的写法逐行匹配，不是通用的 YAML 解析，所以不需要 IFD。
2. 本机平台用得上的包各自 `fetchurl`（地址按 registry 的规则拼出，哈希就是锁文件里的 `integrity`）。平台不符的包不下载，pnpm 本来也会跳过它们。
3. 把这些包的 `resolution` 改写成 `{integrity: …, tarball: 'file:/nix/store/…'}`，`pnpm install --offline --frozen-lockfile` 从这些 tarball 装好。改写后的锁文件会被 pnpm 11 的供应链策略拒绝，所以设了 `pnpm_config_trust_lockfile`：每个 tarball 已由 `fetchurl` 按 `integrity` 核对过。

它只认来自 registry、`resolution` 只带 `integrity` 的包。锁文件里出现 git 依赖或 tarball 地址时求值直接失败并指出是哪个包，届时要扩展这个文件，而不是装出缺包的结果。它下载锁文件里所有本机能用的包，不按 workspace 成员裁剪：`pnpm install --filter '@msime/desktop...'` 仍要装根目录 importer 的依赖（`vite-plus` 等），而 `apps/harmony` 的依赖都在 `apps/desktop` 里，从这三个 importer 能到达的正好就是全部。每个包单独缓存，锁文件只改了几个包时只下载那几个。

## `msime-fcitx5`

本目录上一级的 CMake 构建，打开 `MSIME_ENABLE_FCITX5`。IBus engine 等其余入口照常一起构建和安装：顶层 CMake 把 IBus 列为必需，`msime-linux-setup`、`msime-linux-prepare` 也是 Fcitx5 首次配置要用的。构建时跑与门禁相同的 ctest（`doCheck`）。

`msime-ibus` 是它的 `override { enableFcitx5 = false; }`：关掉 `MSIME_ENABLE_FCITX5`，`fcitx5` 不进 `buildInputs`，包里没有 `lib/fcitx5` 和 `share/fcitx5`，`pname` 随之改成 `msime-ibus`。它不跑 ctest（`doCheck = enableFcitx5`）：少的只是 `fcitx5` 子目录，其余测试与 `msime-fcitx5` 完全相同；装后检查照跑。反过来去不掉 IBus：顶层 CMake 要求 `ibus-1.0`，在线、翻译、语音、剪贴板等 provider 程序都链接 IBus 宿主库（`msime-ibus-host`）。

可以 `override` 的参数，`default.nix` 默认传入前三个：

| 参数 | 默认 | 传 `null` 或不传时 |
|---|---|---|
| `settingsWindow` | `msime-desktop` | 没有 `msime-linux-settings` 与桌面入口 |
| `handwritingModel` | `msime-handwriting-model` | `msime-linux-handwriting --local` 报告没有安装模型 |
| `voiceRuntime` | `msime-voice-runtime`（锁里没有本机架构时为 `null`） | 本地识别不可用，云端识别不受影响 |
| `bundledResources` | 不传 | 不带词库，由用户首次配置时下载 |
| `enableFcitx5` | `true` | 不带 Fcitx5 插件，即 `msime-ibus` |

几处与其他打包路线不同的地方：

- **systemd 用户单元**装在 `lib/systemd/user`（`MSIME_SYSTEMD_USER_UNIT_DIR`）：NixOS 的 `systemd.packages` 只从 `lib/systemd/user` 与 `etc/systemd/user` 取，默认的 `share/systemd/user` 会被静默忽略。
- **脚本的解释器**：`postPatch` 为了让 ctest 直接执行源码树里的脚本，把它们的 shebang 改到构建用的 `python3` 与 `bash`，CMake 从那里装进 `bin` 的就是改写过的脚本，而 fixup 的 `patchShebangs` 不动已经指向 store 的 shebang。所以 `postInstall` 对 `$out/bin` 跑 `patchShebangs --update --host`，按 `buildInputs` 重新改写：Python 换成带 `websockets` 的那份（豆包流式识别要它的同步客户端），`sh`、`bash` 换成 `bashNonInteractive`。由 `.in` 模板经 `configure_file` 生成的脚本（`bin` 下的 `msime-linux-settings`，`share/msime-client/omarchy` 下的 `theme-set` 与 `menu`）不一样：模板不可执行，`postPatch` 不改它们，装出去的仍是 `#!/bin/bash` 这类系统路径，由 fixup 阶段自动执行的 `patchShebangs` 按 `buildInputs` 改到 `bashNonInteractive`。NixOS 上没有 `/bin/bash`，所以 `bashNonInteractive` 要在 `buildInputs` 里。
- **设置窗口**经 `MSIME_DESKTOP_BINARY` 交给 CMake，装成同一前缀下的 `msime-linux-desktop`。它按自己所在的前缀找 `msime-linux-setup`、手写模型和内置音效包，所以不能单独成包再链接过来。`wrapGAppsHook3` 默认会把 `bin` 下每个可执行文件都包一层，这里 `dontWrapGApps` 后只给它包：GTK 的运行环境、TLS 用的 `glib-networking`，以及剪贴板工具。包装后真正的二进制是 `.msime-linux-desktop-wrapped`，按 `current_exe` 找到的前缀不变。
- **`msime-mcp`** 经 `MSIME_MCP_BINARY` 交给 CMake，装在同一前缀的 `bin` 下：设置窗口的 MCP 页在自己旁边找它。这条路径在 `/nix/store` 里，升级和垃圾回收后会失效，所以写进助手配置的是 PATH 上 Nix profile 里解析后就是这个文件的链接（如 `/run/current-system/sw/bin/msime-mcp`），找不到时才写 store 路径，规则见 `crates/host-api/src/mcp_clients.rs` 的 `stable_command_in`。以前写进去的任一版 store 路径，设置页显示为已连接并提示更新，点「更新」换成 profile 链接，权限照旧。
- **`wl-clipboard` 与 `xclip`** 加在剪贴板监视器和设置窗口的 PATH 前面：两者在 Wayland 上读写剪贴板只经 `wl-copy`、`wl-paste`，在 X11 上只经 `xclip`、`xsel`，找不到时剪贴板历史什么也记不下，设置窗口的同步、复制与粘贴报告不可用。
- **IBus engine** 和 Fcitx5 插件出自同一次构建，组件文件装在 `share/ibus/component`，它的 `<exec>` 是本包 `bin` 下启动脚本的绝对路径。但本包不能直接放进 `i18n.inputMethod.ibus.engines`：nixpkgs 的 `ibus-with-plugins` 把引擎和 `ibus` 合成一个 `buildEnv`，要求 `lib/systemd/user` 只来自 `ibus`，本包的 provider 单元也在 `lib/systemd/user`，构建会失败；整包的 `bin` 也会混进 IBus 的环境。所以另给一个 `passthru.ibusEngine`（`msime-fcitx5` 与 `msime-ibus` 都有）：只有指回本包的组件文件，带着引擎类型要求的 `meta.isIbusEngine`。它取的是 `finalPackage`，`override` 过的包的 `ibusEngine` 指向的也是 override 之后的那份。
- **`msime-linux-setup` 包了一层**：它经 `gsettings` 把引擎写进 IBus 的输入源列表（`org.freedesktop.ibus.general`），而 IBus 的 schema 只在 `ibus` 自己的包装器和 GNOME 会话的 `XDG_DATA_DIRS` 里，Hyprland 这类会话找不到，注册会退回打印手动步骤。包装器把 `ibus` 的 schema 目录和 glib 的 `bin`（`gsettings`、`gdbus`）追加在 `XDG_DATA_DIRS`、`PATH` 后面，会话里已有的优先。真正的脚本是同目录下的 `.msime-linux-setup-wrapped`，它按自己所在的前缀找同级程序，不受影响。
- **音频工具不随包**：provider 取 PATH 上找到的第一个（`parec`、`pw-cat`、`arecord`），带上 PulseAudio 的工具会让只有 PipeWire、没开 pipewire-pulse 的系统选到连不上的 `parec`，所以交给系统的音频栈。

ctest 跑的是构建目录，看不到装出去的东西能不能加载，所以 fixup 之后还有装后检查（`installCheckPhase`）：插件和 `msime-linux-ibus` 按 RUNPATH 找到的 Host API 必须是本包 `lib/msime-client` 里的那份，IBus 组件文件的 `<exec>` 指向本包里能执行的启动脚本，`msime-linux-setup` 包装后还能运行、带着 IBus 的 schema；语音运行库同理，它的依赖都要能单独解析，并且 `msime-voice-local` 能打开它；设置窗口的依赖在收缩 RUNPATH 后都还解析得到，包装器带着 TLS 模块和 `xclip`；`msime-mcp` 能运行；两份第三方许可证声明都装进来了。

### 随包词库

`msime-fcitx5.override { bundledResources = msime-resources; }` 把 `resources/desktop-dictionary.lock.json` 钉住的词库装进包内的 `share/msime-client/resources`，旁边的 `share/doc/msime-resources` 带着逐项列出词库来源与上游条款的 `msime-engine-dictionary-NOTICE.md`。默认不这样做：首次配置会把 store 里的词库目录记进用户的 `runtime-options.json`，词库锁不变时重新构建不会刷新这条记录，旧路径被垃圾回收后输入法就找不到词库。

## 锁文件驱动的下载

词库、手写模型和语音运行库都直接按 `resources/` 下锁文件里的地址和 SHA-256 下载（`locked-artifacts.nix`、`voice-runtime.nix`），前端依赖按 `pnpm-lock.yaml` 的 `integrity` 下载（`pnpm-lock.nix`）。Nix 文件里不另记哈希：更新这些依赖只改锁文件，各条打包路线（deb、rpm、Arch、Gentoo、Nix）读的是同一份。验证尽量写成 `platforms/linux/CMakeLists.txt` 里的 ctest，让每条打包路线都跑；Nix 的装后检查只管安装布局与 fixup 之后才能确认的事。

## NixOS 模块

`programs.msime.enable` 把包加进 `i18n.inputMethod.fcitx5.addons`、把它的 `ibusEngine` 加进 `i18n.inputMethod.ibus.engines`（并以比 `mkDefault` 低一级的优先级打开 Fcitx5，GNOME 以 `mkDefault` 设的 `ibus` 会胜出；nixpkgs 只读 `i18n.inputMethod.type` 选中的那一个）。包的默认值跟着 `type` 走：`ibus` 时是 `msime-ibus`，其余是 `msime-fcitx5`；`type` 是 `fcitx5` 而包不带插件时断言失败。包同时加进 `environment.systemPackages`，经 `systemd.packages` 注册用户单元。NixOS 不理会包里单元的 `[Install]` 段，所以模块照单元文件（`platforms/linux/data`）里的 `WantedBy` 给出 `wantedBy`：两个 socket 随 `sockets.target`，剪贴板监视器随 `graphical-session.target`。

## 检查

`nix flake check` 构建各个包（构建本身跑 ctest 和装后检查，`msime-ibus` 只跑装后检查），另外经 overlay 再求值一次 `msime-fcitx5`：不用模块的配置经 overlay 取包，`callPackage` 能从 `pkgs` 里取到这几个包，只查 `packages` 发现不了参数被它们自动填上的问题。

`nixos-module` 起一台虚拟机（需要 KVM）：

- linger 的用户管理器里两个 socket 就位，连上后 systemd 拉起在线与语音服务；语音服务对一个配置测试请求作出应答，说明它的启动走完了。
- 语音服务脚本的解释器能导入豆包要的 `websockets`。
- 自动登录的 X 会话（IceWM）里，剪贴板监视器由 `graphical-session.target` 拉起；从 PATH 上的 `msime-linux-settings` 打开设置窗口，等窗口与 WebKit 的网页进程出现，再用 OCR 等首次配置页上由后端填进的配置目录或云候选服务的域名：缺了 `tauri/custom-protocol` 或前端是空的时，窗口和网页进程照样出现，只有页面上的字能说明嵌入的前端加载出来了。截图留在测试输出里（`result/settings-window.png`）。

另一台虚拟机把 `i18n.inputMethod.type` 设为 `ibus`，以 alice 起 `ibus-daemon`（PATH 上的是模块合成的 `ibus-with-plugins`），等 `ibus list-engine` 列出 `msime-linux`：它只从合成环境里的 `share/ibus/component` 读组件，列出来就说明引擎包进了这个环境、`ibus-with-plugins` 也构建得出来。再核对 PATH 上的 `msime-linux-setup` 来自 `msime-ibus`，那个前缀下没有 `lib/fcitx5`。

它只注册在 x86_64 上：`flake check` 只构建本机系统的 checks，却会求值所有系统的，多一个系统就多求值一整套 NixOS（约 9 秒、600 MB）。这台虚拟机核对的是模块接线，与架构无关；aarch64 特有的部分由那边的包构建里的 ctest 和装后检查覆盖。

## 开发 shell

`nix develop` 带着钉住的 Rust 工具链，以及 `msime-host-api`、`msime-desktop`、`msime-fcitx5` 的构建依赖（CMake、Fcitx5、IBus、GTK、WebKit 等），`scripts/verify-local.sh` 在里面能跑 Linux 原生宿主与 Tauri 外壳的编译门禁。
