#include "PrepareHost.h"
#include "msime_client.h"
#include "../../../common/HostApiString.h"
#include "../../common/AuxiliaryCrashCapture.h"
#include <iostream>

int wmain(int argc, wchar_t **argv) {
  if (argc == 2 && std::wstring(argv[1]) == L"--help") {
    std::cout << "Usage: msime-client-prepare <absolute-resource-directory> "
                 "<absolute-new-state-directory>\n"
                 "Run as the input-method user before starting Server or TSF.\n"
                 "The state directory must not exist; its parent must exist.\n";
    return 0;
  }
  if (argc != 3) {
    std::cerr << "Expected absolute resource and new state directories\n";
    return 2;
  }
  try {
    std::unique_ptr<msime::telemetry::CaptureOnly> crash_capture;
    const std::filesystem::path state(argv[2]);
    msime::windows::prepare_host_state(argv[1], state, [&](const std::string &request) {
      // 独占建目录之后才按包内版本读取新偏好；既不能抢建目录，也不能在版本标记之前保存 full 默认值。
      crash_capture = std::make_unique<msime::telemetry::CaptureOnly>([state] {
        return msime::windows::auxiliary_crash_host("prepare", MSIME_WINDOWS_VERSION, state);
      });
      auto response = msime::host_api::own_string(
          msime_client_prepare_host(
              reinterpret_cast<const uint8_t *>(request.data()), request.size()));
      if (!response)
        throw std::runtime_error("Shared host preparation failed");
      return std::string(response.get());
    });
    std::cout << "Prepared runtime-options.json successfully\n";
    return 0;
  } catch (...) {
    // 宿主错误可能带私人路径，不转发原始内容。
    std::cerr << "Preparation failed; existing and partially prepared data retained\n";
    return 1;
  }
}
