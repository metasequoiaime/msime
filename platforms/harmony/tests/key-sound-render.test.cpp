// Checks native/key_sound_render.cpp on the build machine. The renderer is plain C++ over miniaudio with no NAPI in it, so what it writes, and what it refuses, can be read back here without a device.
#include "key_sound_render.h"

#include "key_sound_miniaudio.h"
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <string>
#include <vector>

namespace {

int failures = 0;

void check(bool condition, const char *label) {
    std::printf("%s %s\n", condition ? "ok  " : "FAIL", label);
    if (!condition) ++failures;
}

void put16(std::vector<uint8_t> &out, uint32_t value) {
    out.push_back(static_cast<uint8_t>(value & 0xff));
    out.push_back(static_cast<uint8_t>((value >> 8) & 0xff));
}

void put32(std::vector<uint8_t> &out, uint32_t value) {
    put16(out, value & 0xffff);
    put16(out, value >> 16);
}

// A 16-bit PCM WAV of `frames` frames of a 440 Hz tone, synthetic test data.
std::vector<uint8_t> wav(uint32_t rate, uint32_t channels, uint32_t frames) {
    const uint32_t data = frames * channels * 2;
    std::vector<uint8_t> out;
    out.insert(out.end(), {'R', 'I', 'F', 'F'});
    put32(out, 36 + data);
    out.insert(out.end(), {'W', 'A', 'V', 'E', 'f', 'm', 't', ' '});
    put32(out, 16);
    put16(out, 1);
    put16(out, channels);
    put32(out, rate);
    put32(out, rate * channels * 2);
    put16(out, channels * 2);
    put16(out, 16);
    out.insert(out.end(), {'d', 'a', 't', 'a'});
    put32(out, data);
    for (uint32_t frame = 0; frame < frames; ++frame) {
        const double value = std::sin(2.0 * 3.14159265358979 * 440.0 * frame / rate) * 12000.0;
        for (uint32_t channel = 0; channel < channels; ++channel) {
            put16(out, static_cast<uint32_t>(static_cast<int16_t>(value)) & 0xffff);
        }
    }
    return out;
}

void write(const std::string &path, const std::vector<uint8_t> &bytes) {
    std::ofstream output(path, std::ios::out | std::ios::binary | std::ios::trunc);
    output.write(reinterpret_cast<const char *>(bytes.data()), static_cast<std::streamsize>(bytes.size()));
}

// Frames, channels and rate of a written note, read back through miniaudio.
bool describe(const std::string &path, ma_uint64 &frames, ma_uint32 &channels, ma_uint32 &rate) {
    ma_decoder decoder;
    if (ma_decoder_init_file(path.c_str(), nullptr, &decoder) != MA_SUCCESS) return false;
    const bool ok = ma_decoder_get_data_format(&decoder, nullptr, &channels, &rate, nullptr, 0) == MA_SUCCESS
        && ma_decoder_get_length_in_pcm_frames(&decoder, &frames) == MA_SUCCESS;
    ma_decoder_uninit(&decoder);
    return ok;
}

bool near(ma_uint64 actual, double expected) {
    return std::fabs(static_cast<double>(actual) - expected) <= expected * 0.01 + 80.0;
}

}  // namespace

int main(int argc, char **argv) {
    if (argc != 2 || argv[1][0] != '/') {
        std::fprintf(stderr, "usage: key-sound-render <absolute scratch directory>\n");
        return 2;
    }
    const std::string root = argv[1];
    const std::string sample = root + "/sample.wav";
    // 0.5 s of mono 44.1 kHz, the shape of the built-in packs.
    write(sample, wav(44100, 1, 22050));

    const KeySoundRender notes = renderKeySoundNotes(sample, {0, 12, -12, 7}, root, 1500);
    check(notes.ok && notes.files.size() == 4, "one note per semitone");
    if (notes.ok && notes.files.size() == 4) {
        ma_uint64 frames = 0;
        ma_uint32 channels = 0;
        ma_uint32 rate = 0;
        check(describe(notes.files[0], frames, channels, rate) && rate == kKeySoundOutputRate
            && channels == 1 && near(frames, 24000.0), "the unpitched note keeps its length at 48 kHz");
        check(describe(notes.files[1], frames, channels, rate) && near(frames, 12000.0),
            "an octave up plays twice as fast, so half as long");
        check(describe(notes.files[2], frames, channels, rate) && near(frames, 48000.0),
            "an octave down is twice as long");
        check(describe(notes.files[3], frames, channels, rate)
            && near(frames, 24000.0 / std::pow(2.0, 7.0 / 12.0)), "a fifth up is 2^(7/12) shorter");
        check(notes.files[3] == root + "/note-3.wav", "notes are named by their index");
    }

    const std::string stereo = root + "/stereo.wav";
    write(stereo, wav(22050, 2, 2205));
    const KeySoundRender two = renderKeySoundNotes(stereo, {0}, root, 1500);
    ma_uint64 frames = 0;
    ma_uint32 channels = 0;
    ma_uint32 rate = 0;
    check(two.ok && describe(two.files[0], frames, channels, rate) && channels == 2 && near(frames, 4800.0),
        "a stereo sample stays stereo and is resampled to 48 kHz");

    const std::string exact = root + "/exact.wav";
    write(exact, wav(8000, 1, 12000));
    check(renderKeySoundNotes(exact, {0}, root, 1500).ok, "exactly 1.5 s at 8 kHz is allowed");
    const std::string longer = root + "/longer.wav";
    write(longer, wav(8000, 1, 12001));
    check(!renderKeySoundNotes(longer, {0}, root, 1500).ok, "one frame over the length bound is refused");

    const std::string slow = root + "/slow.wav";
    write(slow, wav(4000, 1, 400));
    check(!renderKeySoundNotes(slow, {0}, root, 1500).ok, "a rate below 8 kHz is refused");
    const std::string surround = root + "/surround.wav";
    write(surround, wav(44100, 3, 441));
    check(!renderKeySoundNotes(surround, {0}, root, 1500).ok, "three channels are refused");
    const std::string empty = root + "/empty.wav";
    write(empty, wav(44100, 1, 0));
    check(!renderKeySoundNotes(empty, {0}, root, 1500).ok, "an empty sample is refused");

    const std::string garbage = root + "/garbage.wav";
    write(garbage, std::vector<uint8_t>(4096, 0x5a));
    check(!renderKeySoundNotes(garbage, {0}, root, 1500).ok, "bytes that are not WAV are refused");
    const std::string ogg = root + "/tone.ogg";
    std::vector<uint8_t> ogg_bytes = {'O', 'g', 'g', 'S', 0, 2};
    ogg_bytes.resize(512, 0);
    write(ogg, ogg_bytes);
    check(!renderKeySoundNotes(ogg, {0}, root, 1500).ok, "Ogg is not decoded here");

    // A data chunk that claims less than the file holds: miniaudio stops at the declared end, so the note is the declared length and no more.
    std::vector<uint8_t> lying = wav(44100, 1, 22050);
    const uint32_t declared = 4410 * 2;
    lying[40] = static_cast<uint8_t>(declared & 0xff);
    lying[41] = static_cast<uint8_t>((declared >> 8) & 0xff);
    lying[42] = 0;
    lying[43] = 0;
    const std::string short_header = root + "/short-header.wav";
    write(short_header, lying);
    const KeySoundRender cut = renderKeySoundNotes(short_header, {0}, root, 1500);
    check(cut.ok && describe(cut.files[0], frames, channels, rate) && near(frames, 4800.0),
        "a sample is decoded only as far as its header declares");

    const std::string big = root + "/big.wav";
    write(big, std::vector<uint8_t>(kKeySoundMaxSampleBytes + 1, 0));
    check(!renderKeySoundNotes(big, {0}, root, 1500).ok, "a file over the pack's size bound is not read");

    // A validated pack file may be replaced before the native worker opens it;
    // the renderer must not follow that replacement outside the pack.
    const std::string outside_sample = root + "/outside-sample.wav";
    write(outside_sample, wav(44100, 1, 22050));
    const std::string linked_sample = root + "/linked-sample.wav";
    std::error_code input_symlink_error;
    std::filesystem::create_symlink(outside_sample, linked_sample, input_symlink_error);
    if (input_symlink_error) {
        check(false, "create input symlink");
    } else {
        check(!renderKeySoundNotes(linked_sample, {0}, root, 1500).ok,
            "input symlinks are refused before decoding");
    }

    check(!renderKeySoundNotes(sample, {25}, root, 1500).ok, "a semitone past two octaves is refused");
    check(!renderKeySoundNotes(sample, {}, root, 1500).ok, "no semitones is refused");
    check(!renderKeySoundNotes(sample, std::vector<int32_t>(kKeySoundMaxNotes + 1, 0), root, 1500).ok,
        "more notes than a melody may have are refused");
    check(!renderKeySoundNotes(sample, {0}, "relative", 1500).ok, "a relative directory is refused");
    check(!renderKeySoundNotes(root + "/missing.wav", {0}, root, 1500).ok, "a missing file is refused");

    // 输出文件是符号链接时，不能把生成的音频写到链接目标。
    const std::string outside = root + "/outside.wav";
    write(outside, {'k', 'e', 'e', 'p'});
    const std::string linked_note = root + "/note-0.wav";
    std::remove(linked_note.c_str());
    std::error_code symlink_error;
    std::filesystem::create_symlink(outside, linked_note, symlink_error);
    if (symlink_error) {
        check(false, "create output symlink");
    } else {
        const KeySoundRender linked = renderKeySoundNotes(sample, {0}, root, 1500);
        std::ifstream preserved(outside, std::ios::binary);
        std::string bytes((std::istreambuf_iterator<char>(preserved)), std::istreambuf_iterator<char>());
        check(linked.ok && bytes == "keep" && !std::filesystem::is_symlink(linked_note),
            "output replaces a symlink without writing through it");
    }

    std::printf("%d failure(s)\n", failures);
    return failures == 0 ? 0 : 1;
}
