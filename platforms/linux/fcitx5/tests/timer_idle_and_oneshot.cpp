#include "../FcitxIdleWatch.h"

#include <fcitx-utils/event.h>

#include <cerrno>
#include <cstdint>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <stdexcept>
#include <string>

namespace {

void require(bool condition, const char *message) {
  if (!condition) throw std::runtime_error(message);
}

void writeFile(const std::filesystem::path &path, const std::string &content) {
  std::filesystem::create_directories(path.parent_path());
  std::ofstream out(path, std::ios::binary | std::ios::trunc);
  out << content;
  out.flush();
  require(static_cast<bool>(out), "写入合成测试文件失败");
}

void atomicReplaceFile(const std::filesystem::path &path,
                       const std::string &content) {
  auto staged = path;
  staged += ".staged";
  writeFile(staged, content);
  std::filesystem::rename(staged, path);
}

void runLoopForUs(fcitx::EventLoop &loop, std::uint64_t duration_us) {
  auto stop = loop.addTimeEvent(
      CLOCK_MONOTONIC, fcitx::now(CLOCK_MONOTONIC) + duration_us, 1,
      [&](fcitx::EventSourceTime *timer, std::uint64_t) {
        timer->setEnabled(false);
        loop.exit();
        return true;
      });
  loop.exec();
}

int failingInotifyInit(int) {
  errno = EMFILE;
  return -1;
}

// 验证真实 `libFcitx5Utils` (`sd-event`) 一次性定时器调 `setEnabled(false)` 只触发一次、不会重装过期截止时刻造成紧循环；
// 同时覆盖与 `Fcitx5 5.0.21` 兼容的接口（不使用 5.1.x 才引入的 `addPostEvent`）。
void testOneShotDisarmWithoutSpin() {
  fcitx::EventLoop loop;
  std::uint64_t fires = 0;
  std::uint64_t hud_invocations = 0;
  bool hud_aux_empty = true;

  auto oneshot = loop.addTimeEvent(
      CLOCK_MONOTONIC, fcitx::now(CLOCK_MONOTONIC) + 2'000, 1,
      [&](fcitx::EventSourceTime *timer, std::uint64_t) {
        timer->setEnabled(false);
        ++fires;
        return true;
      });
  // 模拟 `mode_hud_aux_timer_` 提前返回路径：先关闭自身再检查文本状态。
  auto hud_timer = loop.addTimeEvent(
      CLOCK_MONOTONIC, fcitx::now(CLOCK_MONOTONIC) + 4'000, 1,
      [&](fcitx::EventSourceTime *timer, std::uint64_t) {
        timer->setEnabled(false);
        ++hud_invocations;
        if (hud_aux_empty) return true;
        return true;
      });

  runLoopForUs(loop, 35'000);
  require(fires == 1, "一次性定时器在窗口期内必须只触发一次");
  require(!oneshot->isEnabled(), "一次性定时器触发后必须处于关闭状态");
  require(hud_invocations == 1, "提前返回路径上的一次性定时器也必须只触发一次");
  require(!hud_timer->isEnabled(), "提前返回路径上的一次性定时器触发后必须处于关闭状态");
}

// 验证 `FcitxIdleWatch` 空闲零武装定时器、忽略宿主自写噪声文件、50ms 防抖合并及缺失父目录创建后的监听恢复。
void testIdleZeroWakeupAndInotifyDebounce() {
  char pattern[] = "/tmp/msime-fcitx-idle-watch-XXXXXX";
  const char *raw_dir = ::mkdtemp(pattern);
  require(raw_dir != nullptr, "创建隔离临时目录失败");
  const std::filesystem::path root(raw_dir);

  struct Cleanup {
    std::filesystem::path path;
    ~Cleanup() {
      std::error_code ignored;
      std::filesystem::remove_all(path, ignored);
    }
  } cleanup{root};

  const auto config_dir = root / "config" / "msime-client";
  const auto prefs_dir = root / "state" / "prefs";
  const auto user_data_dir = root / "state" / "user-data";
  const auto runtime_dir = root / "run" / "msime-client";
  const auto clipboard_dir = root / "state" / "clipboard";
  // `classicui.conf` 所在的 `fcitx5/conf` 故意先不创建，验证祖先目录逐步建立后的监听迁移。
  const auto classicui_conf = root / "config" / "fcitx5" / "conf" / "classicui.conf";
  const auto runtime_options = config_dir / "runtime-options.json";
  const auto quiesce_lease = user_data_dir / ".msime-dictionary-quiesce";
  const auto clipboard_file = clipboard_dir / "clipboard_history.json";

  std::filesystem::create_directories(prefs_dir);
  std::filesystem::create_directories(user_data_dir);
  std::filesystem::create_directories(runtime_dir);
  std::filesystem::create_directories(clipboard_dir);

  fcitx::EventLoop loop;
  std::uint64_t callbacks = 0;
  msime::fcitx_host::FcitxIdleWatch watch(loop, [&] { ++callbacks; });
  require(!watch.usingFallback(), "正常宿主环境应启用 inotify 而非回退定时器");

  ::setenv("XDG_CONFIG_HOME", (root / "config").c_str(), 1);
  ::setenv("XDG_RUNTIME_DIR", (root / "run").c_str(), 1);
  ::unsetenv("MSIME_FCITX5_OPTIONS");
  auto cfg = msime::fcitx_host::buildBaseFcitxWatchConfig(
      msime::fcitx_host::resolveFcitxConfigHome(), "msime-client", nullptr);
  const std::string prefs_str = prefs_dir.string();
  const std::string user_data_str = user_data_dir.string();
  const std::string clipboard_str = clipboard_dir.string();
  msime::fcitx_host::appendContextWatchPaths(
      cfg, prefs_str, user_data_str, ".msime-dictionary-quiesce", clipboard_str, {});
  watch.configure(cfg);
  require(!watch.hasArmedTimer(), "inotify 就绪后空闲状态不得挂起任何定时器");

  // `sd-event` 在 `loop.exit()` 后进入终态不可重入，使用单次长生命周期 `loop.exec()` 按阶段推进验收。
  int phase = 0;
  auto driver = loop.addTimeEvent(
      CLOCK_MONOTONIC, fcitx::now(CLOCK_MONOTONIC) + 80'000, 1,
      [&](fcitx::EventSourceTime *timer, std::uint64_t) {
        switch (phase++) {
        case 0:
          // 阶段 0：80 毫秒完全空闲，没有外部文件变化也没有武装的内部定时器。
          require(callbacks == 0, "无外部文件变化时不应触发刷新回调");
          require(!watch.hasArmedTimer(), "空闲期间监听器不得挂起任何定时器");
          // 写入宿主自身状态与锁文件，验证不会触发自激唤醒。
          writeFile(runtime_dir / "input-status.json", "{\"active\":true}\n");
          writeFile(runtime_dir / "candidate-panel.json", "{\"host\":\"fcitx5\"}\n");
          writeFile(runtime_dir / "panel-input.sock", "");
          writeFile(prefs_dir / "diagnostic.log", "focus_in\n");
          writeFile(prefs_dir / "preferences.json.lock", "");
          writeFile(user_data_dir / ".msime-dictionary-quiesce.lock", "");
          timer->setNextInterval(80'000);
          timer->setOneShot();
          return true;
        case 1:
          require(callbacks == 0, "宿主自身状态与锁文件不应触发外部刷新回调");
          require(!watch.hasArmedTimer(), "噪声文件写入后不得武装防抖定时器");
          // 同一突发窗口内连续修改多处受监听目标，验证 50 毫秒防抖合并为单次回调。
          atomicReplaceFile(prefs_dir / "preferences.json", "{\"revision\":1}\n");
          atomicReplaceFile(runtime_options, "{\"preferences\":{}}\n");
          writeFile(runtime_dir / "voice.sock", "");
          writeFile(clipboard_file, "{\"entries\":[]}\n");
          timer->setNextInterval(110'000);
          timer->setOneShot();
          return true;
        case 2:
          require(callbacks == 1, "突发外部写入应经 50ms 防抖合并为一次回调");
          require(!watch.hasArmedTimer(), "防抖回调完成后必须解除防抖定时器");
          timer->setNextInterval(80'000);
          timer->setOneShot();
          return true;
        case 3:
          require(callbacks == 1, "防抖回调完成后不得重装定时器");
          require(!watch.hasArmedTimer(), "空闲窗口内防抖定时器必须持续处于关闭状态");
          // 先前不存在的 `fcitx5/conf/classicui.conf` 建立时应触发回调并迁移监听。
          writeFile(classicui_conf, "Theme=default\n");
          timer->setNextInterval(100'000);
          timer->setOneShot();
          return true;
        case 4:
          require(callbacks == 2, "新建父目录并写出 classicui.conf 应触发回调");
          require(!watch.hasArmedTimer(), "迁移监听回调完成后不得残留已武装定时器");
          atomicReplaceFile(classicui_conf, "Theme=msime\n");
          atomicReplaceFile(quiesce_lease, "9999999999999\n1 0\n");
          timer->setNextInterval(100'000);
          timer->setOneShot();
          return true;
        case 5:
          require(callbacks == 3, "已迁移目录上的原子替换与词库租约变更应合并触发回调");
          require(!watch.hasArmedTimer(), "二次触发防抖完成后不得残留已武装定时器");
          watch.setActive(false);
          atomicReplaceFile(prefs_dir / "preferences.json", "{\"revision\":2}\n");
          timer->setNextInterval(90'000);
          timer->setOneShot();
          return true;
        default:
          require(callbacks == 3, "监听器停用后不应再投递回调");
          require(!watch.hasArmedTimer(), "监听器停用后不得挂起任何定时器");
          timer->setEnabled(false);
          loop.exit();
          return true;
        }
      });
  loop.exec();
  require(phase == 7, "inotify 防抖全部测试阶段应完整执行");
}

// 验证 `inotify_init1 < 0` 时启用受控回退定时器，并在停用后立即恢复零唤醒。
void testInotifyFailureFallback() {
  fcitx::EventLoop loop;
  std::uint64_t callbacks = 0;
  msime::fcitx_host::FcitxIdleWatch fallback_watch(
      loop, [&] { ++callbacks; }, 50'000, 20'000, &failingInotifyInit);
  require(fallback_watch.usingFallback(), "inotify 初始化失败时应报告回退模式");

  msime::fcitx_host::FcitxWatchConfig cfg;
  cfg.files = {"/tmp/msime-synthetic-fallback-options.json"};
  fallback_watch.configure(cfg);
  require(fallback_watch.hasArmedTimer(), "回退模式存在监听目标时应启用回退定时器");

  std::uint64_t settled = 0;
  int phase = 0;
  auto driver = loop.addTimeEvent(
      CLOCK_MONOTONIC, fcitx::now(CLOCK_MONOTONIC) + 55'000, 1,
      [&](fcitx::EventSourceTime *timer, std::uint64_t) {
        if (phase++ == 0) {
          require(callbacks >= 2, "inotify 不可用时回退定时器应按间隔触发回调");
          fallback_watch.setActive(false);
          require(!fallback_watch.hasArmedTimer(), "停用后必须立即关闭回退定时器");
          settled = callbacks;
          timer->setNextInterval(55'000);
          timer->setOneShot();
          return true;
        }
        require(callbacks == settled, "回退模式在无活动上下文停用后不应继续轮询");
        require(!fallback_watch.hasArmedTimer(), "回退定时器停用后不得自行重装");
        timer->setEnabled(false);
        loop.exit();
        return true;
      });
  loop.exec();
  require(phase == 2, "回退模式测试阶段应完整执行");
}

} // namespace

int main() {
  testOneShotDisarmWithoutSpin();
  testIdleZeroWakeupAndInotifyDebounce();
  testInotifyFailureFallback();
  std::cout << "Fcitx5 一次性定时器不重装、空闲零唤醒与 inotify 防抖/回退回归通过\n";
  return 0;
}
