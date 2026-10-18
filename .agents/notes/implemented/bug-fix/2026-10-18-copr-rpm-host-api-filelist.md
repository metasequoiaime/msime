# Agent Note: COPR RPM 包含 Host API 脚本

Status: implemented

## Problem

`platforms/linux/CMakeLists.txt` 会把 `scripts/host_api.py` 安装到 `%{_bindir}`，但 RPM 规格只用 `%{_bindir}/msime_*.py` 声明 Python 脚本。Fedora COPR 的 `rpmbuild` 在 `check-files` 阶段因此报出 `/usr/bin/host_api.py` 是已安装但未打包的文件，x86_64 与 aarch64 构建都失败。

## Decision

RPM `%files` 显式列出 `%{_bindir}/host_api.py`，并在发行版构建契约测试中检查这条声明。带下划线前缀的 `msime_*.py` 通配符继续覆盖现有 MSIME 脚本；独立的 `host_api.py` 需要单独列出，避免扩大通配符范围而掩盖其他未打包文件。

## Alternatives considered

- 把通配符改成 `%{_bindir}/*.py`：能暂时包含该文件，但会把未来意外安装到二进制目录的 Python 文件静默纳入 RPM，削弱 `rpmbuild` 的未打包文件检查，因此不采用。
- 从 CMake 安装列表移除 `host_api.py`：会破坏字典 smoke test 和运行时脚本对 Host API 的调用，不能解决发行版包的实际缺口。

## Consequences

RPM 源码包和 COPR 的 Fedora 构建会把 `host_api.py` 纳入包文件清单，`check-files` 不再因该文件失败。新增的契约断言只检查 RPM 规格的显式声明；如果 CMake 安装路径或脚本命名发生变化，维护者需要同时更新规格和测试。现有 release tarball 与 DEB 文件清单不受影响。
