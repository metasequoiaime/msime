#pragma once
#include <windows.h>
#include <wincodec.h>
#include <wrl/client.h>
#include <string>

namespace msime::windows {
// 套用 canvas 配色前，用 WIC 完整解码默认图，确认至少能提供静态可读外观。
// 动画由 CandidateImageFrames 另行有界解码、合成并播放；损坏或超预算时保持这个默认图。
// WebP 沿用上游的系统 WIC 解码器要求；调用线程必须已进入 COM 套间。

inline bool candidate_canvas_image_decodes(const std::wstring &path) {
  if (path.empty())
    return false;
  Microsoft::WRL::ComPtr<IWICImagingFactory> factory;
  if (FAILED(CoCreateInstance(CLSID_WICImagingFactory, nullptr, CLSCTX_INPROC_SERVER,
                              IID_PPV_ARGS(factory.GetAddressOf()))))
    return false;
  Microsoft::WRL::ComPtr<IWICBitmapDecoder> decoder;
  Microsoft::WRL::ComPtr<IWICBitmapFrameDecode> frame;
  Microsoft::WRL::ComPtr<IWICFormatConverter> converter;
  Microsoft::WRL::ComPtr<IWICBitmap> bitmap;
  UINT width = 0, height = 0;
  return SUCCEEDED(factory->CreateDecoderFromFilename(path.c_str(), nullptr, GENERIC_READ,
                                                      WICDecodeMetadataCacheOnLoad,
                                                      decoder.GetAddressOf())) &&
         SUCCEEDED(decoder->GetFrame(0, frame.GetAddressOf())) &&
         SUCCEEDED(frame->GetSize(&width, &height)) && width > 0 && height > 0 &&
         SUCCEEDED(factory->CreateFormatConverter(converter.GetAddressOf())) &&
         SUCCEEDED(converter->Initialize(frame.Get(), GUID_WICPixelFormat32bppPBGRA,
                                         WICBitmapDitherTypeNone, nullptr, 0.0,
                                         WICBitmapPaletteTypeMedianCut)) &&
         // WICBitmapCacheOnLoad 让 WIC 在这里就把整帧解出来。
         SUCCEEDED(factory->CreateBitmapFromSource(converter.Get(), WICBitmapCacheOnLoad,
                                                   bitmap.GetAddressOf()));
}
} // namespace msime::windows
