#include "../src/voice/VoiceAction.h"
#include "../src/voice/VoiceWorker.h"

#include <atomic>
#include <cassert>
#include <chrono>
#include <stdexcept>
#include <thread>

int main() {
  assert(msime_voice_bound_result("水杉") == "水杉");
  auto oversized = msime_voice_bound_result("水杉水杉", 6);
  assert(oversized == "水杉");
  assert(msime_voice_result_or_transcript("最终", "中间", "预编辑") == "最终");
  assert(msime_voice_result_or_transcript("", "中间😀", "预编辑") == "中间😀");
  assert(msime_voice_result_or_transcript("", "", "预编辑") == "预编辑");
  assert(msime_voice_result_or_transcript("", "", "") == "");
  assert(std::string(msime_voice_provider_failure_notice("voice_dependency_missing:websockets")) ==
         "豆包语音需要 websockets 15 或更高版本，请安装 python3-websockets");
  assert(std::string(msime_voice_provider_failure_notice("voice_dependency_missing:recorder")) ==
         "未找到录音工具，请安装 pulseaudio-utils、pipewire-bin 或 alsa-utils");
  assert(std::string(msime_voice_provider_failure_notice("voice_dependency_missing:local_asr")) ==
         "本地语音识别组件无法加载，请重新安装输入法");
  assert(std::string(msime_voice_provider_failure_notice("")) ==
         "语音输入失败，请检查语音服务、麦克风及提供商配置后重试");
  assert(std::string(msime_voice_provider_failure_notice("voice_dependency_missing:token=secret")) ==
         "语音输入失败，请检查语音服务、麦克风及提供商配置后重试");

  MsimeVoiceWorker worker;
  std::atomic_bool delivered{false};
  worker.run(
      [](const std::atomic_bool &cancelled) {
        while (!cancelled.load())
          std::this_thread::sleep_for(std::chrono::milliseconds(1));
        return std::string("cancelled");
      },
      [&](std::string) { delivered.store(true); });
  worker.cancel();
  assert(!delivered.load());

  worker.run([](const std::atomic_bool &) { return std::string("完成"); },
             [&](std::string value) {
               delivered.store(value == "完成");
             });
  for (int i = 0; i < 100 && !delivered.load(); ++i)
    std::this_thread::sleep_for(std::chrono::milliseconds(1));
  worker.cancel();
  assert(delivered.load());

  worker.run(
      [](const std::atomic_bool &) -> std::string {
        throw std::runtime_error("synthetic provider failure");
      },
      [&](std::string) { delivered.store(false); });
  std::this_thread::sleep_for(std::chrono::milliseconds(20));
  worker.cancel();
  assert(delivered.load());

  worker.run_stream(
      [](const std::atomic_bool &, const MsimeVoiceWorker::Progress &) -> std::string {
        throw std::runtime_error("synthetic streaming provider failure");
      },
      {},
      [&](std::string) { delivered.store(false); });
  std::this_thread::sleep_for(std::chrono::milliseconds(20));
  worker.cancel();
  assert(delivered.load());
}
