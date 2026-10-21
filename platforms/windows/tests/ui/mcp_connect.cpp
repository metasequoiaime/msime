#include "../../settings/McpConnect.h"
#include <cstdio>
#include <string>

using namespace msime::settings::mcp;
namespace {
int failures = 0;
void check(bool condition, const char *what) {
  if (!condition) {
    std::fprintf(stderr, "FAIL: %s\n", what);
    ++failures;
  }
}

std::wstring kinds(std::vector<Token> const &tokens, Syntax kind) {
  std::wstring joined;
  for (auto const &token : tokens)
    if (token.kind == kind)
      joined += token.text + L"|";
  return joined;
}
} // namespace

int main() {
  const Flags both{L"--allow-write", L"--allow-dictionary-read"};
  const Flags write{L"--allow-write"};
  const Flags read{L"--allow-dictionary-read"};

  // 两个开关的顺序就是写进 args 的顺序；不认识的权限丢掉，重复的只留一个。
  check(permissions[0].flag == L"--allow-write" && permissions[1].flag == L"--allow-dictionary-read" &&
            permissions[0].title == L"允许修改设置" && permissions[1].title == L"允许读取词库",
        "the two permissions");
  check(canonical({L"--allow-dictionary-read", L"--x", L"--allow-write", L"--allow-write"}) == both,
        "canonical order");
  check(toggled(both, L"--allow-write", false) == read && toggled(read, L"--allow-write", true) == both &&
            toggled({}, L"--allow-dictionary-read", true) == read,
        "toggling one keeps the other");
  check(same_flags({L"--allow-dictionary-read", L"--allow-write"}, both) && !same_flags(write, both) &&
            same_flags({}, {}),
        "same flags ignore order");
  // 本机存的选择：每项一位，没存过时两项都开。
  check(flag_bits(both) == all_flag_bits && flag_bits(read) == 2u && flag_bits({}) == 0u, "flag bits");
  check(flags_from_bits(all_flag_bits) == both && flags_from_bits(1u) == write &&
            flags_from_bits(0u).empty() && flags_from_bits(0xFFu) == both,
        "flags from bits");

  // 授权确认框和写入结果，与共享页逐字相同。
  check(grant_title(L"Cursor") == L"允许 Cursor 中的助手使用这些权限？", "grant title");
  check(grant_message(L"Cursor", both) ==
            L"写入后，Cursor 里的 AI 助手可以修改设置和快捷短语、制作、切换和删除候选窗口皮肤，用你在设置里登录的账号把皮肤发布到社区；读取你的用户词库、查看编码的候选；增删、调整和导入词。只在你信任它时允许；之后关掉开关再更新即可收回。",
        "grant message with both");
  check(grant_message(L"Claude Desktop", read) ==
            L"写入后，Claude Desktop 里的 AI 助手可以读取你的用户词库、查看编码的候选。只在你信任它时允许；之后关掉开关再更新即可收回。",
        "grant message with one");
  check(install_result(L"unchanged", L"Cursor") == L"Cursor 已经连接，无需改动。" &&
            install_result(L"updated", L"Cursor") == L"已更新 Cursor 的配置。重新启动 Cursor 后生效。" &&
            install_result(L"added", L"Cursor") == L"已写入 Cursor 的配置。重新启动 Cursor 后生效。" &&
            install_result(L"replaced", L"Cursor") == L"已写入 Cursor 的配置。重新启动 Cursor 后生效。",
        "install results");
  check(outdated_notice(L"Cursor", false, L"C:\\m.exe") ==
            L"下面的权限和 Cursor 现在的配置不同。点「更新 Cursor」写入。",
        "outdated notice");
  check(outdated_notice(L"Cursor", true, L"/run/current-system/sw/bin/msime-mcp") ==
            L"Cursor 现在的配置指向 Nix store 里某一版的服务器程序，系统升级或垃圾回收后会失效。点「更新 Cursor」改成 /run/current-system/sw/bin/msime-mcp，按下面的权限写入。",
        "stale notice");

  // shellQuote：安全字符不加引号，Windows 路径用双引号，POSIX 用单引号。
  check(shell_quote(L"/usr/bin/msime-mcp", false) == L"/usr/bin/msime-mcp", "safe argument");
  check(shell_quote(L"C:\\Program Files\\MSIME\\msime-mcp.exe", true) ==
            L"\"C:\\Program Files\\MSIME\\msime-mcp.exe\"",
        "windows path");
  check(shell_quote(L"a\"b", true) == L"\"a\\\"b\"", "windows double quote");
  check(shell_quote(L"/home/o'neil/x y", false) == L"'/home/o'\\''neil/x y'", "posix single quote");
  check(shell_quote(L"", false) == L"''", "empty argument");
  check(is_windows_path(L"C:\\x") && is_windows_path(L"d:\\") && !is_windows_path(L"/usr") &&
            !is_windows_path(L"\\\\server\\share") && !is_windows_path(L"C:"),
        "windows paths");

  const std::wstring command = L"C:\\Program Files\\MSIME\\server\\msime-mcp.exe";
  const std::optional<std::wstring> options = L"C:\\Users\\me\\AppData\\Local\\MSIME-Client\\runtime-options.json";
  const std::wstring quoted_command = L"\"C:\\Program Files\\MSIME\\server\\msime-mcp.exe\"";
  const std::wstring quoted_options = L"\"C:\\Users\\me\\AppData\\Local\\MSIME-Client\\runtime-options.json\"";
  check(tokens_text(install_command(Assistant::claude_code, L"msime", command, options, both)) ==
            L"claude mcp add --scope user msime -- " + quoted_command + L" --options " +
                quoted_options + L" --allow-write --allow-dictionary-read",
        "claude code install");
  check(tokens_text(install_command(Assistant::codex, L"msime-wubi", command, options, {})) ==
            L"codex mcp add msime-wubi -- " + quoted_command + L" --options " + quoted_options,
        "codex install with the edition's server name and no flags");
  check(tokens_text(install_command(Assistant::codex, L"msime", L"/usr/bin/msime-mcp", std::nullopt, write)) ==
            L"codex mcp add msime -- /usr/bin/msime-mcp --allow-write",
        "no options");
  const auto claude = install_command(Assistant::claude_code, L"msime", command, options, write);
  check(kinds(claude, Syntax::program) == L"claude|" + quoted_command + L"|" &&
            kinds(claude, Syntax::flag) == L"--scope|--|--options|--allow-write|" &&
            kinds(claude, Syntax::string) == quoted_options + L"|",
        "install command colours");
  check(tokens_text(remove_command(Assistant::claude_code, L"msime")) ==
            L"claude mcp remove --scope user msime" &&
            tokens_text(remove_command(Assistant::codex, L"msime")) == L"codex mcp remove msime",
        "remove commands");

  const auto program = quoted_command + L" --options " + quoted_options + L" --allow-write";
  check(tokens_text(terminal_instructions(command, options, write)) ==
            L"水杉输入法（MSIME）可以在终端里直接管理：\n- 查看可用的工具和参数：" + program +
                L" tools\n- 调用一个工具，参数是 JSON 对象，输出 JSON：把 JSON 参数以 UTF-8 写进一个文件，再运行 " +
                program + L" call <工具名> @<文件路径>\n- 排查输入法问题（卡顿、候选窗口不见了）的步骤：" +
                program + L" prompt diagnose",
        "terminal instructions on windows");
  check(tokens_text(terminal_instructions(L"/usr/bin/msime-mcp", std::nullopt, {})) ==
            L"水杉输入法（MSIME）可以在终端里直接管理：\n- 查看可用的工具和参数：/usr/bin/msime-mcp tools\n- 调用一个工具，参数是 JSON 对象，输出 JSON：/usr/bin/msime-mcp call <工具名> '<JSON 参数>'，参数中有单引号时改为写进 UTF-8 文件并传 @<文件路径>\n- 排查输入法问题（卡顿、候选窗口不见了）的步骤：/usr/bin/msime-mcp prompt diagnose",
        "terminal instructions on posix");
  check(kinds(terminal_instructions(command, options, {}), Syntax::placeholder) ==
            L"<工具名>|@<文件路径>|",
        "placeholders");

  // serde_json 的缩进格式：没有权限时原文照旧并按词法着色，有权限时接在 args 末尾，排法与 JSON.stringify(…, null, 2) 相同。
  const std::wstring config =
      L"{\n  \"mcpServers\": {\n    \"msime\": {\n      \"args\": [\n        \"--options\",\n        \"C:\\\\Users\\\\me\\\\runtime-options.json\"\n      ],\n      \"command\": \"C:\\\\msime-mcp.exe\"\n    }\n  }\n}";
  const auto plain_config = config_tokens(config, {});
  check(tokens_text(plain_config) == config, "config without flags is unchanged");
  check(kinds(plain_config, Syntax::key) == L"\"mcpServers\"|\"msime\"|\"args\"|\"command\"|",
        "keys are coloured as keys");
  check(kinds(plain_config, Syntax::string) ==
            L"\"--options\"|\"C:\\\\Users\\\\me\\\\runtime-options.json\"|\"C:\\\\msime-mcp.exe\"|",
        "values are coloured as strings");
  const std::wstring with_flags =
      L"{\n  \"mcpServers\": {\n    \"msime\": {\n      \"args\": [\n        \"--options\",\n        \"C:\\\\Users\\\\me\\\\runtime-options.json\",\n        \"--allow-write\",\n        \"--allow-dictionary-read\"\n      ],\n      \"command\": \"C:\\\\msime-mcp.exe\"\n    }\n  }\n}";
  check(tokens_text(config_tokens(config, both)) == with_flags, "flags are appended to args");
  const std::wstring edition =
      L"{\n  \"mcpServers\": {\n    \"msime-wubi\": {\n      \"command\": \"x\",\n      \"args\": [\n        \"--options\"\n      ]\n    }\n  }\n}";
  check(tokens_text(config_tokens(edition, write)) ==
            L"{\n  \"mcpServers\": {\n    \"msime-wubi\": {\n      \"command\": \"x\",\n      \"args\": [\n        \"--options\",\n        \"--allow-write\"\n      ]\n    }\n  }\n}",
        "another edition's server name and key order");
  check(tokens_text(config_tokens(L"{\"mcpServers\":{\"msime\":{\"args\":[\"--options\"]}}}", write)) ==
            L"{\"mcpServers\":{\"msime\":{\"args\":[\"--options\", \"--allow-write\"]}}}",
        "a one-line array gets an inline flag");
  check(tokens_text(config_tokens(L"{\"mcpServers\":{\"msime\":{\"args\":[]}}}", both)) ==
            L"{\"mcpServers\":{\"msime\":{\"args\":[\"--allow-write\", \"--allow-dictionary-read\"]}}}",
        "an empty array");
  // 读不懂时原文不着色，也不接权限。
  const std::wstring broken = L"{\"mcpServers\": \"oops";
  check(config_tokens(broken, both).size() == 1 && config_tokens(broken, both)[0].kind == Syntax::plain &&
            tokens_text(config_tokens(broken, both)) == broken,
        "an unterminated string falls back");
  const std::wstring no_args = L"{\"mcpServers\": {\"msime\": {\"command\": \"x\"}}}";
  check(config_tokens(no_args, both).size() == 1 && tokens_text(config_tokens(no_args, both)) == no_args,
        "no args falls back");
  const std::wstring nested = L"{\"other\": {\"msime\": {\"args\": [\"a\"]}}}";
  check(tokens_text(config_tokens(nested, both)) == nested, "args outside mcpServers are left alone");

  if (failures == 0)
    std::puts("mcp connect: ok");
  return failures == 0 ? 0 : 1;
}
