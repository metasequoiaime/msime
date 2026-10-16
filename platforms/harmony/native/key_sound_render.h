#pragma once

#include <cstdint>
#include <string>
#include <vector>

// 包的各项上限取自 client-services 的 `sound_pack` 模块（包的路径到达本宿主之前已由它校验过）：`MAX_SAMPLE_BYTES`、`SAMPLE_RATES`、`MAX_SEMITONES` 和 `SEMITONE_RANGE`。渲染器自己读文件，所以会再检查一遍。
constexpr size_t kKeySoundMaxSampleBytes = 512 * 1024;
constexpr uint32_t kKeySoundMinSampleRate = 8000;
constexpr uint32_t kKeySoundMaxSampleRate = 192000;
constexpr size_t kKeySoundMaxNotes = 128;
constexpr int32_t kKeySoundSemitoneRange = 24;
// Every note is written at this rate, one SoundPool plays everywhere, whatever rate the pack's sample was recorded at.
constexpr uint32_t kKeySoundOutputRate = 48000;

struct KeySoundRender {
    bool ok = false;
    std::string error;
    // One file per requested semitone, in request order.
    std::vector<std::string> files;
};

// Decodes the WAV sample at `sample` once per entry of `semitones` and writes each as `<directory>/note-<index>.wav`, 16-bit PCM at kKeySoundOutputRate, pitched by that many semitones the way host-api's player pitches a melody: by playback rate, so a note two semitones up is also shorter. The sample is refused when its header declares more than `max_millis` of audio, and a decode that yields more frames than the header declared, or none, refuses it too, so a file that lies about its length costs at most one sample's worth of memory. Only WAV is decoded: the caller plays an Ogg sample through SoundPool as it is.
KeySoundRender renderKeySoundNotes(const std::string &sample, const std::vector<int32_t> &semitones,
                                   const std::string &directory, uint32_t max_millis);
