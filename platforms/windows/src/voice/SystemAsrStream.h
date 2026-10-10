#pragma once

#include "LocalAsrAudioQueue.h"
#include "OnDeviceAsrStream.h"

#include <atomic>
#include <condition_variable>
#include <exception>
#include <functional>
#include <memory>
#include <mutex>
#include <optional>
#include <string>
#include <string_view>

namespace msime::windows {
// 「Windows 系统识别」的一次听写：SAPI 5.4 进程内识别器加听写语法，不需要 API Key，也不用下载模型，音频不离开本机。识别器读的不是它自己打开的麦克风，而是这次录音的采集回调推进来的音频，所以所选录音设备、电平和静音其他声音与其他识别服务完全一样。识别器给出的中间结果（假设）经 on_partial 送到内联预编辑或浮层，和豆包、本地模型一样边说边出字。
class SystemAsrStream final : public OnDeviceAsrStream {
public:
  using Partial = std::function<void(const std::string &)>;

  SystemAsrStream();

  bool push(const float *samples, std::size_t count) override;
  std::string finish() override;
  void cancel() override;
  std::exception_ptr failure() override;

  // 录音开始时启动的识别任务：在这个线程上初始化 COM、创建识别器并一直处理识别事件，直到音频结束、取消或识别器出错。
  void run(const std::string &language, const Partial &on_partial);

private:
  std::shared_ptr<std::atomic_bool> cancelled_ =
      std::make_shared<std::atomic_bool>(false);
  // 与交给识别器的 COM 音频流共用：识别器在自己的线程上读它，读到队列关闭就是流结束。
  std::shared_ptr<LocalAsrAudioQueue> queue_;
  std::mutex mutex_;
  std::condition_variable done_wake_;
  bool done_ = false;
  std::string result_;
  std::exception_ptr error_;
};

// 控制线程在开始录音前调用：这台电脑能不能用系统识别听写这种语言。能用时返回空；不能用时返回给人看的那句话（语言不支持、没装该语言的语音识别、系统识别组件不可用），这次录音就不开始。只读注册表里的识别器令牌，不加载识别引擎。
std::optional<std::string> system_asr_start_problem(std::string_view language);
} // namespace msime::windows
