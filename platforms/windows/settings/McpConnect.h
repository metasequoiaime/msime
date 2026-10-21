#pragma once
#include <array>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

// 「AI」页「连接助手」标签的规则，照共享设置 UI 的 `packages/ui/src/settings/mcp-connect.tsx` 和 `mcp-syntax.tsx`：两个权限开关，Claude Code、Codex 的安装和移除命令，命令行用法，加上权限的 MCP 配置，以及这些文字按种类着色用的记号；写入前的授权说明、写入结果和配置过期的提示。命令里的服务器名取本版本登记的名字（full 是 msime，与共享页相同）。不依赖 Windows 和 WinRT 头文件，方便单独测试。
namespace msime::settings::mcp {

// 一段显示出来的命令或配置的种类，决定它的颜色（mcp-syntax.tsx 的 SyntaxKind）。
enum class Syntax { plain, program, flag, string, placeholder, key, punctuation };

struct Token {
  std::wstring text;
  Syntax kind;
};

// 记号拼起来的文字，也就是复制按钮放进剪贴板的内容。
inline std::wstring tokens_text(std::vector<Token> const &tokens) {
  std::wstring text;
  for (auto const &token : tokens)
    text += token.text;
  return text;
}

// 放宽助手权限的两个开关，按写进 `args` 的固定顺序（permissionFlags）。
struct Permission {
  std::wstring_view flag;
  std::wstring_view title;
  std::wstring_view detail;
};

inline constexpr std::array<Permission, 2> permissions{{
    {L"--allow-write", L"允许修改设置",
     L"修改快捷短语和设置、制作、切换和删除候选窗口皮肤，用设置里登录的账号把皮肤发布到社区"},
    {L"--allow-dictionary-read", L"允许读取词库",
     L"读取用户词库、查看编码的候选；与上一项同时开启才能增删、调整和导入词"},
}};

using Flags = std::vector<std::wstring>;

inline bool has_flag(Flags const &flags, std::wstring_view flag) {
  for (auto const &entry : flags)
    if (entry == flag)
      return true;
  return false;
}

// 只留认识的权限，去重并按 `permissions` 的顺序排好。
inline Flags canonical(Flags const &flags) {
  Flags result;
  for (auto const &permission : permissions)
    if (has_flag(flags, permission.flag))
      result.emplace_back(permission.flag);
  return result;
}

// 打开或关掉一项，其余照旧（setFlag 的 toggled）。
inline Flags toggled(Flags const &current, std::wstring_view flag, bool on) {
  Flags result;
  for (auto const &permission : permissions)
    if (permission.flag == flag ? on : has_flag(current, permission.flag))
      result.emplace_back(permission.flag);
  return result;
}

inline bool same_flags(Flags const &a, Flags const &b) { return canonical(a) == canonical(b); }

// 存在本机的选择：每项一位，按 `permissions` 的顺序。没存过时两项都开（savedFlags 的缺省）。
inline constexpr std::uint32_t all_flag_bits = (1u << permissions.size()) - 1;

inline std::uint32_t flag_bits(Flags const &flags) {
  std::uint32_t bits = 0;
  for (std::size_t i = 0; i < permissions.size(); ++i)
    if (has_flag(flags, permissions[i].flag))
      bits |= 1u << i;
  return bits;
}

inline Flags flags_from_bits(std::uint32_t bits) {
  Flags flags;
  for (std::size_t i = 0; i < permissions.size(); ++i)
    if (bits & (1u << i))
      flags.emplace_back(permissions[i].flag);
  return flags;
}

// 写入带权限的条目前确认框的标题和说明（grantMessage）：用大白话说清楚助手将能做什么。
inline std::wstring grant_title(std::wstring_view name) {
  return L"允许 " + std::wstring(name) + L" 中的助手使用这些权限？";
}

inline std::wstring grant_message(std::wstring_view name, Flags const &flags) {
  const bool write = has_flag(flags, L"--allow-write");
  const bool read = has_flag(flags, L"--allow-dictionary-read");
  std::vector<std::wstring> abilities;
  if (write)
    abilities.emplace_back(
        L"修改设置和快捷短语、制作、切换和删除候选窗口皮肤，用你在设置里登录的账号把皮肤发布到社区");
  if (read)
    abilities.emplace_back(L"读取你的用户词库、查看编码的候选");
  if (write && read)
    abilities.emplace_back(L"增删、调整和导入词");
  std::wstring joined;
  for (std::size_t i = 0; i < abilities.size(); ++i)
    joined += (i == 0 ? L"" : L"；") + abilities[i];
  return L"写入后，" + std::wstring(name) + L" 里的 AI 助手可以" + joined +
         L"。只在你信任它时允许；之后关掉开关再更新即可收回。";
}

// 写入成功后的一句（write 的 setResult）。`outcome` 是 msime_client_mcp_install 的回答。
inline std::wstring install_result(std::wstring_view outcome, std::wstring_view name) {
  const std::wstring client(name);
  if (outcome == L"unchanged")
    return client + L" 已经连接，无需改动。";
  if (outcome == L"updated")
    return L"已更新 " + client + L" 的配置。重新启动 " + client + L" 后生效。";
  return L"已写入 " + client + L" 的配置。重新启动 " + client + L" 后生效。";
}

// 已连接的助手和页面上的权限对不上，或指向 Nix store 里以前那一版时的提示（SettingsNotice）。
inline std::wstring outdated_notice(std::wstring_view name, bool stale, std::wstring_view command) {
  const std::wstring client(name);
  std::wstring text =
      stale ? client + L" 现在的配置指向 Nix store 里某一版的服务器程序，系统升级或垃圾回收后会失效。"
            : L"下面的权限和 " + client + L" 现在的配置不同。";
  text += L"点「更新 " + client + L"」";
  if (stale)
    text += L"改成 " + std::wstring(command) + L"，按下面的权限";
  return text + L"写入。";
}

// `C:\` 这样开头的路径（isWindowsPath）。
inline bool is_windows_path(std::wstring_view path) {
  return path.size() >= 3 &&
         ((path[0] >= L'A' && path[0] <= L'Z') || (path[0] >= L'a' && path[0] <= L'z')) &&
         path[1] == L':' && path[2] == L'\\';
}

// 按粘贴到的 shell 给一个参数加引号（shellQuote）：只含字母、数字和 `_@%+=:,./-` 的不加；Windows 路径用双引号，里面的双引号前加反斜杠；POSIX shell 用单引号。
inline std::wstring shell_quote(std::wstring_view value, bool windows) {
  auto safe = [](wchar_t c) {
    return (c >= L'A' && c <= L'Z') || (c >= L'a' && c <= L'z') || (c >= L'0' && c <= L'9') ||
           std::wstring_view(L"_@%+=:,./-").find(c) != std::wstring_view::npos;
  };
  bool bare = !value.empty();
  for (wchar_t c : value)
    bare = bare && safe(c);
  if (bare)
    return std::wstring(value);
  std::wstring quoted;
  if (windows) {
    quoted = L"\"";
    for (wchar_t c : value) {
      if (c == L'"')
        quoted += L'\\';
      quoted += c;
    }
    return quoted + L"\"";
  }
  quoted = L"'";
  for (wchar_t c : value) {
    if (c == L'\'')
      quoted += L"'\\''";
    else
      quoted += c;
  }
  return quoted + L"'";
}

inline Token plain(std::wstring text) { return {std::move(text), Syntax::plain}; }

// 服务器程序连同运行时选项和打开的权限，按 shell 加好引号（serverProgram）。`options` 是 msime_client_mcp_status 的 `options`。
inline std::vector<Token> server_program(std::wstring_view command,
                                         std::optional<std::wstring> const &options,
                                         Flags const &flags) {
  const bool windows = is_windows_path(command);
  std::vector<Token> tokens{{shell_quote(command, windows), Syntax::program}};
  if (options) {
    tokens.push_back(plain(L" "));
    tokens.push_back({L"--options", Syntax::flag});
    tokens.push_back(plain(L" "));
    tokens.push_back({shell_quote(*options, windows), Syntax::string});
  }
  for (auto const &flag : flags) {
    tokens.push_back(plain(L" "));
    tokens.push_back({flag, Syntax::flag});
  }
  return tokens;
}

// 两个在终端里注册服务器的助手。
enum class Assistant { claude_code, codex };

// 安装命令（installCommand）。`server` 是登记用的服务器名。
inline std::vector<Token> install_command(Assistant assistant, std::wstring_view server,
                                          std::wstring_view command,
                                          std::optional<std::wstring> const &options,
                                          Flags const &flags) {
  std::vector<Token> tokens;
  if (assistant == Assistant::claude_code) {
    tokens = {{L"claude", Syntax::program},
              plain(L" mcp add "),
              {L"--scope", Syntax::flag},
              plain(L" user " + std::wstring(server) + L" ")};
  } else {
    tokens = {{L"codex", Syntax::program}, plain(L" mcp add " + std::wstring(server) + L" ")};
  }
  tokens.push_back({L"--", Syntax::flag});
  tokens.push_back(plain(L" "));
  for (auto &token : server_program(command, options, flags))
    tokens.push_back(std::move(token));
  return tokens;
}

// 移除以前登记的服务器（removeCommand）：两个助手都不肯添加已有的名字，改权限要先移除。
inline std::vector<Token> remove_command(Assistant assistant, std::wstring_view server) {
  if (assistant == Assistant::claude_code)
    return {{L"claude", Syntax::program},
            plain(L" mcp remove "),
            {L"--scope", Syntax::flag},
            plain(L" user " + std::wstring(server))};
  return {{L"codex", Syntax::program}, plain(L" mcp remove " + std::wstring(server))};
}

// 告诉能在终端里运行命令的助手怎么直接调用工具（terminalInstructions）。cmd 和 Windows PowerShell 都传不完整带引号的 JSON 参数，所以 Windows 上参数经文件传。
inline std::vector<Token> terminal_instructions(std::wstring_view command,
                                                std::optional<std::wstring> const &options,
                                                Flags const &flags) {
  const auto msime = server_program(command, options, flags);
  std::vector<Token> tokens;
  auto append = [&tokens](std::vector<Token> const &more) {
    tokens.insert(tokens.end(), more.begin(), more.end());
  };
  tokens.push_back(plain(L"水杉输入法（MSIME）可以在终端里直接管理：\n- 查看可用的工具和参数："));
  append(msime);
  tokens.push_back(plain(L" tools\n- 调用一个工具，参数是 JSON 对象，输出 JSON："));
  if (is_windows_path(command)) {
    tokens.push_back(plain(L"把 JSON 参数以 UTF-8 写进一个文件，再运行 "));
    append(msime);
    tokens.push_back(plain(L" call "));
    tokens.push_back({L"<工具名>", Syntax::placeholder});
    tokens.push_back(plain(L" "));
    tokens.push_back({L"@<文件路径>", Syntax::placeholder});
  } else {
    append(msime);
    tokens.push_back(plain(L" call "));
    tokens.push_back({L"<工具名>", Syntax::placeholder});
    tokens.push_back(plain(L" "));
    tokens.push_back({L"'<JSON 参数>'", Syntax::string});
    tokens.push_back(plain(L"，参数中有单引号时改为写进 UTF-8 文件并传 "));
    tokens.push_back({L"@<文件路径>", Syntax::placeholder});
  }
  tokens.push_back(plain(L"\n- 排查输入法问题（卡顿、候选窗口不见了）的步骤："));
  append(msime);
  tokens.push_back(plain(L" prompt diagnose"));
  return tokens;
}

// 宿主给的 MCP 配置（`{"mcpServers": {"<服务器名>": {...}}}`，serde_json 的缩进格式）按 JSON 的词法着色，并把打开的权限接在服务器条目 `args` 的末尾（configWithFlags）。逐个词法单元处理，不重新排版：没有权限时原文照旧，有权限时照数组已有的换行和缩进接上，数组写在一行时接成 `, "--allow-write"`。读不懂（字符串没有结束、找不到 `mcpServers` 下条目的 `args` 数组）时原文不着色，也不接权限，与共享页一样。
inline std::vector<Token> config_tokens(std::wstring_view config, Flags const &flags) {
  const std::vector<Token> fallback{plain(std::wstring(config))};
  std::vector<Token> tokens;
  for (std::size_t i = 0; i < config.size();) {
    const wchar_t c = config[i];
    if (c == L' ' || c == L'\n' || c == L'\r' || c == L'\t') {
      std::size_t end = i;
      while (end < config.size() &&
             (config[end] == L' ' || config[end] == L'\n' || config[end] == L'\r' ||
              config[end] == L'\t'))
        ++end;
      tokens.push_back(plain(std::wstring(config.substr(i, end - i))));
      i = end;
    } else if (c == L'"') {
      std::size_t end = i + 1;
      while (end < config.size() && config[end] != L'"')
        end += config[end] == L'\\' ? 2 : 1;
      if (end >= config.size())
        return fallback;
      tokens.push_back({std::wstring(config.substr(i, end + 1 - i)), Syntax::string});
      i = end + 1;
    } else if (std::wstring_view(L"{}[],:").find(c) != std::wstring_view::npos) {
      tokens.push_back({std::wstring(1, c), Syntax::punctuation});
      ++i;
    } else {
      std::size_t end = i;
      while (end < config.size() &&
             std::wstring_view(L"{}[],:\" \n\r\t").find(config[end]) == std::wstring_view::npos)
        ++end;
      tokens.push_back(plain(std::wstring(config.substr(i, end - i))));
      i = end;
    }
  }
  // 后面紧跟冒号的字符串是键。
  auto next_solid = [&tokens](std::size_t from) {
    while (from < tokens.size() && tokens[from].kind == Syntax::plain &&
           tokens[from].text.find_first_not_of(L" \n\r\t") == std::wstring::npos)
      ++from;
    return from;
  };
  for (std::size_t i = 0; i < tokens.size(); ++i)
    if (tokens[i].kind == Syntax::string) {
      const auto next = next_solid(i + 1);
      if (next < tokens.size() && tokens[next].text == L":")
        tokens[i].kind = Syntax::key;
    }
  if (flags.empty())
    return tokens;
  // 找 `mcpServers` 里第一个条目的 `args` 数组：根对象（第 1 层）→ mcpServers 对象（第 2 层）→ 条目对象（第 3 层）里的键 "args"。
  std::vector<std::wstring> keys;
  std::wstring pending;
  std::optional<std::size_t> open;
  for (std::size_t i = 0; i < tokens.size() && !open; ++i) {
    auto const &token = tokens[i];
    if (token.kind == Syntax::key) {
      pending = token.text;
    } else if (token.text == L"{" || token.text == L"[") {
      if (token.text == L"[" && keys.size() == 3 && keys[1] == L"\"mcpServers\"" &&
          pending == L"\"args\"")
        open = i;
      keys.push_back(pending);
      pending.clear();
    } else if (token.text == L"}" || token.text == L"]") {
      if (keys.empty())
        return fallback;
      keys.pop_back();
    }
  }
  if (!open)
    return fallback;
  std::size_t depth = 0;
  std::optional<std::size_t> close;
  for (std::size_t i = *open; i < tokens.size() && !close; ++i) {
    if (tokens[i].text == L"{" || tokens[i].text == L"[")
      ++depth;
    else if ((tokens[i].text == L"}" || tokens[i].text == L"]") && --depth == 0)
      close = i;
  }
  if (!close)
    return fallback;
  // 数组已有的元素前面是什么空白：换行加缩进，或写在一行时的一个空格。
  const bool empty = next_solid(*open + 1) == *close;
  const auto first = *open + 1;
  std::wstring separator = L" ";
  if (first < *close && tokens[first].kind == Syntax::plain &&
      tokens[first].text.find(L'\n') != std::wstring::npos)
    separator = tokens[first].text;
  // 接在最后一个元素后面，也就是收尾的 `]` 和它前面的空白之前。
  auto insert_at = *close;
  if (insert_at > first && tokens[insert_at - 1].kind == Syntax::plain &&
      tokens[insert_at - 1].text.find_first_not_of(L" \n\r\t") == std::wstring::npos)
    --insert_at;
  std::vector<Token> added;
  for (auto const &flag : flags) {
    if (!(empty && added.empty()))
      added.push_back({L",", Syntax::punctuation});
    if (!(empty && added.empty() && separator == L" "))
      added.push_back(plain(separator));
    added.push_back({L"\"" + flag + L"\"", Syntax::string});
  }
  tokens.insert(tokens.begin() + static_cast<std::ptrdiff_t>(insert_at), added.begin(), added.end());
  return tokens;
}

} // namespace msime::settings::mcp
