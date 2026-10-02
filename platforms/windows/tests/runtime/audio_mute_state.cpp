#include "AudioMuteState.h"

#include <cassert>
#include <chrono>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <string>

int main() {
  namespace fs = std::filesystem;
  const auto root = fs::temp_directory_path() /
                    ("msime-audio-mute-state-" +
                     std::to_string(std::chrono::steady_clock::now()
                                        .time_since_epoch()
                                        .count()));
  fs::create_directories(root);
  const auto path = root / "state.txt";
  const auto outside = root / "outside.txt";
  std::ofstream(outside) << "keep";
#ifndef _WIN32
  fs::create_symlink(outside, path);
  std::string contents;
  assert(!msime::windows::read_audio_mute_state(path, contents));
#else
  std::ofstream(path) << "old";
#endif
  assert(msime::windows::write_audio_mute_state(path, "0\tsynthetic-endpoint\n"));
  {
    std::ifstream input(outside);
    assert(std::string(std::istreambuf_iterator<char>(input),
                       std::istreambuf_iterator<char>()) == "keep");
  }
  assert(msime::windows::read_audio_mute_state(path, contents));
  assert(contents == "0\tsynthetic-endpoint\n");
  assert(!msime::windows::write_audio_mute_state(
      path, std::string(msime::windows::kAudioMuteStateMaxBytes + 1, 'x')));
  fs::remove_all(root);
  return 0;
}
