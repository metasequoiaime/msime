#pragma once

#ifdef __cplusplus
#include <string>

// 仅负责本进程的崩溃落盘，不依赖遥测、偏好、宿主 ABI 或网络。调用方在正常启动时准备路径和记录头。
namespace msime::crash {
void arm(const std::string &path, const std::string &header);
void disarm();
bool is_armed();
void install_crash_handlers();
void remove_crash_handlers();
std::string current_stack(int skip);
void record(const char *summary);
}

extern "C" {
#endif
// 供 Rust/JNI 在读取用户同意后启用；空路径会停用。只能在正常执行路径调用。
void msime_native_crash_arm(const char *path, const char *header);
void msime_native_crash_disarm(void);
void msime_native_crash_install(void);
// 供允许分配内存的 panic/terminate 回调使用，摘要由调用方提供固定类型，不含异常里的用户数据。
void msime_native_crash_record(const char *summary);
#ifdef __cplusplus
}
#endif
