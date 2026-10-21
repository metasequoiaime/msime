# Agent Note: macOS 迁移对照的覆盖数字改成可重跑，偏好门禁递归到嵌套字段

Status: implemented

## Problem

`docs/macos-parity.md` 与 `docs/macos-feature-inventory.md` 用「五条轴各自可重跑、明确缺口一档为空」支撑「macOS 迁移已完成」。2026-10-21 的对抗性核对（`/tmp/msime-audit/parity-falsify.md`）证明支撑这句话的覆盖证据不成立：

- **断言轴只覆盖约一成。** 文档记「来源 93 条 `require(...)` 断言逐条核对」，来源实有 958 条。旧命令 `grep -rhoE 'require\([^,]*,\s*"[^"]+"' *.mm *.cpp` 有四处缺陷：只认小写 `require(`（大写 `Require(` 416 条）、`[^,]*` 遇首参逗号失配、不跨行、glob 不含 `.swift`。93 不是抽样，是正则缺陷的产物。
- **偏好门禁有确定漏检。** `traditional_chinese_variant`（`crates/client-core/src/preferences.rs:1049`）在 macOS 宿主没有任何消费方：macOS 的简繁转换固定走 OpenCC `s2t`（`platforms/macos/src/core/ChineseTextConversion.mm` 调 `msime_client_simplified_to_traditional`），带变体的 `msime_client_convert_to_traditional` 只有 Linux 的 IBus / Fcitx5 调用（`crates/client-core/src/host_surface.rs` 的 `traditional_chinese_variants: matches!(platform, HostPlatform::Linux)`）。但门禁把整个文件当纯文本，「字段名出现在任何地方」即算消费，而它在扫描集里唯一一次出现是 `crates/host-api/src/ffi/host.rs:74` 的**文档注释**。于是门禁判它已消费，它既没进 `NOT_APPLICABLE`，本机跑脚本也照样 exit 0。从 Linux 或云同步带过来的 `s2tw` / `s2twp` 在 macOS 上被静默按 s2t 处理——正是这个门禁开头描述的「开关保存了、页面说保存了、实际什么也没发生」。
- **偏好门禁只看顶层字段。** 旧脚本 `re.findall(r"\n    pub (\w+):", body)` 只抓顶层 `pub` 字段，`Preferences` 的 207 个嵌套子字段一个都不检查；只要顶层结构体名在宿主代码里出现一次，整个子结构就被当作已覆盖。
- **文档数字不可重跑。** 偏好写 82/71/11（实际 114/92/22）、文件清单写 42（实际 45）、符号写 518（实际 517）、文案同一文档一处 623 一处 643（自相矛盾）。

## Decision

**断言轴。** 新增 `platforms/macos/scripts/extract_source_assertions.py`，按括号配对切分 `require(` / `Require(` 调用，覆盖 `.mm` / `.cpp` / `.swift`，计数分「总数 / 带字符串消息 / 无消息」三档。固定基线 `663d7db` 上得 **958 / 847 / 111**，去重消息 845。`docs/macos-assertion-audit.md` 记录这条命令与旧命令的分解（旧命令命中 93，同文件被正则漏 443，glob 与宏名都到不了的 422），并把「逐条核对」的范围老实写成那 93 条，其余 865 条标注「尚未逐条核对」。

**不把消息文本当跨仓核对口径。** 845 条去重消息里只有 85 条能在本仓库逐字命中，其余是描述来源内部夹具的英文句子；断言文本本来就不是目标仓的可检索字符串。所以核对靠按语义读测试、在目标找对应实现或测试，抽取脚本只负责把待核对集合数准。111 条无字符串消息（单参 `Require(bool)`、`require(_ value: Bool)`、以 `error.localizedDescription.UTF8String` 作消息）没有可比对文本，只能读代码，脚本不把它们算进「已核对」。

**偏好门禁。** `platforms/macos/tests/settings/preference_coverage.py` 改为：

1. 用大括号配对解析每个 `pub struct`，从 `Preferences` 按字段路径递归下降，解开 `Option<T>`；同类型结构在不同父路径各自展开，只有当前祖先链遇到环才停止，不展开列表等集合容器。字段按点分路径（`keybindings.switch_language_ctrl_space`）出现在报告里。已豁免的父结构不再下降，理由已覆盖整块。
2. 可达性仍按**裸字段名**判定。宿主读嵌套项时用的是 Objective-C 访问器或共享文档的 JSON 键（`AppearancePreferences.mm` 的 `merged[@"voice_input"]`），不拼 Rust 的 `struct.field` 路径；改用限定名会误报 100 多个字段（试过，`voice_input.enabled` 之类全红）。
3. 匹配前剥掉 `///` / `//!` 文档注释行：散文里点名一个偏好不等于有人读它。
4. `NOT_APPLICABLE` 增补 `traditional_chinese_variant`、`keybindings.switch_language_ctrl_space`、`keybindings.toggle_japanese_ctrl_shift_j`、`wubi_candidate_selection.second_key/third_key`、`fuzzy_pinyin.seeded`，并记录 `custom_theme.keyboard` 中 macOS 不读取的触屏设计字段，每条写明理由。

结果：324 个字段（117 顶层 + 207 嵌套）中 283 个有消费，41 个豁免（23 顶层 + 18 嵌套），脚本 exit 0。

**文案轴。** 新增 `platforms/macos/scripts/extract_source_literals.py`，口径固定为「来源 `platforms/macos/src` 单行含汉字字符串字面量、按内容去重」= **694**。旧文档的 623 / 643 来自一次没有记录、无法复跑的过滤，两个数互相矛盾，一并改成脚本口径。

**文件与符号轴。** 文件清单改 45（`comm -23`，固定基线 110 个来源文件、65 个同名），补上此前漏列的 `BackendLocalSnapshot.swift`、`BackendSkinArtworkClient.swift`、`IOSPreferencePlan.swift` 与单独删除的 `WubiCommitPolicy.h`；符号改 517（`macos-feature-inventory.md` 第六节命令）。

## Alternatives considered

- **偏好门禁只把「不递归」写成已知边界，不改递归。** 改动最小，也满足「二选一、别含糊」。否掉：审计反例 3 指出的正是这个洞——只要顶层结构体名出现一次，整块子结构就绕过门禁。实测注入一个没人消费的嵌套字段（`navigation.zzzprobe`），旧脚本 exit 0、新脚本 exit 1，说明递归确实补上了真实漏检。
- **嵌套字段按限定名（`struct.field`）核对。** 语义更准，能避开同名裸字段的假阳性。否掉：宿主读嵌套项走 ObjC 访问器与 JSON 键，不出现限定名，实测 100 多个字段被误报为不可达；要修就得为每个平台写专门的消费检测，超出这道门禁的范围。
- **断言轴继续用 grep，只把正则改成 `\b[Rr]equire\(`。** 计数会接近，但首参里的逗号、跨行消息和 `.swift` 仍会错，而且拿不到「带消息 / 无消息」的分档；无消息那 111 条正是不能自动核对的部分，必须单独报出来。
- **用消息文本逐条在目标仓检索当作核对。** 实测 845 条只有 85 条逐字命中，会把「措辞不同但行为已实现」误判成缺口，也会把「文本恰好出现」误判成已核对。否掉。
- **文案轴保留旧数字、只加一句「方法未记录」。** 否掉：三份文档的数字要互相一致、与重跑结果一致，留一个不可复跑的数字就还会被下一次审计抓到。

## Consequences

- 覆盖数字从「文档说了算」变成「脚本说了算」：断言 958、文案 694、符号 517、文件 45、偏好 324 字段中 283 有消费 / 41 豁免，每条都附了可重跑命令。
- 结论的措辞跟着收紧：`macos-parity.md` 从「明确缺口一档现在是空的」改成「相对固定基线 `663d7db`，在本次抽到的维度上没有发现新缺口」，并单列「哪些仍不覆盖」。
- 偏好门禁的已知边界写进了脚本 docstring：裸字段名判定消费，看不见「名字出现过、但行为没接」；不覆盖 `.py` 与测试文件里的引用；经辅助方法读字段的（`WubiCandidateSelectionPreferences::slot`）需要人工豁免并写明消费点。
- 断言轴 865 条未逐条核对是真实的欠账，不是可以靠改正则消掉的；要清掉它得按测试文件逐个读来源测试、在目标找对应实现，工作量按文件数算（22 个文件），不该再包装成「已核对」。

## Verification

- `python3 platforms/macos/tests/settings/preference_coverage.py .` → `283 of 324 shared preferences reach the macOS host; the other 41 are accounted for (207 nested fields checked).`，exit 0；改前同一条命令是 `92 of 114 ... the other 22 are accounted for.`，exit 0（漏检 `traditional_chinese_variant`）。
- 注入探针：在 `NavigationPreferences` 加 `zzzprobe` 后新脚本报 `navigation.zzzprobe is not read on macOS and has no entry here` 并 exit 1，旧脚本 exit 0；还原后 exit 0。
- `python3 platforms/macos/scripts/extract_source_assertions.py /tmp/msime-audit/MSIME-Apple` → 958 / 847 / 111 / 845；旧命令 `grep -hoE 'require\([^,]*,\s*"[^"]+"' *.mm *.cpp | wc -l` → 93。
- `python3 platforms/macos/scripts/extract_source_literals.py /tmp/msime-audit/MSIME-Apple` → 694 去重 / 852 出现。
- 文件 `comm -23` → 45；符号命令 → 517。
- `python3 platforms/macos/tests/settings/settings_route_coverage.py .` → exit 0（相邻门禁未受影响）；两个新脚本 `python3 -m py_compile` 通过。
- 未跑：macOS 的 `ctest -R preference-coverage` 需要完整 CMake 构建树（本 worktree 未构建，且按仓库约定不新起构建目录），只按 CMakeLists 里注册的同一条命令直接跑脚本。

审查回归：`scripts/test-macos-preference-coverage.py` 的四项合成用例验证同类型兄弟路径、可选结构、递归环与父路径豁免。新增门禁由 `run-checks.sh` 自动发现；真实偏好覆盖脚本与相邻设置路由门禁通过。裸字段名仍可能被其他结构中的同名字段命中，这属于本门禁明确保留的语义边界。
