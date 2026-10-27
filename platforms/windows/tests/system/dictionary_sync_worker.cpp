#include "system/DictionarySyncWorker.h"
#include "msime_client.h"
#include <atomic>
#include <cassert>
#include <cstdint>
#include <cstdlib>
#include <thread>

// 本测试不连接 Rust 或网盘；未配置的 worker 不应调用宿主接口。
extern "C" char *msime_client_dictionary_sync(const uint8_t *, size_t) {
  std::abort();
}

extern "C" void msime_client_string_free(char *value) { std::free(value); }

int main() {
  for (unsigned attempt = 0; attempt < 64; ++attempt) {
    msime::windows::DictionarySyncWorker worker(false);
    worker.start({});
    std::atomic<bool> running{true};
    std::atomic<unsigned> turns{0};
    const auto notify = [&] {
      while (running.load(std::memory_order_acquire)) {
        worker.configure(false);
        worker.note_local_change();
        turns.fetch_add(1, std::memory_order_release);
      }
    };
    std::thread preferences(notify);
    std::thread deletions(notify);
    // 先等到真实通知已在运行，再与停机重叠，不靠固定睡眠猜时序。
    while (turns.load(std::memory_order_acquire) < 32)
      std::this_thread::yield();
    worker.stop();
    running.store(false, std::memory_order_release);
    preferences.join();
    deletions.join();
    // 停机后的迟到通知、重复停机和再次 start 都必须安全且不能恢复调度。
    worker.configure(true);
    worker.note_local_change();
    worker.start({});
    worker.stop();
    assert(turns.load(std::memory_order_acquire) >= 32);
  }
}
