#pragma once
#include <windows.h>
#include <wincodec.h>
#include <wrl/client.h>
#include <string>

namespace msime::windows {
// canvas 皮肤的一张图（九宫格面板或自由挂件）能不能画：用和 msimeui DeviceResources::GetBitmapFromFile 相同的 WIC 路径打开文件、取第一帧、转成 32bpp PBGRA，并把像素真正解出来。只看文件头会放过数据损坏的图，那样候选窗每画一帧都要重新解一次，而且面板解不出来时卡片要用的是另一套配色（共享层的 `canvas_panel_drawn`），必须在套用皮肤时就知道。动图（APNG、GIF、WebP）只看第一帧，Windows 宿主只画第一帧。系统没有对应解码器（例如没装 WebP 扩展）时同样算解不出来。调用线程要已经进入 COM 套间：Server 主线程和截图工具都由 CandidateWindow 的 Apartment 或自己的 CoInitializeEx 负责。
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
