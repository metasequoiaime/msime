// 移植自 msime-windows 固定提交 dd018f79；来源、GPL 许可与本地调整见 msimeui/UPSTREAM.md。
#include "msimeui/AnimatedImage.h"

#include <algorithm>
#include <array>
#include <cstring>
#include <shlwapi.h>
#include <utility>
#include <windows.h>
#include <wrl/client.h>

namespace msimeui
{
using Microsoft::WRL::ComPtr;

namespace
{
constexpr uint8_t kPngSignature[8] = {0x89, 'P', 'N', 'G', 0x0D, 0x0A, 0x1A, 0x0A};

constexpr uint32_t Tag(char a, char b, char c, char d)
{
    return (static_cast<uint32_t>(static_cast<uint8_t>(a)) << 24) |
           (static_cast<uint32_t>(static_cast<uint8_t>(b)) << 16) |
           (static_cast<uint32_t>(static_cast<uint8_t>(c)) << 8) | static_cast<uint32_t>(static_cast<uint8_t>(d));
}

constexpr uint32_t kIhdr = Tag('I', 'H', 'D', 'R');
constexpr uint32_t kIdat = Tag('I', 'D', 'A', 'T');
constexpr uint32_t kIend = Tag('I', 'E', 'N', 'D');
constexpr uint32_t kActl = Tag('a', 'c', 'T', 'L');
constexpr uint32_t kFctl = Tag('f', 'c', 'T', 'L');
constexpr uint32_t kFdat = Tag('f', 'd', 'A', 'T');

enum class Dispose : uint8_t
{
    None = 0,
    Background = 1,
    Previous = 2,
};

uint32_t ReadBE32(const uint8_t *p)
{
    return (static_cast<uint32_t>(p[0]) << 24) | (static_cast<uint32_t>(p[1]) << 16) |
           (static_cast<uint32_t>(p[2]) << 8) | static_cast<uint32_t>(p[3]);
}

uint16_t ReadBE16(const uint8_t *p)
{
    return static_cast<uint16_t>((p[0] << 8) | p[1]);
}

void AppendBE32(std::vector<uint8_t> &out, uint32_t value)
{
    out.push_back(static_cast<uint8_t>(value >> 24));
    out.push_back(static_cast<uint8_t>(value >> 16));
    out.push_back(static_cast<uint8_t>(value >> 8));
    out.push_back(static_cast<uint8_t>(value));
}

const std::array<uint32_t, 256> &CrcTable()
{
    static const std::array<uint32_t, 256> table = []() {
        std::array<uint32_t, 256> result{};
        for (uint32_t n = 0; n < 256; ++n)
        {
            uint32_t c = n;
            for (int k = 0; k < 8; ++k)
            {
                c = (c & 1U) ? 0xEDB88320U ^ (c >> 1) : c >> 1;
            }
            result[n] = c;
        }
        return result;
    }();
    return table;
}

uint32_t UpdateCrc(uint32_t crc, const uint8_t *data, size_t size)
{
    const auto &table = CrcTable();
    for (size_t i = 0; i < size; ++i)
    {
        crc = table[(crc ^ data[i]) & 0xFFU] ^ (crc >> 8);
    }
    return crc;
}

uint32_t ChunkCrc(uint32_t type, const uint8_t *data, size_t size)
{
    const uint8_t typeBytes[4] = {static_cast<uint8_t>(type >> 24), static_cast<uint8_t>(type >> 16),
                                  static_cast<uint8_t>(type >> 8), static_cast<uint8_t>(type)};
    uint32_t crc = UpdateCrc(0xFFFFFFFFU, typeBytes, 4);
    crc = UpdateCrc(crc, data, size);
    return crc ^ 0xFFFFFFFFU;
}

void AppendChunk(std::vector<uint8_t> &out, uint32_t type, const uint8_t *data, size_t size)
{
    AppendBE32(out, static_cast<uint32_t>(size));
    AppendBE32(out, type);
    if (size > 0)
    {
        out.insert(out.end(), data, data + size);
    }
    AppendBE32(out, ChunkCrc(type, data, size));
}

struct PngChunk
{
    uint32_t type = 0;
    const uint8_t *data = nullptr;
    uint32_t length = 0;
};

bool HasPngSignature(const uint8_t *data, size_t size)
{
    return size >= sizeof(kPngSignature) && std::memcmp(data, kPngSignature, sizeof(kPngSignature)) == 0;
}

bool ReadPngChunks(const uint8_t *data, size_t size, bool verifyCrc, std::vector<PngChunk> &chunks)
{
    if (!HasPngSignature(data, size))
    {
        return false;
    }
    size_t pos = sizeof(kPngSignature);
    while (pos + 12 <= size)
    {
        const uint32_t length = ReadBE32(data + pos);
        if (length > 0x7FFFFFFFU || length > size - pos - 12)
        {
            return false;
        }
        PngChunk chunk;
        chunk.type = ReadBE32(data + pos + 4);
        chunk.data = data + pos + 8;
        chunk.length = length;
        if (verifyCrc && ChunkCrc(chunk.type, chunk.data, length) != ReadBE32(data + pos + 8 + length))
        {
            return false;
        }
        chunks.push_back(chunk);
        pos += 12 + static_cast<size_t>(length);
        if (chunk.type == kIend)
        {
            return true;
        }
    }
    return false;
}

ComPtr<IWICBitmapDecoder> CreateDecoder(IWICImagingFactory *factory, const uint8_t *data, size_t size)
{
    ComPtr<IWICBitmapDecoder> decoder;
    const UINT streamSize = static_cast<UINT>(size);
    if (!factory || !data || size == 0 || static_cast<size_t>(streamSize) != size)
    {
        return decoder;
    }
    ComPtr<IStream> stream;
    stream.Attach(SHCreateMemStream(data, streamSize));
    if (!stream || FAILED(factory->CreateDecoderFromStream(stream.Get(), nullptr, WICDecodeMetadataCacheOnLoad,
                                                           decoder.GetAddressOf())))
    {
        decoder.Reset();
    }
    return decoder;
}

bool CopyPremultipliedBgra(IWICImagingFactory *factory, IWICBitmapSource *source, uint32_t maxDimension,
                           uint32_t &width, uint32_t &height, std::vector<uint8_t> &pixels)
{
    UINT w = 0;
    UINT h = 0;
    ComPtr<IWICFormatConverter> converter;
    if (!source || FAILED(source->GetSize(&w, &h)) || w == 0 || h == 0 || w > maxDimension || h > maxDimension ||
        FAILED(factory->CreateFormatConverter(converter.GetAddressOf())) ||
        FAILED(converter->Initialize(source, GUID_WICPixelFormat32bppPBGRA, WICBitmapDitherTypeNone, nullptr, 0.0,
                                     WICBitmapPaletteTypeMedianCut)))
    {
        return false;
    }
    const size_t stride = static_cast<size_t>(w) * 4;
    pixels.assign(stride * h, 0);
    if (FAILED(
            converter->CopyPixels(nullptr, static_cast<UINT>(stride), static_cast<UINT>(pixels.size()), pixels.data())))
    {
        return false;
    }
    width = w;
    height = h;
    return true;
}

struct Region
{
    uint32_t x = 0;
    uint32_t y = 0;
    uint32_t width = 0;
    uint32_t height = 0;
};

class Canvas
{
  public:
    Canvas(uint32_t width, uint32_t height)
        : width_(width), height_(height), pixels_(static_cast<size_t>(width) * height * 4, 0)
    {
    }

    void Draw(const std::vector<uint8_t> &frame, uint32_t frameWidth, uint32_t frameHeight, int64_t left, int64_t top,
              bool over)
    {
        const Region region = Clip(left, top, frameWidth, frameHeight);
        for (uint32_t row = 0; row < region.height; ++row)
        {
            const size_t srcY = static_cast<size_t>(region.y - top + row);
            const size_t srcX = static_cast<size_t>(region.x - left);
            const uint8_t *src = frame.data() + (srcY * frameWidth + srcX) * 4;
            uint8_t *dst = pixels_.data() + ((static_cast<size_t>(region.y) + row) * width_ + region.x) * 4;
            if (!over)
            {
                std::memcpy(dst, src, static_cast<size_t>(region.width) * 4);
                continue;
            }
            for (uint32_t col = 0; col < region.width; ++col, src += 4, dst += 4)
            {
                const uint32_t alpha = src[3];
                if (alpha == 255)
                {
                    std::memcpy(dst, src, 4);
                }
                else if (alpha != 0)
                {
                    for (int c = 0; c < 4; ++c)
                    {
                        dst[c] =
                            static_cast<uint8_t>((std::min)(255U, src[c] + (dst[c] * (255U - alpha) + 127U) / 255U));
                    }
                }
            }
        }
    }

    Region Clip(int64_t left, int64_t top, uint32_t width, uint32_t height) const
    {
        const int64_t x0 = (std::max)(left, int64_t{0});
        const int64_t y0 = (std::max)(top, int64_t{0});
        const int64_t x1 = (std::min)(left + static_cast<int64_t>(width), static_cast<int64_t>(width_));
        const int64_t y1 = (std::min)(top + static_cast<int64_t>(height), static_cast<int64_t>(height_));
        if (x1 <= x0 || y1 <= y0)
        {
            return {};
        }
        return {static_cast<uint32_t>(x0), static_cast<uint32_t>(y0), static_cast<uint32_t>(x1 - x0),
                static_cast<uint32_t>(y1 - y0)};
    }

    void Clear(const Region &region)
    {
        for (uint32_t row = 0; row < region.height; ++row)
        {
            std::memset(pixels_.data() + ((static_cast<size_t>(region.y) + row) * width_ + region.x) * 4, 0,
                        static_cast<size_t>(region.width) * 4);
        }
    }

    std::vector<uint8_t> Save(const Region &region) const
    {
        std::vector<uint8_t> saved(static_cast<size_t>(region.width) * region.height * 4);
        for (uint32_t row = 0; row < region.height; ++row)
        {
            std::memcpy(saved.data() + static_cast<size_t>(row) * region.width * 4,
                        pixels_.data() + ((static_cast<size_t>(region.y) + row) * width_ + region.x) * 4,
                        static_cast<size_t>(region.width) * 4);
        }
        return saved;
    }

    void Restore(const Region &region, const std::vector<uint8_t> &saved)
    {
        for (uint32_t row = 0; row < region.height; ++row)
        {
            std::memcpy(pixels_.data() + ((static_cast<size_t>(region.y) + row) * width_ + region.x) * 4,
                        saved.data() + static_cast<size_t>(row) * region.width * 4,
                        static_cast<size_t>(region.width) * 4);
        }
    }

    const std::vector<uint8_t> &Pixels() const
    {
        return pixels_;
    }

  private:
    uint32_t width_;
    uint32_t height_;
    std::vector<uint8_t> pixels_;
};

struct FrameStep
{
    std::vector<uint8_t> pixels;
    uint32_t width = 0;
    uint32_t height = 0;
    int64_t left = 0;
    int64_t top = 0;
    bool over = false;
    Dispose dispose = Dispose::None;
};

std::vector<uint8_t> ComposeStep(Canvas &canvas, const FrameStep &step, bool first)
{
    Dispose dispose = step.dispose;

    if (first && dispose == Dispose::Previous)
    {
        dispose = Dispose::Background;
    }
    const Region region = canvas.Clip(step.left, step.top, step.width, step.height);
    std::vector<uint8_t> saved;
    if (dispose == Dispose::Previous)
    {
        saved = canvas.Save(region);
    }
    canvas.Draw(step.pixels, step.width, step.height, step.left, step.top, step.over);
    std::vector<uint8_t> shown = canvas.Pixels();
    if (dispose == Dispose::Background)
    {
        canvas.Clear(region);
    }
    else if (dispose == Dispose::Previous)
    {
        canvas.Restore(region, saved);
    }
    return shown;
}

bool ReadMetadataUInt(IWICMetadataQueryReader *reader, const wchar_t *name, uint32_t &value)
{
    if (!reader)
    {
        return false;
    }
    PROPVARIANT variant;
    PropVariantInit(&variant);
    bool ok = false;
    if (SUCCEEDED(reader->GetMetadataByName(name, &variant)))
    {
        switch (variant.vt)
        {
        case VT_UI1:
            value = variant.bVal;
            ok = true;
            break;
        case VT_UI2:
            value = variant.uiVal;
            ok = true;
            break;
        case VT_UI4:
            value = variant.ulVal;
            ok = true;
            break;
        default:
            break;
        }
    }
    PropVariantClear(&variant);
    return ok;
}

bool ReadMetadataBytes(IWICMetadataQueryReader *reader, const wchar_t *name, std::vector<uint8_t> &bytes)
{
    if (!reader)
    {
        return false;
    }
    PROPVARIANT variant;
    PropVariantInit(&variant);
    bool ok = false;
    if (SUCCEEDED(reader->GetMetadataByName(name, &variant)) && variant.vt == (VT_UI1 | VT_VECTOR))
    {
        bytes.assign(variant.caub.pElems, variant.caub.pElems + variant.caub.cElems);
        ok = true;
    }
    PropVariantClear(&variant);
    return ok;
}

bool ReadFileBytes(const std::wstring &filePath, uint64_t maxBytes, std::vector<uint8_t> &bytes)
{
    HANDLE file = CreateFileW(filePath.c_str(), GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_DELETE, nullptr,
                              OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (file == INVALID_HANDLE_VALUE)
    {
        return false;
    }
    LARGE_INTEGER size{};
    bool ok = GetFileSizeEx(file, &size) && size.QuadPart > 0 && static_cast<uint64_t>(size.QuadPart) <= maxBytes;
    if (ok)
    {
        bytes.resize(static_cast<size_t>(size.QuadPart));
        size_t done = 0;
        while (ok && done < bytes.size())
        {
            DWORD read = 0;
            const DWORD chunk = static_cast<DWORD>((std::min)(bytes.size() - done, size_t{1} << 30));
            ok = ReadFile(file, bytes.data() + done, chunk, &read, nullptr) && read > 0;
            done += read;
        }
    }
    CloseHandle(file);
    return ok;
}

bool FitsLimits(uint32_t width, uint32_t height, size_t frames, const AnimatedImage::Limits &limits)
{
    if (width == 0 || height == 0 || width > limits.maxDimension || height > limits.maxDimension ||
        frames > limits.maxFrames)
    {
        return false;
    }
    return static_cast<uint64_t>(width) * height * 4 * frames <= limits.maxDecodedBytes;
}
}

struct AnimatedImageDecoder
{

    static bool DecodeApng(IWICImagingFactory *factory, const uint8_t *data, size_t size,
                           const AnimatedImage::Limits &limits, AnimatedImage &out)
    {
        std::vector<PngChunk> chunks;
        if (!ReadPngChunks(data, size, true, chunks) || chunks.empty() || chunks.front().type != kIhdr ||
            chunks.front().length != 13)
        {
            return false;
        }
        const uint8_t *ihdr = chunks.front().data;
        const uint32_t width = ReadBE32(ihdr);
        const uint32_t height = ReadBE32(ihdr + 4);

        struct Frame
        {
            uint32_t width = 0;
            uint32_t height = 0;
            uint32_t x = 0;
            uint32_t y = 0;
            uint16_t delayNum = 0;
            uint16_t delayDen = 0;
            uint8_t dispose = 0;
            uint8_t blend = 0;
            std::vector<std::pair<const uint8_t *, uint32_t>> data;
        };
        std::vector<Frame> frames;
        std::vector<PngChunk> header;
        bool haveActl = false;
        uint32_t declaredFrames = 0;
        uint32_t playCount = 0;
        uint32_t sequence = 0;
        bool idatSeen = false;
        bool idatDone = false;

        bool defaultIsFrame = false;
        uint32_t previousType = kIhdr;
        for (size_t i = 1; i < chunks.size(); ++i)
        {
            const PngChunk &chunk = chunks[i];
            if (idatSeen && previousType == kIdat && chunk.type != kIdat)
            {
                idatDone = true;
            }
            previousType = chunk.type;
            switch (chunk.type)
            {
            case kActl:
                if (haveActl || idatSeen || chunk.length != 8)
                {
                    return false;
                }
                haveActl = true;
                declaredFrames = ReadBE32(chunk.data);
                playCount = ReadBE32(chunk.data + 4);
                break;
            case kFctl: {
                if (chunk.length != 26 || ReadBE32(chunk.data) != sequence++)
                {
                    return false;
                }

                if (!frames.empty() && frames.back().data.empty())
                {
                    return false;
                }
                Frame frame;
                frame.width = ReadBE32(chunk.data + 4);
                frame.height = ReadBE32(chunk.data + 8);
                frame.x = ReadBE32(chunk.data + 12);
                frame.y = ReadBE32(chunk.data + 16);
                frame.delayNum = ReadBE16(chunk.data + 20);
                frame.delayDen = ReadBE16(chunk.data + 22);
                frame.dispose = chunk.data[24];
                frame.blend = chunk.data[25];
                if (frame.width == 0 || frame.height == 0 || frame.x > width || frame.y > height ||
                    frame.width > width - frame.x || frame.height > height - frame.y || frame.dispose > 2 ||
                    frame.blend > 1)
                {
                    return false;
                }
                if (!idatSeen)
                {

                    if (!frames.empty() || frame.x != 0 || frame.y != 0 || frame.width != width ||
                        frame.height != height)
                    {
                        return false;
                    }
                    defaultIsFrame = true;
                }
                frames.push_back(std::move(frame));
                break;
            }
            case kIdat:
                if (idatDone)
                {
                    return false;
                }
                idatSeen = true;
                if (defaultIsFrame)
                {
                    if (frames.size() != 1)
                    {
                        return false;
                    }
                    frames.front().data.emplace_back(chunk.data, chunk.length);
                }
                break;
            case kFdat:

                if (chunk.length < 4 || ReadBE32(chunk.data) != sequence++ || !idatSeen || frames.empty() ||
                    (defaultIsFrame && frames.size() == 1))
                {
                    return false;
                }
                frames.back().data.emplace_back(chunk.data + 4, chunk.length - 4);
                break;
            case kIend:
                break;
            default:
                if (!idatSeen)
                {
                    header.push_back(chunk);
                }
                break;
            }
        }
        if (!haveActl || !idatSeen || declaredFrames == 0 || frames.size() != declaredFrames ||
            !FitsLimits(width, height, frames.size(), limits))
        {
            return false;
        }
        for (const Frame &frame : frames)
        {
            if (frame.data.empty())
            {
                return false;
            }
        }

        Canvas canvas(width, height);
        out.frames_.clear();
        out.timeline_ = {};
        for (size_t index = 0; index < frames.size(); ++index)
        {
            const Frame &frame = frames[index];

            std::vector<uint8_t> png(kPngSignature, kPngSignature + sizeof(kPngSignature));
            uint8_t frameIhdr[13];
            std::memcpy(frameIhdr, ihdr, sizeof(frameIhdr));
            for (int b = 0; b < 4; ++b)
            {
                frameIhdr[b] = static_cast<uint8_t>(frame.width >> (24 - 8 * b));
                frameIhdr[4 + b] = static_cast<uint8_t>(frame.height >> (24 - 8 * b));
            }
            AppendChunk(png, kIhdr, frameIhdr, sizeof(frameIhdr));
            for (const PngChunk &chunk : header)
            {
                if (chunk.type != kActl && chunk.type != kFctl)
                {
                    AppendChunk(png, chunk.type, chunk.data, chunk.length);
                }
            }
            for (const auto &part : frame.data)
            {
                AppendChunk(png, kIdat, part.first, part.second);
            }
            AppendChunk(png, kIend, nullptr, 0);

            ComPtr<IWICBitmapDecoder> decoder = CreateDecoder(factory, png.data(), png.size());
            ComPtr<IWICBitmapFrameDecode> decoded;
            FrameStep step;
            if (!decoder || FAILED(decoder->GetFrame(0, decoded.GetAddressOf())) ||
                !CopyPremultipliedBgra(factory, decoded.Get(), limits.maxDimension, step.width, step.height,
                                       step.pixels) ||
                step.width != frame.width || step.height != frame.height)
            {
                return false;
            }
            step.left = frame.x;
            step.top = frame.y;
            step.over = frame.blend == 1;
            step.dispose = static_cast<Dispose>(frame.dispose);
            out.frames_.push_back(ComposeStep(canvas, step, index == 0));

            const uint32_t den = frame.delayDen == 0 ? 100U : frame.delayDen;
            const uint32_t delayMs = (static_cast<uint32_t>(frame.delayNum) * 1000U + den / 2U) / den;
            out.timeline_.frameDelaysMs.push_back(NormalizeFrameDelayMs(delayMs));
        }
        out.width_ = width;
        out.height_ = height;
        out.timeline_.playCount = playCount;
        out.format_ = AnimatedImage::Format::Apng;
        return true;
    }

    static bool DecodeGif(IWICImagingFactory *factory, IWICBitmapDecoder *decoder, const AnimatedImage::Limits &limits,
                          AnimatedImage &out)
    {
        UINT count = 0;
        if (FAILED(decoder->GetFrameCount(&count)) || count < 2)
        {
            return false;
        }
        ComPtr<IWICMetadataQueryReader> reader;
        decoder->GetMetadataQueryReader(reader.GetAddressOf());
        uint32_t width = 0;
        uint32_t height = 0;
        ReadMetadataUInt(reader.Get(), L"/logscrdesc/Width", width);
        ReadMetadataUInt(reader.Get(), L"/logscrdesc/Height", height);
        if (width == 0 || height == 0)
        {
            ComPtr<IWICBitmapFrameDecode> first;
            UINT w = 0;
            UINT h = 0;
            if (FAILED(decoder->GetFrame(0, first.GetAddressOf())) || FAILED(first->GetSize(&w, &h)))
            {
                return false;
            }
            width = w;
            height = h;
        }
        if (!FitsLimits(width, height, count, limits))
        {
            return false;
        }

        uint32_t playCount = 1;
        std::vector<uint8_t> application;
        std::vector<uint8_t> loop;
        if (ReadMetadataBytes(reader.Get(), L"/appext/application", application) && application.size() == 11 &&
            (std::memcmp(application.data(), "NETSCAPE2.0", 11) == 0 ||
             std::memcmp(application.data(), "ANIMEXTS1.0", 11) == 0) &&
            ReadMetadataBytes(reader.Get(), L"/appext/data", loop) && loop.size() >= 4 && loop[0] >= 3 && loop[1] == 1)
        {
            const uint32_t repeats = static_cast<uint32_t>(loop[2]) | (static_cast<uint32_t>(loop[3]) << 8);
            playCount = repeats == 0 ? 0 : repeats + 1;
        }

        Canvas canvas(width, height);
        out.frames_.clear();
        out.timeline_ = {};
        for (UINT index = 0; index < count; ++index)
        {
            ComPtr<IWICBitmapFrameDecode> frame;
            if (FAILED(decoder->GetFrame(index, frame.GetAddressOf())))
            {
                return false;
            }
            ComPtr<IWICMetadataQueryReader> frameReader;
            frame->GetMetadataQueryReader(frameReader.GetAddressOf());
            uint32_t left = 0;
            uint32_t top = 0;
            uint32_t delay = 0;
            uint32_t disposal = 0;
            ReadMetadataUInt(frameReader.Get(), L"/imgdesc/Left", left);
            ReadMetadataUInt(frameReader.Get(), L"/imgdesc/Top", top);
            ReadMetadataUInt(frameReader.Get(), L"/grctlext/Delay", delay);
            ReadMetadataUInt(frameReader.Get(), L"/grctlext/Disposal", disposal);
            FrameStep step;
            if (!CopyPremultipliedBgra(factory, frame.Get(), limits.maxDimension, step.width, step.height, step.pixels))
            {
                return false;
            }
            step.left = left;
            step.top = top;

            step.over = true;
            step.dispose = disposal == 2 ? Dispose::Background : disposal == 3 ? Dispose::Previous : Dispose::None;
            out.frames_.push_back(ComposeStep(canvas, step, index == 0));
            out.timeline_.frameDelaysMs.push_back(NormalizeFrameDelayMs(delay * 10U));
        }
        out.width_ = width;
        out.height_ = height;
        out.timeline_.playCount = playCount;
        out.format_ = AnimatedImage::Format::Gif;
        return true;
    }

    static bool DecodeMultiFrame(IWICImagingFactory *factory, IWICBitmapDecoder *decoder,
                                 const AnimatedImage::Limits &limits, AnimatedImage &out)
    {
        UINT count = 0;
        if (FAILED(decoder->GetFrameCount(&count)) || count < 2 || count > limits.maxFrames)
        {
            return false;
        }
        out.frames_.clear();
        out.timeline_ = {};
        for (UINT index = 0; index < count; ++index)
        {
            ComPtr<IWICBitmapFrameDecode> frame;
            uint32_t width = 0;
            uint32_t height = 0;
            std::vector<uint8_t> pixels;
            if (FAILED(decoder->GetFrame(index, frame.GetAddressOf())) ||
                !CopyPremultipliedBgra(factory, frame.Get(), limits.maxDimension, width, height, pixels) ||
                (index > 0 && (width != out.width_ || height != out.height_)) ||
                !FitsLimits(width, height, count, limits))
            {
                return false;
            }
            out.width_ = width;
            out.height_ = height;
            out.frames_.push_back(std::move(pixels));
            out.timeline_.frameDelaysMs.push_back(100U);
        }
        out.format_ = AnimatedImage::Format::MultiFrame;
        return true;
    }

    static bool DecodeStill(IWICImagingFactory *factory, IWICBitmapDecoder *decoder,
                            const AnimatedImage::Limits &limits, AnimatedImage &out)
    {
        ComPtr<IWICBitmapFrameDecode> frame;
        std::vector<uint8_t> pixels;
        uint32_t width = 0;
        uint32_t height = 0;
        if (FAILED(decoder->GetFrame(0, frame.GetAddressOf())) ||
            !CopyPremultipliedBgra(factory, frame.Get(), limits.maxDimension, width, height, pixels)
            || !FitsLimits(width, height, 1, limits))
        {
            return false;
        }
        out.width_ = width;
        out.height_ = height;
        out.frames_.clear();
        out.frames_.push_back(std::move(pixels));
        out.timeline_ = {};
        out.format_ = AnimatedImage::Format::Still;
        return true;
    }
};

uint32_t NormalizeFrameDelayMs(uint32_t delayMs)
{
    return delayMs <= 10U ? 100U : delayMs;
}

uint64_t AnimationTimeline::LoopDurationMs() const
{
    uint64_t total = 0;
    for (const uint32_t delay : frameDelaysMs)
    {
        total += delay;
    }
    return total;
}

AnimationTimeline::Sample AnimationTimeline::SampleAt(uint64_t elapsedMs) const
{
    if (frameDelaysMs.size() <= 1)
    {
        return {};
    }
    const size_t last = frameDelaysMs.size() - 1;
    const uint64_t loop = LoopDurationMs();
    if (loop == 0)
    {
        return {};
    }
    const uint64_t play = elapsedMs / loop;
    if (playCount != 0 && play >= playCount)
    {
        return {last, std::nullopt};
    }
    const uint64_t loopStart = play * loop;
    const uint64_t intoLoop = elapsedMs - loopStart;
    uint64_t frameEnd = 0;
    for (size_t index = 0; index <= last; ++index)
    {
        frameEnd += frameDelaysMs[index];
        if (intoLoop < frameEnd)
        {
            const bool finalFrame = playCount != 0 && play + 1 == playCount && index == last;
            if (finalFrame)
            {
                return {index, std::nullopt};
            }
            return {index, loopStart + frameEnd};
        }
    }
    return {last, std::nullopt};
}

std::optional<AnimatedImage> AnimatedImage::DecodeMemory(IWICImagingFactory *factory, const uint8_t *data, size_t size,
                                                         const Limits &limits)
{
    if (!factory || !data || size == 0 || size > limits.maxFileBytes)
    {
        return std::nullopt;
    }
    AnimatedImage image;
    if (HasPngSignature(data, size) && AnimatedImageDecoder::DecodeApng(factory, data, size, limits, image))
    {
        return image;
    }
    ComPtr<IWICBitmapDecoder> decoder = CreateDecoder(factory, data, size);
    if (!decoder)
    {
        return std::nullopt;
    }
    GUID container = {};
    if (SUCCEEDED(decoder->GetContainerFormat(&container)))
    {
        if (IsEqualGUID(container, GUID_ContainerFormatGif) &&
            AnimatedImageDecoder::DecodeGif(factory, decoder.Get(), limits, image))
        {
            return image;
        }
        if (IsEqualGUID(container, GUID_ContainerFormatWebp) &&
            AnimatedImageDecoder::DecodeMultiFrame(factory, decoder.Get(), limits, image))
        {
            return image;
        }
    }
    image = AnimatedImage();
    if (AnimatedImageDecoder::DecodeStill(factory, decoder.Get(), limits, image))
    {
        return image;
    }
    return std::nullopt;
}

std::optional<AnimatedImage> AnimatedImage::DecodeFile(IWICImagingFactory *factory, const std::wstring &filePath,
                                                       const Limits &limits)
{
    std::vector<uint8_t> bytes;
    if (!ReadFileBytes(filePath, limits.maxFileBytes, bytes))
    {
        return std::nullopt;
    }
    return DecodeMemory(factory, bytes.data(), bytes.size(), limits);
}

bool AnimatedImage::MayBeAnimated(IWICImagingFactory *factory, const std::wstring &filePath)
{
    std::vector<uint8_t> bytes;
    if (!factory || !ReadFileBytes(filePath, Limits().maxFileBytes, bytes))
    {
        return false;
    }
    if (HasPngSignature(bytes.data(), bytes.size()))
    {

        size_t pos = sizeof(kPngSignature);
        while (pos + 12 <= bytes.size())
        {
            const uint32_t length = ReadBE32(bytes.data() + pos);
            const uint32_t type = ReadBE32(bytes.data() + pos + 4);
            if (type == kActl)
            {
                return true;
            }
            if (type == kIdat || type == kIend || length > bytes.size() - pos - 12)
            {
                return false;
            }
            pos += 12 + static_cast<size_t>(length);
        }
        return false;
    }
    ComPtr<IWICBitmapDecoder> decoder = CreateDecoder(factory, bytes.data(), bytes.size());
    GUID container = {};
    UINT count = 0;
    return decoder && SUCCEEDED(decoder->GetContainerFormat(&container)) &&
           (IsEqualGUID(container, GUID_ContainerFormatGif) || IsEqualGUID(container, GUID_ContainerFormatWebp)) &&
           SUCCEEDED(decoder->GetFrameCount(&count)) && count > 1;
}
}
