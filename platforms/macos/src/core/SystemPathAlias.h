#pragma once

#include <cerrno>
#include <filesystem>
#include <sys/stat.h>
#include <system_error>
#include <vector>

// 存储路径的符号链接检查。规则与 `crates/path-trust/src/lib.rs` 完全一致，宿主各处（诊断日志、资源包、语音静音日志、词典安装）都只问这里，不要再各写一份按名字放行 `/tmp`、`/var` 的逐层检查。
namespace msime::mac
{
// macOS 自带、存储路径可以经过的系统符号链接，以及每条链接唯一受信任的目标。对应 `crates/path-trust/src/lib.rs` 里 macOS 的 `SYSTEM_ALIASES`，两边必须保持相同。
inline bool IsTrustedSystemAliasTarget(const std::filesystem::path &path, const std::filesystem::path &target)
{
    const char *expected = path == "/tmp" ? "/private/tmp" : (path == "/var" ? "/private/var" : nullptr);
    if (!expected) return false;
    // 与 Rust 的 normalize_lexical 一样按字面相对链接所在目录解析目标，不跟随目标里的其它链接；末尾的 `/` 不算一层。
    std::filesystem::path resolved = (path.parent_path() / target).lexically_normal();
    if (!resolved.has_filename() && resolved != resolved.root_path()) resolved = resolved.parent_path();
    return resolved == expected;
}

// `path` 是否是 macOS 的系统别名链接：名字对得上还不够，读出来的目标也必须是唯一受信任的那个。
inline bool IsTrustedSystemAlias(const std::filesystem::path &path)
{
    std::error_code error;
    const std::filesystem::path target = std::filesystem::read_symlink(path, error);
    return !error && IsTrustedSystemAliasTarget(path, target);
}

// 从 `/` 逐层检查绝对路径 `path`：任何一层是符号链接都拒绝，唯一的例外是最后一层之上至多一个受信任的系统别名（见 IsTrustedSystemAlias）。不存在的层可以接受，调用方正要创建它们；其它 lstat 错误、相对路径和含 `..` 的路径一律拒绝。`existingComponentsMustBeDirectories` 为真时，已存在的每一层（别名除外）还必须是真实目录。
inline bool StoragePathIsSafe(const std::filesystem::path &path, bool existingComponentsMustBeDirectories)
{
    if (!path.is_absolute()) return false;
    std::vector<std::filesystem::path> components;
    for (const auto &component : path.relative_path()) {
        if (component.empty() || component == ".") continue;
        if (component == "..") return false;
        components.push_back(component);
    }
    std::filesystem::path current = path.root_path();
    bool sawSystemAlias = false;
    for (std::size_t index = 0; index < components.size(); ++index) {
        current /= components[index];
        struct stat info = {};
        if (lstat(current.c_str(), &info) != 0) {
            if (errno == ENOENT) continue;
            return false;
        }
        if (S_ISLNK(info.st_mode)) {
            const bool last = index + 1 == components.size();
            if (last || sawSystemAlias || !IsTrustedSystemAlias(current)) return false;
            sawSystemAlias = true;
            continue;
        }
        if (existingComponentsMustBeDirectories && !S_ISDIR(info.st_mode)) return false;
    }
    return true;
}
} // namespace msime::mac
