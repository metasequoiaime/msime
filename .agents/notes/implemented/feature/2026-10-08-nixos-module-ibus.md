# Agent Note: Nix 的 IBus 接入：msime-ibus 包与模块按 i18n.inputMethod.type 选包

Status: implemented

## Problem

`msime-fcitx5` 一直连 IBus engine 一起构建和安装（`share/ibus/component/msime-linux.xml`，`<exec>` 指向本包 `bin` 下的启动脚本），但 `programs.msime` 只把包放进 `i18n.inputMethod.fcitx5.addons`，`i18n.inputMethod.type` 不是 `fcitx5` 时只给一条警告。用 GNOME 默认输入源（IBus）的 NixOS 用户因此用不上水杉。

把包直接放进 `i18n.inputMethod.ibus.engines` 行不通，两处都会挡住：

- 引擎的选项类型要求 `meta.isIbusEngine = true`。
- nixpkgs 的 `ibus-with-plugins` 把引擎和 `ibus` 合成一个 `buildEnv`，postBuild 要求 `lib/systemd/user` 是指回 `ibus` 的链接。本包的 provider 单元也装在 `lib/systemd/user`（NixOS 的 `systemd.packages` 只认这里），合成后它成了真目录，构建以 `File …/lib/systemd/user does not point to to a file in …ibus-1.5.34` 失败。

## Decision

- `platforms/linux/nix/fcitx5.nix` 改用 `finalAttrs`，给包加 `passthru.ibusEngine`：一个 `runCommand`，只把 `finalAttrs.finalPackage` 的 `share/ibus/component/*.xml` 逐个链接到自己的 `share/ibus/component`，`meta.isIbusEngine = true`。组件里的 `<exec>` 是绝对路径，引擎进程仍从完整的包里起。取 `finalPackage`，所以 `override` / `overrideAttrs` 之后的 `ibusEngine` 指向改过的那份。
- `fcitx5.nix` 加参数 `enableFcitx5 ? true`，接到 `MSIME_ENABLE_FCITX5`；关掉时 `fcitx5` 不进 `buildInputs`，`pname` 改为 `msime-ibus`，`passthru.enableFcitx5` 记下这一点。`default.nix` 导出 `msime-ibus = msime-fcitx5.override { enableFcitx5 = false; }`，flake 的 `packages`、`overlays.default` 和 `checks` 都带上它，经 overlay 的用户可以只装 IBus 一侧。只能单向裁剪：顶层 CMake 要求 `ibus-1.0`，provider 程序都链接 IBus 宿主库 `msime-ibus-host`，所以没有「只要 Fcitx5」的变体。
- 装后检查对两个变体都核对 `msime-linux-ibus` 按 RUNPATH 找到本包的 Host API、组件 `<exec>` 指向本包里可执行的启动脚本；插件的那项只在 `enableFcitx5` 时检查。
- `platforms/linux/nix/module.nix` 的 `package` 默认值跟着 `i18n.inputMethod.type`：`ibus` 时是 `msime-ibus`，其余是 `msime-fcitx5`。`type` 不依赖 `package`，不会循环。`type` 是 `fcitx5` 而包的 `enableFcitx5` 为假时断言失败，不让用户静默得到一个没有插件的系统。
- 模块同时填 `i18n.inputMethod.fcitx5.addons = [ cfg.package ]` 和 `i18n.inputMethod.ibus.engines = [ cfg.package.ibusEngine ]`，`type` 仍默认设为 `fcitx5`，但用 `mkOverride 1001`，比 `mkDefault` 低一级：nixpkgs 的 `gnome.nix` 以 `mkDefault` 设 `type = "ibus"`，`type` 是 `nullOr (enum …)`，同优先级的两个不同值合并时报 conflicting definition values，GNOME 用户不显式设 `type` 就无法求值（这在本改动之前就存在，但 IBus 接入的主要受众正是 GNOME 用户）。降一级后 GNOME 下 `type` 取 `ibus`、包跟着换成 `msime-ibus`；其余桌面仍是 `fcitx5`，用户显式设的值照样优先。nixpkgs 的两个框架模块各自 `mkIf type == …`，只有选中的那个会读自己的列表，所以模块不必自己分支。警告改为 `type` 不在 `fcitx5`、`ibus` 之内时才给。
- 实机（Hyprland 会话）上 `msime-linux-setup --register` 原本注册不了 IBus，有两层原因，都在本改动里修：
  - `running()` 只用 `pgrep -x` 比对进程名，而 Nix 的 `wrapProgram` 启动的 `ibus-daemon` 进程名是 `.ibus-daemon-wr`，脚本于是报「Fcitx5 和 IBus 都没有在运行」。`running()` 再用 `pgrep -f` 比对命令行的第一个词（包装脚本以 `exec -a` 保留了 `…/bin/ibus-daemon`），只出现在参数里的不算。这是 `scripts/msime-linux-setup` 的通用改动，任何经包装脚本启动的守护进程都受益。
  - 认出 IBus 之后，`gsettings` 仍找不到 `org.freedesktop.ibus.general`：IBus 的 schema 只在 `ibus` 自己的包装器和 GNOME 会话的 `XDG_DATA_DIRS` 里。`fcitx5.nix` 给 `msime-linux-setup` 包一层，把 `glib.getSchemaDataDirPath ibus`（`gsettings` 在它下面的 `glib-2.0/schemas` 里找）和 glib 的 `bin` 追加到 `XDG_DATA_DIRS`、`PATH` 后面。
- 其余接线（`environment.systemPackages`、`systemd.packages`、provider 的 `wantedBy`）与框架无关，不变。首次配置的 `msime-linux-setup` 按正在运行的框架注册（`register_ibus`），上一条修好后在 IBus 下也能注册。

## Alternatives considered

- **整个包放进 `ibus.engines`，只补 `meta.isIbusEngine`** — 改动最小，不需要第二个 derivation。但 `ibus-with-plugins` 的构建如上失败；即使 nixpkgs 以后放宽那项检查，整包的 `bin`、`lib`、`share` 也会混进 IBus 的环境（`ibus` 包装器的 `XDG_DATA_DIRS` 等都指向它）。
- **在模块里由 `cfg.package` 现场生成引擎包** — 不碰包定义。但不用模块、经 overlay 自己写配置的用户就得自己复制这段，而 README 已经给这类用户列出了要填的几项；放在 `passthru` 上两种用法共用一份。
- **把引擎包做成 flake 里独立的 `packages` 输出（而不是 `passthru`）** — 发现性更好。但它绑定的是 flake 自己构建的那份宿主，用户 `override` 了 `programs.msime.package` 时引擎仍指向未改过的那份，两边的宿主不一致。
- **只保留一个包，选 IBus 时照样带着 Fcitx5 插件** — 只有一份构建、一套 ctest，不多一个变体。代价在本机上量过：只因 Fcitx5 进闭包的路径约 108 MiB，其中 systemd、cryptsetup 等 NixOS 本来就有，真正多出的是 `fcitx5` 本体约 25 MB 和 `xcb-imdkit` 等几 MB，外加插件的编译时间。省得不多，但经 overlay 的 IBus 用户没有办法不装 Fcitx5，选了 IBus 还要编 Fcitx5 插件也让人困惑。
- **`msime-ibus` 沿用 `pname = "msime-fcitx5"`** — store 路径名不变，少一处条件。但只装 IBus 的系统里出现 `msime-fcitx5-<版本>` 会误导排查（`/proc/<pid>/maps`、`readlink` 的结果）。
- **把整个 `share/ibus/component` 目录链接过来** — 少一行 glob。但 `ibus` 自己也有 `share/ibus/component`，`buildEnv` 合并时要靠它跟随目录链接的行为；逐个链接文件不依赖这一点。

## Consequences

- **收益**：`i18n.inputMethod.type = "ibus"` 加 `programs.msime.enable = true` 即可在 IBus（含 GNOME 输入源）里用水杉，provider 服务、设置窗口与 Fcitx5 时相同，系统里不带 Fcitx5 插件和 `fcitx5`。不用模块的用户取 `pkgs.msime-ibus` 与它的 `ibusEngine` 即可。
- **代价**：`nix flake check` 多一次完整的 CMake 构建（`msime-ibus`）。它不跑 ctest（`doCheck = enableFcitx5`）：关掉插件只少了 `fcitx5/` 子目录，其余测试与 `msime-fcitx5` 相同，再跑一遍只是重复；只构建 `msime-ibus` 的用户因此看不到 ctest 结果，测试结果由 `msime-fcitx5` 和 CI 给出。变体特有的安装布局（`msime-linux-ibus` 的 RUNPATH、组件 `<exec>`）由装后检查核对。
- **约束**：`programs.msime.package` 现在要求包带 `ibusEngine`（只在 `type = "ibus"` 时被求值）；换成不是从 `msime-fcitx5` 派生的包时会在求值时报缺属性。引擎图标仍靠 `environment.systemPackages` 链进 `/run/current-system/sw/share/icons`，没有放进 `ibus-with-plugins` 的环境。
- **已知上限**：正在运行的 `ibus-daemon` 只读它自己那份 `ibus-with-plugins` 的组件，`ibus restart` 按原路径重启，切换配置后要重新登录才换到新构建；这与 Fcitx5 的情况相同，README 已写明。GNOME、KDE 以外的 Wayland 混成器上，nixpkgs 的 ibus 模块以 `--xim` 方式自启动 IBus 并导出 `GTK_IM_MODULE`、`QT_IM_MODULE`，原生 Wayland 程序切不到水杉，GTK、Qt 程序的候选和提示在 Hyprland 上成了抢焦点的窗口。要以 `ibus start --type wayland` 启动并设 `ibus.waylandFrontend = true`。这些是 nixpkgs 决定的启动方式和环境变量，X11 会话和 GNOME 仍需要原样，模块不覆盖它们，做法写在 `platforms/linux/README.md` 的「Nix 与 NixOS」一节。

## Verification

`nix build .#checks.x86_64-linux.nixos-module`：虚拟机测试新增节点 `ibus`（`i18n.inputMethod.type = "ibus"`），以 alice 在 linger 的会话总线上起 PATH 上的 `ibus-daemon`，等 `ibus list-engine` 列出 `msime-linux` 和「水杉输入法」，再核对 PATH 上的命令来自 `msime-ibus`、前缀下没有 `lib/fcitx5`。它同时证明 `ibus-with-plugins` 能构建、引擎类型检查通过。`nix build .#msime-ibus` 跑关掉插件后的装后检查。模块的默认包与断言用 `nixosSystem` 分别对 `ibus`、`fcitx5`、`kime` 和「`fcitx5` 配 `msime-ibus`」求值核对过；另对开着 `services.desktopManager.gnome` 的配置求值：不设 `type` 时得到 `ibus` 与 `msime-ibus`（改用 `mkOverride 1001` 之前此处报冲突），显式设 `fcitx5` 时得到 `fcitx5` 与 `msime-fcitx5`，不开 GNOME 时仍是 `fcitx5`。`tests/core/setup_registration.py` 新增一例：只有命令行是 `…/bin/ibus-daemon` 的进程（进程名不是）时照样认出 IBus 并写入 `preload-engines`，只在参数里出现 `ibus-daemon` 的进程不算；去掉 `running()` 的改动时这一例失败。实机上在 Hyprland 会话里 `ibus engine msime-linux` 拉起了 `msime-ibus` 的宿主。选中引擎后宿主真正打字没有在虚拟机里覆盖（需要词库，虚拟机不联网），要在真机上核对。
