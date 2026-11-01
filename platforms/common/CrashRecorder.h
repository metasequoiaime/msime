#pragma once

#ifdef __cplusplus
#include <string>

// 仅负责本进程的崩溃落盘，不依赖遥测、偏好、宿主 ABI 或网络。调用方在正常启动时准备路径和记录头。
namespace msime::crash {
void arm(const std::string &path, const std::string &header);
void disarm();
bool is_armed();
// 模块快照只在 arm 时建立；武装后再加载的库（例如按需 dlopen/System.load 的语音运行时）加载后调用它，崩溃帧才能对上模块名和 build-id。只能在正常执行路径调用。
void refresh_modules();
void install_crash_handlers();
void remove_crash_handlers();
std::string current_stack(int skip);
void record(const char *summary);
#ifdef _WIN32
// install_crash_handlers 把本进程号写进这个进程环境变量：同一进程里加载的 TSF 据此在 WER 快照里标明记录器在，助手不再重复记能到达未处理异常过滤器的故障（一次故障只留进程内这一份，带完整栈和本进程的组件），只补记绕过过滤器的快速失败（__fastfail、/GS、CFG、堆损坏）。值带进程号，子进程继承到时不会误判。
constexpr wchar_t in_process_recorder_variable[] = L"MSIME_INPROCESS_CRASH_RECORDER";
#endif
}

extern "C" {
#endif
// 供 Rust/JNI 在读取用户同意后启用；空路径会停用。只能在正常执行路径调用。
void msime_native_crash_arm(const char *path, const char *header);
void msime_native_crash_disarm(void);
void msime_native_crash_install(void);
// 武装后加载新的动态库后调用，重新建立模块快照；只能在正常执行路径调用。
void msime_native_crash_refresh_modules(void);
// 供允许分配内存的 panic/terminate 回调使用，摘要由调用方提供固定类型，不含异常里的用户数据。
void msime_native_crash_record(const char *summary);
#ifdef __cplusplus
}
#endif
