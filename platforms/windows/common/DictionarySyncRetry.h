#pragma once

#include <string_view>

// 词库同步的宿主在密钥串身份冲突时怎么重试。不依赖 Windows 和 JSON 头文件：Server（nlohmann）、WinUI 设置（WinRT JSON）和 Win7 窗口各自把回答里的错误码交给这里判断，主机上的单元测试也能直接用。
namespace msime::windows::dictionary_sync {

// 共享层的约定（#10032，docs/cloud-clipboard-e2e.md 的「宿主的职责」）：一个安装只有一个设备身份，密钥串的保存从不覆盖另一个身份。同一个安装的两个进程（Server 和设置窗口）第一次使用时同时生成身份，后存的一方在发出请求之前改用存着的那份重来，服务端只登记一台；已经发出请求之后才发现时报这个码，宿主重试一次就会用存着的那份身份。
inline constexpr std::string_view keystore_conflict = "clipboard_keystore_conflict";

// 最多调用几次（含第一次）。再冲突说明身份在这次操作期间又被换了，交给用户稍后重试，不在这里无限循环。
inline constexpr int keystore_conflict_attempts = 3;

// 调 `call()`，回答被 `conflicted` 认作身份冲突时再调，最多 `keystore_conflict_attempts` 次，返回最后一次的回答。`call` 阻塞联网，只在工作线程上用。
template <typename Call, typename Conflicted> auto with_keystore_retry(Call &&call, Conflicted &&conflicted) {
  auto response = call();
  for (int attempt = 1; attempt < keystore_conflict_attempts && conflicted(response); ++attempt)
    response = call();
  return response;
}

} // namespace msime::windows::dictionary_sync
