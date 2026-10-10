#include "CuePlayer.h"

#include "miniaudio.h"
#include <windows.h>

#include <cstdio>

CuePlayer::CuePlayer() : engine_(new ma_engine()), start_sound_(new ma_sound()),
                         end_sound_(new ma_sound()) {}

CuePlayer::~CuePlayer() {
  shutdown();
  delete end_sound_;
  delete start_sound_;
  delete engine_;
}

bool CuePlayer::init(const std::wstring &start_path,
                     const std::wstring &end_path) {
  shutdown();
  if (ma_engine_init(nullptr, engine_) != MA_SUCCESS)
    return false;
  engine_initialized_ = true;
  load_sound(start_path, start_sound_, &start_loaded_, "start");
  load_sound(end_path, end_sound_, &end_loaded_, "end");
  return true;
}

void CuePlayer::shutdown() {
  if (start_loaded_) {
    ma_sound_uninit(start_sound_);
    start_loaded_ = false;
  }
  if (end_loaded_) {
    ma_sound_uninit(end_sound_);
    end_loaded_ = false;
  }
  if (engine_initialized_) {
    ma_engine_uninit(engine_);
    engine_initialized_ = false;
  }
}

// 提示音文件缺失、解码失败或音频引擎起不来时退回系统声音，开始和结束仍有声音反馈，对应 macOS 退回 NSSound 的 Glass 和 Pop。MessageBeep 是异步的，不会卡住调用它的控制线程。
bool CuePlayer::play_start() {
  if (play_sound(start_sound_, start_loaded_, "start"))
    return true;
  (void)MessageBeep(MB_ICONASTERISK);
  return false;
}
void CuePlayer::play_end() {
  if (!play_sound(end_sound_, end_loaded_, "end"))
    (void)MessageBeep(MB_OK);
}

bool CuePlayer::load_sound(const std::wstring &path, ma_sound *sound,
                           bool *loaded, const char *label) {
  *loaded = false;
  if (!engine_initialized_ || path.empty())
    return false;
  const auto path_utf8 = utf8(path);
  if (path_utf8.empty() || ma_sound_init_from_file(
                               engine_, path_utf8.c_str(), 0, nullptr, nullptr,
                               sound) != MA_SUCCESS) {
    std::fprintf(stderr, "[AUDIO] failed to load %s cue\n", label);
    return false;
  }
  *loaded = true;
  return true;
}

bool CuePlayer::play_sound(ma_sound *sound, bool loaded, const char *label) {
  if (!engine_initialized_ || !loaded)
    return false;
  ma_sound_stop(sound);
  if (ma_sound_seek_to_pcm_frame(sound, 0) != MA_SUCCESS ||
      ma_sound_start(sound) != MA_SUCCESS) {
    std::fprintf(stderr, "[AUDIO] failed to play %s cue\n", label);
    return false;
  }
  return true;
}

std::string CuePlayer::utf8(const std::wstring &value) {
  if (value.empty())
    return {};
  const int size = WideCharToMultiByte(CP_UTF8, 0, value.data(),
                                       static_cast<int>(value.size()), nullptr,
                                       0, nullptr, nullptr);
  if (size <= 0)
    return {};
  std::string result(static_cast<size_t>(size), '\0');
  if (WideCharToMultiByte(CP_UTF8, 0, value.data(),
                          static_cast<int>(value.size()), result.data(), size,
                          nullptr, nullptr) != size)
    return {};
  return result;
}
