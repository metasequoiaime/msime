#pragma once
#include <string>
#include <unordered_map>

namespace msime::windows {
// 日文释义行的罗马字读音，在翻译工作线程上用。分词和每个词的假名读音来自微软日语输入法的 IFELanguage（ProgID MSIME.Japan，Windows 自带的日语输入法注册它），由 JapaneseRomaji.h 转成罗马字。系统里没有这个组件（没装日语输入法）时不报错：只由假名组成的释义照样按假名读，含汉字的不标读音，和 macOS 读不全时一样。
//
// 第一次用时在本线程上初始化 COM（单线程套间；线程已经是多线程套间时沿用）并打开 IFELanguage，close() 关闭并反初始化，所以创建、使用和关闭都必须在同一个线程上，TranslationWorker::run 在工作线程的栈上持有一个实例保证这一点。
class JapaneseReader {
public:
  JapaneseReader() = default;
  // 调用 close()。实例要放在线程函数的栈上，让析构在线程函数返回前运行；不要做成 thread_local：那样析构在线程退出回调里持着加载器锁运行（MSVC 和 MinGW 都是），那里不能 CoUninitialize，也不能让 COM 卸载日语输入法的 DLL。
  ~JapaneseReader();
  JapaneseReader(const JapaneseReader &) = delete;
  JapaneseReader &operator=(const JapaneseReader &) = delete;

  // 一个日文词或短句（UTF-8）的罗马字，逐词用一个空格隔开；读不全时为空。结果按文字缓存。
  std::string romaji(const std::string &term);
  // 关闭 IFELanguage 并反初始化本线程上由它初始化的 COM；没打开过时什么也不做。之后再读会重新打开，缓存保留。
  void close();

private:
  bool open();
  std::string read(const std::string &term);

  void *language_ = nullptr;
  bool attempted_ = false;
  bool uninitialize_ = false;
  std::unordered_map<std::string, std::string> cache_;
};
} // namespace msime::windows
