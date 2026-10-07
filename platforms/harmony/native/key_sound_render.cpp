#include "key_sound_render.h"

#include "key_sound_miniaudio.h"
#include <cerrno>
#include <cmath>
#include <cstdio>
#include <filesystem>
#include <fcntl.h>
#include <system_error>
#include <sys/stat.h>
#include <unistd.h>

namespace {

KeySoundRender refused(const char *reason) {
    KeySoundRender result;
    result.error = reason;
    return result;
}

bool readBounded(const std::string &path, std::vector<uint8_t> &out) {
    const int descriptor = ::open(path.c_str(), O_RDONLY | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK);
    if (descriptor < 0) return false;
    struct Descriptor {
        int value;
        ~Descriptor() { ::close(value); }
    } input{descriptor};
    struct stat status {};
    if (::fstat(input.value, &status) != 0 || !S_ISREG(status.st_mode)) return false;

    // One byte past the bound, so a file that grew after validation is noticed rather than cut short.
    out.assign(kKeySoundMaxSampleBytes + 1, 0);
    size_t length = 0;
    while (length < out.size()) {
        const ssize_t count = ::read(input.value, out.data() + length, out.size() - length);
        if (count > 0) {
            length += static_cast<size_t>(count);
            continue;
        }
        if (count == 0) break;
        if (errno == EINTR) continue;
        return false;
    }
    if (length == 0 || length > kKeySoundMaxSampleBytes) return false;
    out.resize(length);
    return true;
}

ma_decoder_config wavConfig(ma_decoder_config config) {
    config.encodingFormat = ma_encoding_format_wav;
    return config;
}

bool writeNote(const std::string &path, const std::vector<int16_t> &pcm, ma_uint32 channels,
               ma_uint64 frames) {
    const ma_encoder_config config = ma_encoder_config_init(ma_encoding_format_wav, ma_format_s16,
        channels, kKeySoundOutputRate);
    std::string pattern = path + ".tmp-XXXXXX";
    std::vector<char> temporary_name(pattern.begin(), pattern.end());
    temporary_name.push_back('\0');
    const int descriptor = ::mkstemp(temporary_name.data());
    if (descriptor < 0) return false;
    const std::string temporary(temporary_name.data());
    if (::close(descriptor) != 0) {
        std::remove(temporary.c_str());
        return false;
    }
    ma_encoder encoder;
    if (ma_encoder_init_file(temporary.c_str(), &config, &encoder) != MA_SUCCESS) {
        std::remove(temporary.c_str());
        return false;
    }
    ma_uint64 written = 0;
    const ma_result status = ma_encoder_write_pcm_frames(&encoder, pcm.data(), frames, &written);
    ma_encoder_uninit(&encoder);
    if (status != MA_SUCCESS || written != frames) {
        std::remove(temporary.c_str());
        return false;
    }
    std::error_code error;
    std::filesystem::rename(temporary, path, error);
    if (!error) return true;
    std::remove(temporary.c_str());
    return false;
}

}  // namespace

KeySoundRender renderKeySoundNotes(const std::string &sample, const std::vector<int32_t> &semitones,
                                   const std::string &directory, uint32_t max_millis) {
    if (semitones.empty() || semitones.size() > kKeySoundMaxNotes) return refused("invalid note count");
    for (const int32_t semitone : semitones) {
        if (semitone < -kKeySoundSemitoneRange || semitone > kKeySoundSemitoneRange) {
            return refused("semitone out of range");
        }
    }
    if (directory.empty() || directory.front() != '/' || max_millis == 0) {
        return refused("invalid render target");
    }
    std::vector<uint8_t> bytes;
    if (!readBounded(sample, bytes)) return refused("sample unreadable or too large");

    // What the header declares, before anything is decoded.
    ma_decoder_config probe_config = wavConfig(ma_decoder_config_init_default());
    ma_decoder probe;
    if (ma_decoder_init_memory(bytes.data(), bytes.size(), &probe_config, &probe) != MA_SUCCESS) {
        return refused("not a WAV sample");
    }
    ma_uint32 channels = 0;
    ma_uint32 rate = 0;
    ma_uint64 frames = 0;
    const bool described =
        ma_decoder_get_data_format(&probe, nullptr, &channels, &rate, nullptr, 0) == MA_SUCCESS
        && ma_decoder_get_length_in_pcm_frames(&probe, &frames) == MA_SUCCESS;
    ma_decoder_uninit(&probe);
    if (!described || rate < kKeySoundMinSampleRate || rate > kKeySoundMaxSampleRate) {
        return refused("unsupported sample rate");
    }
    if (channels < 1 || channels > 2) return refused("only mono and stereo samples are played");
    // client-core's sample_frames_allowed, from the header's own count.
    if (frames == 0 || frames * 1000 > static_cast<ma_uint64>(max_millis) * rate) {
        return refused("sample longer than the pack bound");
    }

    KeySoundRender result;
    for (size_t index = 0; index < semitones.size(); ++index) {
        // Pitching up by a ratio is playing faster: the sample is converted to the output rate divided by that ratio and then labelled with the output rate, which is what a playback rate does.
        const double ratio = std::pow(2.0, static_cast<double>(semitones[index]) / 12.0);
        const ma_uint32 converted = static_cast<ma_uint32>(std::lround(kKeySoundOutputRate / ratio));
        ma_decoder_config config = wavConfig(ma_decoder_config_init(ma_format_s16, channels, converted));
        ma_decoder decoder;
        if (ma_decoder_init_memory(bytes.data(), bytes.size(), &config, &decoder) != MA_SUCCESS) {
            return refused("not a WAV sample");
        }
        // The declared length at the converted rate, with room for the converter's rounding. One frame more than this is a sample that lied about its length.
        const ma_uint64 limit = (frames * converted + rate - 1) / rate + 64;
        std::vector<int16_t> pcm(static_cast<size_t>((limit + 1) * channels));
        ma_uint64 read = 0;
        const ma_result status = ma_decoder_read_pcm_frames(&decoder, pcm.data(), limit + 1, &read);
        ma_decoder_uninit(&decoder);
        if ((status != MA_SUCCESS && status != MA_AT_END) || read == 0) {
            return refused("sample would not decode");
        }
        if (read > limit) return refused("sample decoded past its declared length");
        const std::string file = directory + "/note-" + std::to_string(index) + ".wav";
        if (!writeNote(file, pcm, channels, read)) {
            std::remove(file.c_str());
            return refused("note could not be written");
        }
        result.files.push_back(file);
    }
    result.ok = true;
    return result;
}
