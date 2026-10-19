// APNG/GIF 合成与时钟回归，固定来源与本地调整见 msimeui/UPSTREAM.md。
#include <windows.h>
#include "../../msimeui/tests/includes/test_framework.h"

#include "msimeui/AnimatedImage.h"
#include "CandidateImageFrames.h"

#include <array>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <optional>
#include <string>
#include <vector>
#include <wincodec.h>
#include <wrl/client.h>

using Microsoft::WRL::ComPtr;

namespace
{
struct Rgba
{
    uint8_t r = 0;
    uint8_t g = 0;
    uint8_t b = 0;
    uint8_t a = 0;
};

constexpr Rgba kRed = {255, 0, 0, 255};
constexpr Rgba kGreen = {0, 255, 0, 255};
constexpr Rgba kBlue = {0, 0, 255, 255};
constexpr Rgba kClear = {0, 0, 0, 0};

class ComApartment
{
  public:
    ComApartment() : hr_(CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED))
    {
        REQUIRE(SUCCEEDED(hr_) || hr_ == RPC_E_CHANGED_MODE);
    }

    ~ComApartment()
    {
        if (SUCCEEDED(hr_))
        {
            CoUninitialize();
        }
    }

  private:
    HRESULT hr_;
};

ComPtr<IWICImagingFactory> CreateFactory()
{
    ComPtr<IWICImagingFactory> factory;
    REQUIRE(SUCCEEDED(CoCreateInstance(CLSID_WICImagingFactory, nullptr, CLSCTX_INPROC_SERVER,
                                       IID_PPV_ARGS(factory.GetAddressOf()))));
    return factory;
}

void PutBE32(std::vector<uint8_t> &out, uint32_t value)
{
    for (int shift = 24; shift >= 0; shift -= 8)
    {
        out.push_back(static_cast<uint8_t>(value >> shift));
    }
}

void PutBE16(std::vector<uint8_t> &out, uint16_t value)
{
    out.push_back(static_cast<uint8_t>(value >> 8));
    out.push_back(static_cast<uint8_t>(value));
}

void PutLE16(std::vector<uint8_t> &out, uint16_t value)
{
    out.push_back(static_cast<uint8_t>(value));
    out.push_back(static_cast<uint8_t>(value >> 8));
}

uint32_t Crc32(const uint8_t *data, size_t size)
{
    uint32_t crc = 0xFFFFFFFFU;
    for (size_t i = 0; i < size; ++i)
    {
        crc ^= data[i];
        for (int k = 0; k < 8; ++k)
        {
            crc = (crc & 1U) ? 0xEDB88320U ^ (crc >> 1) : crc >> 1;
        }
    }
    return crc ^ 0xFFFFFFFFU;
}

void PutChunk(std::vector<uint8_t> &out, const char *type, const std::vector<uint8_t> &data)
{
    PutBE32(out, static_cast<uint32_t>(data.size()));
    std::vector<uint8_t> body(type, type + 4);
    body.insert(body.end(), data.begin(), data.end());
    out.insert(out.end(), body.begin(), body.end());
    PutBE32(out, Crc32(body.data(), body.size()));
}

std::vector<uint8_t> ZlibStored(const std::vector<uint8_t> &raw)
{
    std::vector<uint8_t> out = {0x78, 0x01};
    size_t pos = 0;
    do
    {
        const size_t n = (std::min)(raw.size() - pos, size_t{65535});
        const bool last = pos + n == raw.size();
        out.push_back(last ? 1 : 0);
        PutLE16(out, static_cast<uint16_t>(n));
        PutLE16(out, static_cast<uint16_t>(~n));
        out.insert(out.end(), raw.begin() + static_cast<std::ptrdiff_t>(pos),
                   raw.begin() + static_cast<std::ptrdiff_t>(pos + n));
        pos += n;
    } while (pos < raw.size());
    uint32_t a = 1;
    uint32_t b = 0;
    for (const uint8_t byte : raw)
    {
        a = (a + byte) % 65521U;
        b = (b + a) % 65521U;
    }
    PutBE32(out, (b << 16) | a);
    return out;
}

std::vector<uint8_t> RgbaScanlines(uint32_t width, uint32_t height, const std::vector<Rgba> &pixels)
{
    std::vector<uint8_t> raw;
    for (uint32_t y = 0; y < height; ++y)
    {
        raw.push_back(0);
        for (uint32_t x = 0; x < width; ++x)
        {
            const Rgba &p = pixels[y * width + x];
            raw.insert(raw.end(), {p.r, p.g, p.b, p.a});
        }
    }
    return ZlibStored(raw);
}

std::vector<Rgba> Fill(uint32_t width, uint32_t height, Rgba color)
{
    return std::vector<Rgba>(static_cast<size_t>(width) * height, color);
}

struct ApngFrame
{
    uint32_t width = 0;
    uint32_t height = 0;
    uint32_t x = 0;
    uint32_t y = 0;
    uint16_t delayNum = 1;
    uint16_t delayDen = 10;
    uint8_t dispose = 0;
    uint8_t blend = 0;
    std::vector<Rgba> pixels;
};

struct ApngOptions
{
    uint32_t plays = 0;

    std::optional<std::vector<Rgba>> separateDefaultImage;

    std::optional<size_t> dropDataOf;

    std::optional<uint32_t> declaredFrames;
};

std::vector<uint8_t> Ihdr(uint32_t width, uint32_t height)
{
    std::vector<uint8_t> ihdr;
    PutBE32(ihdr, width);
    PutBE32(ihdr, height);
    ihdr.insert(ihdr.end(), {8, 6, 0, 0, 0});
    return ihdr;
}

std::vector<uint8_t> Fctl(uint32_t sequence, const ApngFrame &frame)
{
    std::vector<uint8_t> data;
    PutBE32(data, sequence);
    PutBE32(data, frame.width);
    PutBE32(data, frame.height);
    PutBE32(data, frame.x);
    PutBE32(data, frame.y);
    PutBE16(data, frame.delayNum);
    PutBE16(data, frame.delayDen);
    data.push_back(frame.dispose);
    data.push_back(frame.blend);
    return data;
}

std::vector<uint8_t> BuildApng(uint32_t width, uint32_t height, const std::vector<ApngFrame> &frames,
                               const ApngOptions &options = {})
{
    std::vector<uint8_t> png = {0x89, 'P', 'N', 'G', 0x0D, 0x0A, 0x1A, 0x0A};
    PutChunk(png, "IHDR", Ihdr(width, height));
    std::vector<uint8_t> actl;
    PutBE32(actl, options.declaredFrames.value_or(static_cast<uint32_t>(frames.size())));
    PutBE32(actl, options.plays);
    PutChunk(png, "acTL", actl);
    uint32_t sequence = 0;
    size_t first = 0;
    if (options.separateDefaultImage)
    {
        PutChunk(png, "IDAT", RgbaScanlines(width, height, *options.separateDefaultImage));
    }
    else
    {
        PutChunk(png, "fcTL", Fctl(sequence++, frames[0]));
        PutChunk(png, "IDAT", RgbaScanlines(frames[0].width, frames[0].height, frames[0].pixels));
        first = 1;
    }
    for (size_t i = first; i < frames.size(); ++i)
    {
        PutChunk(png, "fcTL", Fctl(sequence++, frames[i]));
        if (options.dropDataOf == i)
        {
            continue;
        }
        std::vector<uint8_t> fdat;
        PutBE32(fdat, sequence++);
        const std::vector<uint8_t> data = RgbaScanlines(frames[i].width, frames[i].height, frames[i].pixels);
        fdat.insert(fdat.end(), data.begin(), data.end());
        PutChunk(png, "fdAT", fdat);
    }
    PutChunk(png, "IEND", {});
    return png;
}

size_t FindChunk(const std::vector<uint8_t> &png, const char *type, int nth = 0)
{
    size_t pos = 8;
    while (pos + 12 <= png.size())
    {
        const uint32_t length = (static_cast<uint32_t>(png[pos]) << 24) | (static_cast<uint32_t>(png[pos + 1]) << 16) |
                                (static_cast<uint32_t>(png[pos + 2]) << 8) | png[pos + 3];
        if (std::equal(type, type + 4, png.begin() + static_cast<std::ptrdiff_t>(pos + 4)) && nth-- == 0)
        {
            return pos;
        }
        pos += 12 + length;
    }
    throw std::runtime_error("chunk not found");
}

struct GifFrame
{
    uint16_t left = 0;
    uint16_t top = 0;
    uint16_t width = 0;
    uint16_t height = 0;
    uint16_t delayCs = 0;
    uint8_t disposal = 0;
    std::vector<uint8_t> indices;
};

std::vector<uint8_t> LzwUncompressed(const std::vector<uint8_t> &indices)
{
    std::vector<uint8_t> out;
    uint32_t buffer = 0;
    int bits = 0;
    const auto emit = [&](uint32_t code) {
        buffer |= code << bits;
        bits += 3;
        while (bits >= 8)
        {
            out.push_back(static_cast<uint8_t>(buffer));
            buffer >>= 8;
            bits -= 8;
        }
    };
    constexpr uint32_t kClearCode = 4;
    constexpr uint32_t kEndCode = 5;
    for (size_t i = 0; i < indices.size(); ++i)
    {
        if (i % 2 == 0)
        {
            emit(kClearCode);
        }
        emit(indices[i]);
    }
    emit(kEndCode);
    if (bits > 0)
    {
        out.push_back(static_cast<uint8_t>(buffer));
    }
    return out;
}

std::vector<uint8_t> BuildGif(uint16_t width, uint16_t height, const std::vector<GifFrame> &frames,
                              std::optional<uint16_t> loop)
{
    std::vector<uint8_t> gif = {'G', 'I', 'F', '8', '9', 'a'};
    PutLE16(gif, width);
    PutLE16(gif, height);
    gif.insert(gif.end(), {0x81, 0x00, 0x00});
    gif.insert(gif.end(), {255, 0, 0, 0, 0, 255, 0, 255, 0, 0, 0, 0});
    if (loop)
    {
        gif.insert(gif.end(), {0x21, 0xFF, 0x0B});
        const char *app = "NETSCAPE2.0";
        gif.insert(gif.end(), app, app + 11);
        gif.insert(gif.end(), {0x03, 0x01});
        PutLE16(gif, *loop);
        gif.push_back(0x00);
    }
    for (const GifFrame &frame : frames)
    {
        gif.insert(gif.end(), {0x21, 0xF9, 0x04, static_cast<uint8_t>((frame.disposal << 2) | 1)});
        PutLE16(gif, frame.delayCs);
        gif.insert(gif.end(), {0x03, 0x00});
        gif.push_back(0x2C);
        PutLE16(gif, frame.left);
        PutLE16(gif, frame.top);
        PutLE16(gif, frame.width);
        PutLE16(gif, frame.height);
        gif.push_back(0x00);
        gif.push_back(0x02);
        const std::vector<uint8_t> data = LzwUncompressed(frame.indices);
        for (size_t pos = 0; pos < data.size(); pos += 255)
        {
            const size_t n = (std::min)(data.size() - pos, size_t{255});
            gif.push_back(static_cast<uint8_t>(n));
            gif.insert(gif.end(), data.begin() + static_cast<std::ptrdiff_t>(pos),
                       data.begin() + static_cast<std::ptrdiff_t>(pos + n));
        }
        gif.push_back(0x00);
    }
    gif.push_back(0x3B);
    return gif;
}

Rgba PixelAt(const msimeui::AnimatedImage &image, size_t frame, uint32_t x, uint32_t y)
{
    const auto &pixels = image.FramePixels(frame);
    const size_t offset = (static_cast<size_t>(y) * image.Width() + x) * 4;
    return {pixels[offset + 2], pixels[offset + 1], pixels[offset], pixels[offset + 3]};
}

bool Near(const Rgba &actual, const Rgba &expected, int tolerance = 1)
{
    const auto close = [tolerance](uint8_t a, uint8_t b) { return std::abs(static_cast<int>(a) - b) <= tolerance; };
    const bool ok = close(actual.r, expected.r) && close(actual.g, expected.g) && close(actual.b, expected.b) &&
                    close(actual.a, expected.a);
    if (!ok)
    {
        std::printf("pixel %d,%d,%d,%d expected %d,%d,%d,%d\n", actual.r, actual.g, actual.b, actual.a, expected.r,
                    expected.g, expected.b, expected.a);
    }
    return ok;
}

std::optional<msimeui::AnimatedImage> Decode(IWICImagingFactory *factory, const std::vector<uint8_t> &bytes,
                                             const msimeui::AnimatedImage::Limits &limits = {})
{
    return msimeui::AnimatedImage::DecodeMemory(factory, bytes.data(), bytes.size(), limits);
}

ApngFrame Frame(uint32_t width, uint32_t height, Rgba color, uint32_t x = 0, uint32_t y = 0)
{
    ApngFrame frame;
    frame.width = width;
    frame.height = height;
    frame.x = x;
    frame.y = y;
    frame.pixels = Fill(width, height, color);
    return frame;
}

class TempFile
{
  public:
    explicit TempFile(const std::vector<uint8_t> &bytes, const wchar_t *extension)
    {
        wchar_t directory[MAX_PATH] = {};
        REQUIRE(GetTempPathW(MAX_PATH, directory) != 0);
        static int counter = 0;
        path = std::wstring(directory) + L"msimeui-anim-" + std::to_wstring(GetCurrentProcessId()) + L"-" +
               std::to_wstring(++counter) + extension;
        HANDLE file =
            CreateFileW(path.c_str(), GENERIC_WRITE, 0, nullptr, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
        REQUIRE(file != INVALID_HANDLE_VALUE);
        DWORD written = 0;
        const BOOL ok = WriteFile(file, bytes.data(), static_cast<DWORD>(bytes.size()), &written, nullptr);
        CloseHandle(file);
        REQUIRE(ok && written == bytes.size());
    }

    ~TempFile()
    {
        DeleteFileW(path.c_str());
    }

    std::wstring path;
};

}

TEST_CASE(apng_decodes_every_frame_with_browser_delay_rules)
{
    ComApartment com;
    auto factory = CreateFactory();
    std::vector<ApngFrame> frames = {Frame(4, 4, kRed), Frame(4, 4, kGreen), Frame(4, 4, kBlue), Frame(4, 4, kRed)};
    frames[0].delayNum = 1;
    frames[0].delayDen = 20;
    frames[1].delayNum = 1;
    frames[1].delayDen = 100;
    frames[2].delayNum = 30;
    frames[2].delayDen = 0;
    frames[3].delayNum = 0;
    frames[3].delayDen = 1;
    const auto image = Decode(factory.Get(), BuildApng(4, 4, frames));
    REQUIRE(image.has_value());
    REQUIRE(image->GetFormat() == msimeui::AnimatedImage::Format::Apng);
    REQUIRE(image->FrameCount() == 4);
    REQUIRE(image->Width() == 4 && image->Height() == 4);
    REQUIRE(image->Timeline().frameDelaysMs == std::vector<uint32_t>({50, 100, 300, 100}));
    REQUIRE(image->Timeline().playCount == 0);
    REQUIRE(Near(PixelAt(*image, 0, 1, 1), kRed));
    REQUIRE(Near(PixelAt(*image, 1, 1, 1), kGreen));
    REQUIRE(Near(PixelAt(*image, 2, 1, 1), kBlue));
}

TEST_CASE(apng_places_sub_rect_frames_and_disposes_to_background)
{
    ComApartment com;
    auto factory = CreateFactory();
    std::vector<ApngFrame> frames = {Frame(6, 6, kRed), Frame(2, 2, kBlue, 3, 1), Frame(1, 1, kGreen, 0, 0)};
    frames[1].dispose = 1;
    const auto image = Decode(factory.Get(), BuildApng(6, 6, frames));
    REQUIRE(image.has_value() && image->FrameCount() == 3);

    REQUIRE(Near(PixelAt(*image, 1, 3, 1), kBlue));
    REQUIRE(Near(PixelAt(*image, 1, 4, 2), kBlue));
    REQUIRE(Near(PixelAt(*image, 1, 2, 1), kRed));
    REQUIRE(Near(PixelAt(*image, 1, 5, 3), kRed));

    REQUIRE(Near(PixelAt(*image, 2, 3, 1), kClear));
    REQUIRE(Near(PixelAt(*image, 2, 4, 2), kClear));
    REQUIRE(Near(PixelAt(*image, 2, 2, 2), kRed));
    REQUIRE(Near(PixelAt(*image, 2, 0, 0), kGreen));
}

TEST_CASE(apng_dispose_previous_restores_the_canvas_and_means_background_on_the_first_frame)
{
    ComApartment com;
    auto factory = CreateFactory();
    std::vector<ApngFrame> frames = {Frame(4, 4, kRed), Frame(2, 2, kBlue, 1, 1), Frame(1, 1, kGreen, 3, 3)};
    frames[1].dispose = 2;
    auto image = Decode(factory.Get(), BuildApng(4, 4, frames));
    REQUIRE(image.has_value() && image->FrameCount() == 3);
    REQUIRE(Near(PixelAt(*image, 1, 1, 1), kBlue));
    REQUIRE(Near(PixelAt(*image, 2, 1, 1), kRed));
    REQUIRE(Near(PixelAt(*image, 2, 2, 2), kRed));
    REQUIRE(Near(PixelAt(*image, 2, 3, 3), kGreen));

    std::vector<ApngFrame> firstPrevious = {Frame(4, 4, kRed), Frame(1, 1, kGreen, 0, 0)};
    firstPrevious[0].dispose = 2;
    image = Decode(factory.Get(), BuildApng(4, 4, firstPrevious));
    REQUIRE(image.has_value() && image->FrameCount() == 2);
    REQUIRE(Near(PixelAt(*image, 0, 2, 2), kRed));
    REQUIRE(Near(PixelAt(*image, 1, 2, 2), kClear));
    REQUIRE(Near(PixelAt(*image, 1, 0, 0), kGreen));
}

TEST_CASE(apng_blend_over_composites_and_blend_source_replaces)
{
    ComApartment com;
    auto factory = CreateFactory();
    const Rgba halfBlue = {0, 0, 255, 128};
    for (const uint8_t blend : {uint8_t{0}, uint8_t{1}})
    {
        std::vector<ApngFrame> frames = {Frame(2, 2, kRed), Frame(2, 2, halfBlue)};
        frames[1].blend = blend;
        const auto image = Decode(factory.Get(), BuildApng(2, 2, frames));
        REQUIRE(image.has_value() && image->FrameCount() == 2);
        const Rgba pixel = PixelAt(*image, 1, 0, 0);
        if (blend == 0)
        {

            REQUIRE(Near(pixel, {0, 0, 128, 128}));
        }
        else
        {

            REQUIRE(Near(pixel, {127, 0, 128, 255}));
        }
    }
}

TEST_CASE(apng_default_image_outside_the_animation_is_skipped)
{
    ComApartment com;
    auto factory = CreateFactory();
    ApngOptions options;
    options.separateDefaultImage = Fill(3, 3, kGreen);
    const auto image = Decode(factory.Get(), BuildApng(3, 3, {Frame(3, 3, kRed), Frame(3, 3, kBlue)}, options));
    REQUIRE(image.has_value());
    REQUIRE(image->FrameCount() == 2);
    REQUIRE(Near(PixelAt(*image, 0, 1, 1), kRed));
    REQUIRE(Near(PixelAt(*image, 1, 1, 1), kBlue));
}

TEST_CASE(apng_malformed_streams_fall_back_to_the_still_image)
{
    ComApartment com;
    auto factory = CreateFactory();
    const std::vector<ApngFrame> frames = {Frame(4, 4, kRed), Frame(2, 2, kBlue, 1, 1)};
    const auto expectStill = [&](const std::vector<uint8_t> &bytes, const char *what) {
        const auto image = Decode(factory.Get(), bytes);
        std::printf("%s: %s, %zu frame(s)\n", what, image ? "decoded" : "rejected", image ? image->FrameCount() : 0);

        REQUIRE(!image || (image->FrameCount() == 1 && !image->IsAnimated()));
        return image;
    };

    std::vector<uint8_t> badCrc = BuildApng(4, 4, frames);
    const size_t fctl = FindChunk(badCrc, "fcTL", 1);
    badCrc[fctl + 8 + 26] ^= 0xFF;
    const auto still = expectStill(badCrc, "bad crc");
    REQUIRE(still.has_value());
    REQUIRE(Near(PixelAt(*still, 0, 0, 0), kRed));

    std::vector<ApngFrame> outside = frames;
    outside[1].x = 3;
    expectStill(BuildApng(4, 4, outside), "out of bounds");

    ApngOptions missing;
    missing.dropDataOf = 1;
    expectStill(BuildApng(4, 4, frames, missing), "missing fdAT");

    ApngOptions miscounted;
    miscounted.declaredFrames = 3;
    expectStill(BuildApng(4, 4, frames, miscounted), "frame count");

    std::vector<uint8_t> reordered = BuildApng(4, 4, frames);
    const size_t secondFctl = FindChunk(reordered, "fcTL", 1);
    reordered[secondFctl + 8 + 3] = 7;
    const uint32_t crc = Crc32(reordered.data() + secondFctl + 4, 4 + 26);
    for (int b = 0; b < 4; ++b)
    {
        reordered[secondFctl + 8 + 26 + b] = static_cast<uint8_t>(crc >> (24 - 8 * b));
    }
    expectStill(reordered, "sequence");

    std::vector<uint8_t> truncated = BuildApng(4, 4, frames);
    truncated.resize(truncated.size() - 20);
    expectStill(truncated, "truncated");

    const std::vector<uint8_t> garbage(64, 0x5A);
    REQUIRE(!Decode(factory.Get(), garbage).has_value());
}

TEST_CASE(apng_over_the_limits_falls_back_to_the_still_image)
{
    ComApartment com;
    auto factory = CreateFactory();
    msimeui::AnimatedImage::Limits limits;
    limits.maxFrames = 2;
    const auto many =
        Decode(factory.Get(), BuildApng(2, 2, {Frame(2, 2, kRed), Frame(2, 2, kGreen), Frame(2, 2, kBlue)}), limits);
    REQUIRE(many.has_value() && many->FrameCount() == 1);
    limits = {};
    limits.maxDecodedBytes = 4 * 4 * 4;
    const auto big = Decode(factory.Get(), BuildApng(4, 4, {Frame(4, 4, kRed), Frame(4, 4, kGreen)}), limits);
    REQUIRE(big.has_value() && big->FrameCount() == 1);
}

TEST_CASE(animation_timeline_loops_and_holds_the_last_frame_after_the_last_play)
{
    msimeui::AnimationTimeline timeline;
    timeline.frameDelaysMs = {100, 200, 300};
    timeline.playCount = 2;
    auto sample = timeline.SampleAt(0);
    REQUIRE(sample.frameIndex == 0 && sample.nextChangeMs == 100U);
    sample = timeline.SampleAt(150);
    REQUIRE(sample.frameIndex == 1 && sample.nextChangeMs == 300U);
    sample = timeline.SampleAt(599);
    REQUIRE(sample.frameIndex == 2 && sample.nextChangeMs == 600U);
    sample = timeline.SampleAt(600);
    REQUIRE(sample.frameIndex == 0 && sample.nextChangeMs == 700U);

    sample = timeline.SampleAt(1000);
    REQUIRE(sample.frameIndex == 2 && !sample.nextChangeMs.has_value());
    sample = timeline.SampleAt(50000);
    REQUIRE(sample.frameIndex == 2 && !sample.nextChangeMs.has_value());

    timeline.playCount = 0;
    sample = timeline.SampleAt(50000 * 600 + 250);
    REQUIRE(sample.frameIndex == 1 && sample.nextChangeMs == 50000U * 600 + 300);

    msimeui::AnimationTimeline still;
    REQUIRE(!still.SampleAt(1234).nextChangeMs.has_value());
    REQUIRE(msimeui::NormalizeFrameDelayMs(0) == 100 && msimeui::NormalizeFrameDelayMs(10) == 100 &&
            msimeui::NormalizeFrameDelayMs(11) == 11);
}

TEST_CASE(apng_play_count_is_read_from_actl)
{
    ComApartment com;
    auto factory = CreateFactory();
    ApngOptions options;
    options.plays = 3;
    const auto image = Decode(factory.Get(), BuildApng(2, 2, {Frame(2, 2, kRed), Frame(2, 2, kBlue)}, options));
    REQUIRE(image.has_value() && image->Timeline().playCount == 3);
}

TEST_CASE(gif_frames_composite_with_disposal_transparency_and_loop_count)
{
    ComApartment com;
    auto factory = CreateFactory();
    std::vector<GifFrame> frames(4);
    frames[0] = {0, 0, 4, 4, 5, 1, std::vector<uint8_t>(16, 0)};
    frames[1] = {2, 2, 2, 2, 0, 2, std::vector<uint8_t>(4, 1)};
    frames[2] = {0, 0, 2, 1, 20, 3, {2, 3}};
    frames[3] = {1, 0, 1, 1, 1, 0, {2}};
    const auto image = Decode(factory.Get(), BuildGif(4, 4, frames, uint16_t{2}));
    REQUIRE(image.has_value());
    REQUIRE(image->GetFormat() == msimeui::AnimatedImage::Format::Gif);
    REQUIRE(image->FrameCount() == 4);
    REQUIRE(image->Width() == 4 && image->Height() == 4);
    REQUIRE(image->Timeline().frameDelaysMs == std::vector<uint32_t>({50, 100, 200, 100}));

    REQUIRE(image->Timeline().playCount == 3);
    REQUIRE(Near(PixelAt(*image, 1, 3, 3), kBlue));
    REQUIRE(Near(PixelAt(*image, 1, 1, 1), kRed));

    REQUIRE(Near(PixelAt(*image, 2, 3, 3), kClear));
    REQUIRE(Near(PixelAt(*image, 2, 0, 0), kGreen));

    REQUIRE(Near(PixelAt(*image, 2, 1, 0), kRed));

    REQUIRE(Near(PixelAt(*image, 3, 0, 0), kRed));
    REQUIRE(Near(PixelAt(*image, 3, 1, 0), kGreen));

    const auto once = Decode(factory.Get(), BuildGif(4, 4, frames, std::nullopt));
    REQUIRE(once.has_value() && once->Timeline().playCount == 1);
    const auto forever = Decode(factory.Get(), BuildGif(4, 4, frames, uint16_t{0}));
    REQUIRE(forever.has_value() && forever->Timeline().playCount == 0);
}

TEST_CASE(single_frame_images_decode_as_still)
{
    ComApartment com;
    auto factory = CreateFactory();
    std::vector<uint8_t> png = {0x89, 'P', 'N', 'G', 0x0D, 0x0A, 0x1A, 0x0A};
    PutChunk(png, "IHDR", Ihdr(3, 2));
    PutChunk(png, "IDAT", RgbaScanlines(3, 2, Fill(3, 2, kBlue)));
    PutChunk(png, "IEND", {});
    const auto still = Decode(factory.Get(), png);
    REQUIRE(still.has_value());
    REQUIRE(still->GetFormat() == msimeui::AnimatedImage::Format::Still);
    REQUIRE(!still->IsAnimated() && still->Width() == 3 && still->Height() == 2);
    REQUIRE(Near(PixelAt(*still, 0, 2, 1), kBlue));

    const TempFile stillFile(png, L".png");
    REQUIRE(!msimeui::AnimatedImage::MayBeAnimated(factory.Get(), stillFile.path));
    const TempFile apngFile(BuildApng(2, 2, {Frame(2, 2, kRed), Frame(2, 2, kBlue)}), L".apng");
    REQUIRE(msimeui::AnimatedImage::MayBeAnimated(factory.Get(), apngFile.path));
    const auto gifFrames = std::vector<GifFrame>{{0, 0, 1, 1, 5, 0, {0}}, {0, 0, 1, 1, 5, 0, {1}}};
    const TempFile gifFile(BuildGif(1, 1, gifFrames, std::nullopt), L".gif");
    REQUIRE(msimeui::AnimatedImage::MayBeAnimated(factory.Get(), gifFile.path));
}


namespace {
class PixelTarget {
public:
    explicit PixelTarget(IWICImagingFactory *factory) {
        REQUIRE(SUCCEEDED(factory->CreateBitmap(8, 8, GUID_WICPixelFormat32bppPBGRA,
                                                WICBitmapCacheOnLoad, pixels.GetAddressOf())));
        ComPtr<ID2D1Factory> d2d;
        REQUIRE(SUCCEEDED(D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED,
                                            d2d.GetAddressOf())));
        const auto properties = D2D1::RenderTargetProperties();
        REQUIRE(SUCCEEDED(d2d->CreateWicBitmapRenderTarget(pixels.Get(), properties,
                                                            target.GetAddressOf())));
    }

    Rgba draw(ID2D1Bitmap *bitmap) {
        REQUIRE(bitmap != nullptr);
        target->BeginDraw();
        target->Clear(D2D1::ColorF(0, 0.0f));
        target->DrawBitmap(bitmap, D2D1::RectF(0, 0, 8, 8), 1,
                            D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR);
        REQUIRE(SUCCEEDED(target->EndDraw()));
        std::array<uint8_t, 8 * 8 * 4> bytes{};
        REQUIRE(SUCCEEDED(pixels->CopyPixels(nullptr, 8 * 4, static_cast<UINT>(bytes.size()), bytes.data())));
        return {bytes[2], bytes[1], bytes[0], bytes[3]};
    }

    ComPtr<ID2D1RenderTarget> target;
    ComPtr<IWICBitmap> pixels;
};
}

TEST_CASE(candidate_image_cache_plays_updates_and_retains_finite_last_frame) {
    ComApartment com;
    auto factory = CreateFactory();
    PixelTarget output(factory.Get());
    auto frames = std::vector<ApngFrame>{Frame(4, 4, kRed), Frame(4, 4, kBlue)};
    for (auto &frame : frames) { frame.delayNum = 1; frame.delayDen = 10; }
    ApngOptions options;
    options.plays = 1;
    const TempFile file(BuildApng(4, 4, frames, options), L".apng");
    msime::windows::CandidateImageFrames cache;
    cache.begin_frame(output.target.Get(), 0, true);
    auto *first = cache.frame(file.path);
    REQUIRE(Near(output.draw(first), kRed));
    REQUIRE(cache.next_delay() == 100U);
    REQUIRE(cache.decoded_bytes() == 4U * 4 * 4 * 2);
    // 缓存之后删除文件：按键重绘与后续帧必须复用快照，不能重新读盘。
    REQUIRE(DeleteFileW(file.path.c_str()) != 0);
    cache.begin_frame(output.target.Get(), 100, true);
    auto *last = cache.frame(file.path);
    REQUIRE(last == first);
    REQUIRE(Near(output.draw(last), kBlue));
    REQUIRE(!cache.next_delay());
    cache.begin_frame(output.target.Get(), 10000, true);
    REQUIRE(Near(output.draw(cache.frame(file.path)), kBlue));
    REQUIRE(!cache.next_delay());
    cache.begin_frame(output.target.Get(), 0, true);
    REQUIRE(Near(output.draw(cache.frame(file.path)), kRed));
    REQUIRE(cache.next_delay() == 100U);
    cache.stop();
    REQUIRE(!cache.next_delay() && cache.frame(file.path) == nullptr);
    cache.begin_frame(output.target.Get(), 0, false);
    REQUIRE(cache.frame(file.path) == nullptr && !cache.next_delay());
    // 目标更换只重建当前位图，已删除的资源仍可由 CPU 快照恢复。
    PixelTarget second(factory.Get());
    cache.begin_frame(second.target.Get(), 100, true);
    REQUIRE(Near(second.draw(cache.frame(file.path)), kBlue));
    cache.clear();
    REQUIRE(cache.decoded_bytes() == 0 && !cache.next_delay());
    REQUIRE(cache.frame(file.path) == nullptr);
}

TEST_CASE(candidate_image_cache_schedules_only_images_drawn_this_frame) {
    ComApartment com;
    auto factory = CreateFactory();
    PixelTarget output(factory.Get());
    const TempFile file(BuildApng(2, 2, {Frame(2, 2, kRed), Frame(2, 2, kBlue)}), L".apng");
    msime::windows::CandidateImageFrames cache;
    cache.begin_frame(output.target.Get(), 0, true);
    REQUIRE(cache.frame(file.path) != nullptr && cache.next_delay());
    cache.begin_frame(output.target.Get(), 25, true);
    REQUIRE(!cache.next_delay());
    cache.reset_target();
    REQUIRE(!cache.next_delay());
}

TEST_CASE(candidate_image_cache_falls_back_without_continuous_failed_decode) {
    ComApartment com;
    auto factory = CreateFactory();
    PixelTarget output(factory.Get());
    auto broken = BuildApng(2, 2, {Frame(2, 2, kRed), Frame(2, 2, kBlue)});
    broken[broken.size() / 2] ^= 255;
    const TempFile file(broken, L".apng");
    msime::windows::CandidateImageFrames cache;
    cache.begin_frame(output.target.Get(), 0, true);
    REQUIRE(cache.frame(file.path) == nullptr && !cache.next_delay());
    REQUIRE(DeleteFileW(file.path.c_str()) != 0);
    cache.begin_frame(output.target.Get(), 100, true);
    REQUIRE(cache.frame(file.path) == nullptr && cache.decoded_bytes() == 0);
}

TEST_CASE(candidate_image_cache_enforces_a_shared_decoded_pixel_budget) {
    ComApartment com;
    auto factory = CreateFactory();
    PixelTarget output(factory.Get());
    const auto bytes = BuildApng(1000, 1000, {Frame(1000, 1000, kRed), Frame(1000, 1000, kBlue)});
    const TempFile first(bytes, L".apng"), second(bytes, L".apng"), third(bytes, L".apng"), fourth(bytes, L".apng");
    const TempFile extra(BuildApng(2, 2, {Frame(2, 2, kRed), Frame(2, 2, kBlue)}), L".apng");
    msime::windows::CandidateImageFrames cache;
    cache.begin_frame(output.target.Get(), 0, true);
    for (const auto *file : {&first, &second, &third, &fourth})
        REQUIRE(cache.frame(file->path) != nullptr);
    REQUIRE(cache.decoded_bytes() == 32000000U);
    // 每张资源分别合法，累计预算耗尽后的动画仍须退回静态路径。
    REQUIRE(cache.frame(extra.path) == nullptr);
    REQUIRE(cache.decoded_bytes() == 32000000U);
    cache.clear();
    cache.begin_frame(output.target.Get(), 0, true);
    REQUIRE(cache.frame(extra.path) != nullptr);
    REQUIRE(cache.decoded_bytes() == 2U * 2 * 4 * 2);
}
