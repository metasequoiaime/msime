#pragma once

#include <sys/stat.h>

#include <atomic>
#include <bitset>
#include <cerrno>
#include <chrono>
#include <condition_variable>
#include <cstddef>
#include <cstdint>
#include <ctime>
#include <map>
#include <mutex>
#include <optional>
#include <string>
#include <string_view>
#include <utility>

namespace msime::linux_host {

enum class TypingSource {
  Quanpin,
  NineKey,
  Shuangpin,
  Ziranma,
  Microsoft,
  Shoudao,
  Wubi,
  Japanese,
  Korean,
  Cantonese,
  Zhuyin,
  Vietnamese,
  Tibetan,
  Stroke,
  Handwriting,
  English,
  Local,
  Ai,
  Reply,
  Voice,
  Unknown,
};

constexpr std::string_view typing_source_id(TypingSource source) {
  switch (source) {
  case TypingSource::Quanpin:
    return "quanpin";
  case TypingSource::NineKey:
    return "nineKey";
  case TypingSource::Shuangpin:
    return "shuangpin";
  case TypingSource::Ziranma:
    return "ziranma";
  case TypingSource::Microsoft:
    return "microsoft";
  case TypingSource::Shoudao:
    return "shoudao";
  case TypingSource::Wubi:
    return "wubi";
  case TypingSource::Japanese:
    return "japanese";
  case TypingSource::Korean:
    return "korean";
  case TypingSource::Cantonese:
    return "cantonese";
  case TypingSource::Zhuyin:
    return "zhuyin";
  case TypingSource::Vietnamese:
    return "vietnamese";
  case TypingSource::Tibetan:
    return "tibetan";
  case TypingSource::Stroke:
    return "stroke";
  case TypingSource::Handwriting:
    return "handwriting";
  case TypingSource::English:
    return "english";
  case TypingSource::Local:
    return "local";
  case TypingSource::Ai:
    return "ai";
  case TypingSource::Reply:
    return "reply";
  case TypingSource::Voice:
    return "voice";
  case TypingSource::Unknown:
    return "unknown";
  }
  return "unknown";
}

// 共享 Engine 在 View 里用数字表示方案：0 全拼、1 双拼、2 五笔、3 日文、4 韩文、5 粤拼、6 注音、7 越南文、8 藏文、9 笔画。局部模式优先于键盘方案，与 Android 和 Apple 宿主一致。
constexpr TypingSource
resolve_typing_source(int scheme, bool nine_key, bool dedicated_english,
                      std::string_view local_mode,
                      std::string_view shuangpin_profile) {
  if (local_mode == "temporary_japanese")
    return TypingSource::Japanese;
  if (!local_mode.empty() && local_mode != "none")
    return TypingSource::Local;
  if (dedicated_english)
    return TypingSource::English;
  switch (scheme) {
  case 0:
    return nine_key ? TypingSource::NineKey : TypingSource::Quanpin;
  case 1:
    if (shuangpin_profile == "ziranma")
      return TypingSource::Ziranma;
    if (shuangpin_profile == "microsoft")
      return TypingSource::Microsoft;
    if (shuangpin_profile == "shoudao")
      return TypingSource::Shoudao;
    return TypingSource::Shuangpin;
  case 2:
    return TypingSource::Wubi;
  case 3:
    return TypingSource::Japanese;
  case 4:
    return TypingSource::Korean;
  case 5:
    return TypingSource::Cantonese;
  case 6:
    return TypingSource::Zhuyin;
  case 7:
    return TypingSource::Vietnamese;
  case 8:
    return TypingSource::Tibetan;
  case 9:
    return TypingSource::Stroke;
  default:
    return TypingSource::Unknown;
  }
}

// Modifiers that turn a key press into a shortcut rather than typed text. Shift and the level-3/level-5 shifts are deliberately absent: they select which character a key produces, so a character typed with them is still text.
struct PassthroughModifiers {
  bool control = false;
  bool alt = false;
  bool super = false;
  bool hyper = false;
  bool meta = false;
};

// Whether a key the IME handed back to the application still counts as a typed character. Mirrors the Windows ShouldCountPassthroughChar rule: no shortcut modifier held, and a printable scalar value - no C0 control, no DEL, no surrogate, nothing past U+10FFFF. The caller has already ruled out releases, keys the IME consumed, and blocked or private contexts.
constexpr bool should_count_passthrough_character(char32_t character,
                                                  PassthroughModifiers held) {
  if (held.control || held.alt || held.super || held.hyper || held.meta)
    return false;
  if (character < 0x20 || character == 0x7f || character > 0x10ffff)
    return false;
  return character < 0xd800 || character > 0xdfff;
}

// The typing-statistics master switch, cached so that an opt-out stops at the capture boundary: with statistics off a commit formats no date, serializes no request, starts no worker and never takes the store's lock or reads its file. This matches the Windows server, which keeps the switch in an atomic loaded with its configuration, and the macOS host, which caches the same FFI answer.
//
// Hosts refresh it at startup and on every preference reload tick. The switch lives in the statistics document rather than in the preferences, so a tick cannot learn about a change from the preference revision; instead it compares the document's identity (directory, device, inode, size, modification time) with the one it last read, and only asks the store again when that changed. An idle tick costs one stat. The store replaces the document by rename on every write, so turning statistics on or off in the settings always changes that identity.
//
// Until a read succeeds the switch is off, the privacy-preserving default the macOS host uses too. Commits made between turning statistics on and the next tick are not recorded.
class TypingStatisticsSwitch {
public:
  // msime_client_typing_statistics_enabled in the hosts: 1 on, 0 off, negative when the store cannot be read.
  using Query = int32_t (*)(const uint8_t *directory, std::size_t length);

  explicit TypingStatisticsSwitch(Query query) : query_(query) {}

  bool enabled() const { return enabled_.load(std::memory_order_relaxed); }

  // Safe to call from any thread. Hosts call it from their preference workers, so a read that waits on the store's lock while another commit is being written never stalls the event loop.
  void refresh(const std::string &directory) {
    std::lock_guard<std::mutex> guard(mutex_);
    Identity next;
    next.directory = directory;
    // The same bound and absolute-path rule the FFI enforces; anything else cannot hold statistics.
    if (directory.empty() || directory.front() != '/' || directory.size() > 16384) {
      enabled_.store(false, std::memory_order_relaxed);
      read_ = false;
      return;
    }
    struct stat info {};
    if (::stat((directory + "/typing-statistics.json").c_str(), &info) == 0) {
      next.device = static_cast<uint64_t>(info.st_dev);
      next.inode = static_cast<uint64_t>(info.st_ino);
      next.size = static_cast<int64_t>(info.st_size);
      next.modified_seconds = static_cast<int64_t>(info.st_mtim.tv_sec);
      next.modified_nanoseconds = static_cast<int64_t>(info.st_mtim.tv_nsec);
    } else {
      next.error = errno;
    }
    if (read_ && next == identity_)
      return;
    const int32_t answer = query_(reinterpret_cast<const uint8_t *>(directory.data()), directory.size());
    enabled_.store(answer == 1, std::memory_order_relaxed);
    // A failed read stays off but is not remembered, so the next tick asks again instead of trusting a document that did not change.
    read_ = answer >= 0;
    identity_ = std::move(next);
  }

private:
  struct Identity {
    std::string directory;
    int error = 0;
    uint64_t device = 0;
    uint64_t inode = 0;
    int64_t size = 0;
    int64_t modified_seconds = 0;
    int64_t modified_nanoseconds = 0;
    bool operator==(const Identity &other) const {
      return directory == other.directory && error == other.error && device == other.device &&
             inode == other.inode && size == other.size && modified_seconds == other.modified_seconds &&
             modified_nanoseconds == other.modified_nanoseconds;
    }
  };

  Query query_;
  std::atomic_bool enabled_{false};
  std::mutex mutex_;
  Identity identity_;
  bool read_ = false;
};

// The key heatmap's id for an evdev key code (linux/input-event-codes.h): the W3C KeyboardEvent.code name, one of KEY_IDS in crates/client-core/src/typing_statistics.rs. IBus hands engines the evdev code itself; Fcitx5's raw key code is the XKB code, which is the evdev code plus 8. A key outside that list (PrintScreen, ScrollLock, Pause, F13 and up, media keys, and the JIS 半角/全角, katakana and hiragana codes, which XKB does not use for the keys on a JIS keyboard) answers empty and is not counted, because the store rejects a whole batch for one unknown id.
constexpr std::string_view evdev_key_id(uint32_t code) {
  switch (code) {
  case 1: return "Escape";
  case 2: return "Digit1";
  case 3: return "Digit2";
  case 4: return "Digit3";
  case 5: return "Digit4";
  case 6: return "Digit5";
  case 7: return "Digit6";
  case 8: return "Digit7";
  case 9: return "Digit8";
  case 10: return "Digit9";
  case 11: return "Digit0";
  case 12: return "Minus";
  case 13: return "Equal";
  case 14: return "Backspace";
  case 15: return "Tab";
  case 16: return "KeyQ";
  case 17: return "KeyW";
  case 18: return "KeyE";
  case 19: return "KeyR";
  case 20: return "KeyT";
  case 21: return "KeyY";
  case 22: return "KeyU";
  case 23: return "KeyI";
  case 24: return "KeyO";
  case 25: return "KeyP";
  case 26: return "BracketLeft";
  case 27: return "BracketRight";
  case 28: return "Enter";
  case 29: return "ControlLeft";
  case 30: return "KeyA";
  case 31: return "KeyS";
  case 32: return "KeyD";
  case 33: return "KeyF";
  case 34: return "KeyG";
  case 35: return "KeyH";
  case 36: return "KeyJ";
  case 37: return "KeyK";
  case 38: return "KeyL";
  case 39: return "Semicolon";
  case 40: return "Quote";
  case 41: return "Backquote";
  case 42: return "ShiftLeft";
  case 43: return "Backslash";
  case 44: return "KeyZ";
  case 45: return "KeyX";
  case 46: return "KeyC";
  case 47: return "KeyV";
  case 48: return "KeyB";
  case 49: return "KeyN";
  case 50: return "KeyM";
  case 51: return "Comma";
  case 52: return "Period";
  case 53: return "Slash";
  case 54: return "ShiftRight";
  case 55: return "NumpadMultiply";
  case 56: return "AltLeft";
  case 57: return "Space";
  case 58: return "CapsLock";
  case 59: return "F1";
  case 60: return "F2";
  case 61: return "F3";
  case 62: return "F4";
  case 63: return "F5";
  case 64: return "F6";
  case 65: return "F7";
  case 66: return "F8";
  case 67: return "F9";
  case 68: return "F10";
  case 69: return "NumLock";
  case 71: return "Numpad7";
  case 72: return "Numpad8";
  case 73: return "Numpad9";
  case 74: return "NumpadSubtract";
  case 75: return "Numpad4";
  case 76: return "Numpad5";
  case 77: return "Numpad6";
  case 78: return "NumpadAdd";
  case 79: return "Numpad1";
  case 80: return "Numpad2";
  case 81: return "Numpad3";
  case 82: return "Numpad0";
  case 83: return "NumpadDecimal";
  case 86: return "IntlBackslash";
  case 87: return "F11";
  case 88: return "F12";
  case 89: return "IntlRo";
  case 92: return "Convert";
  case 93: return "KanaMode";
  case 94: return "NonConvert";
  case 96: return "NumpadEnter";
  case 97: return "ControlRight";
  case 98: return "NumpadDivide";
  case 100: return "AltRight";
  case 102: return "Home";
  case 103: return "ArrowUp";
  case 104: return "PageUp";
  case 105: return "ArrowLeft";
  case 106: return "ArrowRight";
  case 107: return "End";
  case 108: return "ArrowDown";
  case 109: return "PageDown";
  case 110: return "Insert";
  case 111: return "Delete";
  case 122: return "Lang1";
  case 123: return "Lang2";
  case 124: return "IntlYen";
  case 125: return "MetaLeft";
  case 126: return "MetaRight";
  case 127: return "ContextMenu";
  case 464: return "Fn";
  default: return {};
  }
}

// The local calendar day of a moment, in the store's YYYY-MM-DD form; empty when the clock cannot be converted.
inline std::string local_day(std::time_t moment) {
  std::tm local{};
  if (localtime_r(&moment, &local) == nullptr)
    return {};
  char day[11]{};
  if (std::strftime(day, sizeof(day), "%Y-%m-%d", &local) == 0)
    return {};
  return day;
}

// Key presses counted for one preferences directory and one local day, ready for the store's record_keys operation.
struct KeyPressBatch {
  std::string directory;
  std::string day;
  std::map<std::string, uint64_t> keys;
};

// Per-key press counts a host keeps in memory between writes, so that the store, which takes its file lock and rewrites the whole document on every call, sees a batch at a time rather than one call per key. It holds nothing but counts: no order, no timing beyond the batch's age, no text.
//
// A batch belongs to the directory and the local day its presses were made in. A press for another day or directory hands the old batch back first, so counts made before midnight are written under that day however late they are flushed. The host also takes the batch when it reaches kFlushPresses, when it is kFlushAgeMicroseconds old, on focus loss and on teardown, and writes it off the event loop; only once the host process or addon is shutting down does it write the last batches on the loop itself, then wait for the writes still running (see PendingWrites).
//
// Only key downs count, and a held key counts once: down() remembers each key until its release, so the press events auto-repeat produces while it is held are ignored. Where the display server cannot report auto-repeat as such (X11 without detectable auto-repeat), every repeat arrives as a release and a press of the same key at the same moment; down() takes a press that follows the release of the same key within a repeat gap for such a repeat, a gap no finger can retype a key in. A release that never arrives (the focus moved while the key was down) is forgotten with forget_held() on focus loss.
//
// Runs on the host's event loop only; it has no locking of its own.
class KeyPressCounter {
public:
  static constexpr uint64_t kFlushPresses = 256;
  static constexpr int64_t kFlushAgeMicroseconds = 30'000'000;

  // The repeat gap for event timestamps, which the X server gives a synthetic release and press alike; it allows for the millisecond resolution of those stamps.
  static constexpr int64_t kEventRepeatGapMicroseconds = 2'000;
  // The repeat gap for a host that only knows when an event reached it (IBus passes no event time): the pair is sent back to back, but the release is answered over D-Bus before the press arrives. A deliberate release and press of one key takes tens of milliseconds.
  static constexpr int64_t kArrivalRepeatGapMicroseconds = 8'000;

  // A key went down at at_microseconds, on the clock up() was given. Answers its id when this is a new press of a key with one, and empty for a repeat of a held key, a press that follows its own release within repeat_gap_microseconds, or a key without an id; only then does the host need to read the day.
  std::string_view down(uint32_t evdev_code, int64_t at_microseconds, int64_t repeat_gap_microseconds) {
    const auto id = evdev_key_id(evdev_code);
    if (id.empty() || held_.test(evdev_code))
      return {};
    held_.set(evdev_code);
    if (evdev_code == released_code_ && at_microseconds >= released_microseconds_ &&
        at_microseconds - released_microseconds_ <= repeat_gap_microseconds)
      return {};
    return id;
  }
  // The key came up at at_microseconds, so its next press counts again unless it follows within the repeat gap.
  void up(uint32_t evdev_code, int64_t at_microseconds) {
    if (evdev_code < held_.size())
      held_.reset(evdev_code);
    released_code_ = evdev_code;
    released_microseconds_ = at_microseconds;
  }
  void forget_held() {
    held_.reset();
    released_code_ = kNoKey;
  }

  // Counts one press of id, an answer from down(). Answers a batch the host should write now: the previous one when the directory or day changed, or this one once it holds kFlushPresses presses.
  std::optional<KeyPressBatch> add(std::string_view id, const std::string &directory,
                                   const std::string &day, int64_t now_microseconds) {
    std::optional<KeyPressBatch> ready;
    if (presses_ > 0 && (batch_.directory != directory || batch_.day != day))
      ready = take();
    if (presses_ == 0) {
      batch_.directory = directory;
      batch_.day = day;
      started_microseconds_ = now_microseconds;
    }
    ++batch_.keys[std::string(id)];
    if (++presses_ >= kFlushPresses)
      return take();
    return ready;
  }
  // The pending batch, if any, leaving the counter empty. Held keys are kept: a key still down is not pressed again by a flush.
  std::optional<KeyPressBatch> take() {
    if (presses_ == 0)
      return std::nullopt;
    presses_ = 0;
    return std::exchange(batch_, KeyPressBatch{});
  }
  // The pending batch once its first press is kFlushAgeMicroseconds old, for the host's periodic tick.
  std::optional<KeyPressBatch> take_due(int64_t now_microseconds) {
    if (presses_ == 0 || now_microseconds - started_microseconds_ < kFlushAgeMicroseconds)
      return std::nullopt;
    return take();
  }
  uint64_t pending() const { return presses_; }

private:
  static constexpr uint32_t kNoKey = UINT32_MAX;
  // Every code evdev_key_id names is below 512 (KEY_FN is 464).
  std::bitset<512> held_;
  // The last key released and when, for telling a synthetic repeat from a new press.
  uint32_t released_code_ = kNoKey;
  int64_t released_microseconds_ = 0;
  KeyPressBatch batch_;
  uint64_t presses_ = 0;
  int64_t started_microseconds_ = 0;
};

// Counts the statistics writes a host runs off its event loop, so that on its way out (the IBus main loop has quit, the Fcitx5 addon is unloading) it can wait for them rather than exit or unload the library under them.
class PendingWrites {
public:
  void begin() {
    std::lock_guard lock(mutex_);
    ++running_;
  }
  void end() {
    {
      std::lock_guard lock(mutex_);
      --running_;
    }
    idle_.notify_all();
  }
  // Waits until no write is running or timeout has passed; answers whether they all finished.
  bool wait_idle(std::chrono::milliseconds timeout) {
    std::unique_lock lock(mutex_);
    return idle_.wait_for(lock, timeout, [this] { return running_ == 0; });
  }

private:
  std::mutex mutex_;
  std::condition_variable idle_;
  std::size_t running_ = 0;
};

} // namespace msime::linux_host
