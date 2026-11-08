# Agent Note: Linux 在线 provider 的多架构 host 库回退

Status: implemented

## Problem

`msime-linux-online-provider` 在 `/usr/bin` 下运行时，旧版 provider 没有读取 CMake 生成的 `msime_voice_paths.py`，只能猜 `/usr/lib/msime-client`，找不到 Debian 把 `libmsime_host_api.so` 放在的 `/usr/lib/<multiarch>/msime-client`。报告者执行 `--ensure-anonymous-account` 时因此无法加载 host 库；旧代码还会在构造开发树候选路径时访问不存在的 `parents[3]`，直接抛 `IndexError`。兼容不完整安装时也可能缺少这个路径模块。

## Decision

保留安装时由 CMake 生成的 `msime_voice_paths.py` 路径作为首选，并在旧版 provider 或不完整安装缺少该模块时，用 Python `sysconfig.get_config_var("MULTIARCH")` 补试 `<prefix>/lib/<multiarch>/msime-client` 下的两个标准库名。开发树候选路径继续只在父目录足够深时构造，安装路径查找失败只返回不可用，不让 provider 启动崩溃。

## Alternatives considered

- **只依赖 `msime_voice_paths.py`** — 新版 CMake 安装包的路径最准确，但无法修复已经发布的旧包或被发行版拆分掉该模块的安装，报告中的 provider 仍会失败。
- **按发行版硬编码更多库目录** — 可以覆盖少数已知布局，但会继续遗漏其他多架构命名；`MULTIARCH` 是 Debian/Python 约定的标准来源，且不替代 CMake 写入的精确路径。

## Consequences

标准 Debian 多架构布局和旧版不完整安装可以加载 host 库，匿名账号注册不再因库查找崩溃；新版安装仍优先使用 CMake 记录的真实路径。增加了一次本地配置查询和两个候选路径，找不到库时仍会按原流程返回不可用。

## Verification

`platforms/linux/tests/provider/installed_layout_library_lookup.py` 覆盖 `/usr/bin` 下没有源码树时不访问 `parents[3]`，并覆盖 `MULTIARCH=x86_64-linux-gnu` 时尝试 `/usr/lib/x86_64-linux-gnu/msime-client`。另运行 Linux Python 合约测试清单和笔记校验。
