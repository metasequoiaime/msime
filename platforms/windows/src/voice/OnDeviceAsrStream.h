#pragma once

#include <cstddef>
#include <exception>
#include <string>

namespace msime::windows {
// 边录边识别、在本机完成的识别器：本地模型（LocalAsrStream）和 Windows 系统识别（SystemAsrStream）。VoiceInputSession 只通过这几个调用和它打交道，识别任务本身由各实现的 run() 在录音开始时启动。
class OnDeviceAsrStream {
public:
  virtual ~OnDeviceAsrStream() = default;
  // 采集线程调用，不得阻塞。返回 false 表示有界队列已满，这次录音应当作中断处理。
  virtual bool push(const float *samples, std::size_t count) = 0;
  // 录音结束后的识别任务调用：不会再有音频了。等识别器处理完剩余音频并返回整段文字；识别器失败时重新抛出它的异常。
  virtual std::string finish() = 0;
  // 任意线程、任意次数，finish() 之后也可以。识别器在下一次检查时停下，run() 不再给出文字。
  virtual void cancel() = 0;
  // 控制线程在录音中每轮调用，不阻塞：识别任务已经因出错结束时返回那个异常（finish() 会重新抛出的同一个），否则为空。取消和队列溢出造成的停止不算失败，由调用方各自处理。
  virtual std::exception_ptr failure() = 0;
};
} // namespace msime::windows
