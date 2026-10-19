#pragma once
// Windows.UI.Composition 的 ABI 接口，只声明 DeviceResources 的系统材质（亚克力、云母）用到的那几个，只给 DeviceResources.cpp 用。
//
// MinGW 的 windows.ui.composition.h 只有最早一版的接口：没有 ICompositor3/5/6、壁纸模糊画刷、桌面窗口互操作和绘制表面互操作，也没有 wrl/implements.h；MSVC 的 Windows SDK 都有。为了两种工具链编同一份代码、CI 上 MinGW 编出的截图工具画的就是 MSVC 发行包画的，这里按 Windows SDK 元数据自己声明（方法次序与 windows-rs 0.62 从同一份元数据生成的 vtable 一致），不包含任何 WinRT 头文件。用到的方法写出签名，前面不用的槽位用占位方法占住次序；每个方法名都不同，避免 MSVC 把同名的重载在 vtable 里排到一起。用到的系统函数（RoActivateInstance、CreateDispatcherQueueController 等）由 DeviceResources.cpp 在运行时取，缺了就退回不透明底色。
#include <windows.h>
#include <inspectable.h>

namespace msimeui::composition
{
struct Vector2
{
    float X;
    float Y;
};
struct Vector3
{
    float X;
    float Y;
    float Z;
};
// Windows.Foundation.Size。
struct Size
{
    float Width;
    float Height;
};
// Windows.Graphics.DirectX.DirectXPixelFormat 与 DirectXAlphaMode 里用到的值。
constexpr int kPixelFormatB8G8R8A8UIntNormalized = 87;
constexpr int kAlphaModePremultiplied = 1;
// Windows.UI.Composition.CompositionStretch.None。
constexpr int kStretchNone = 0;

#define MSIMEUI_UNUSED_SLOT(name) virtual HRESULT STDMETHODCALLTYPE name() = 0

// 只拿来做 QueryInterface 目标或在 API 之间传递、不调用自己方法的接口。
struct ICompositionBrush : IInspectable
{
};
struct ICompositionBackdropBrush : IInspectable
{
};
struct ICompositionClip : IInspectable
{
};
struct ICompositionGeometricClip : IInspectable
{
};
struct ICompositionGeometry : IInspectable
{
};
struct ICompositionSurface : IInspectable
{
};
struct ICompositionDrawingSurface : IInspectable
{
};
struct ICompositionEffectSourceParameter : IInspectable
{
};
struct IDesktopWindowTarget : IInspectable
{
};
struct IPropertyValue : IInspectable
{
};
struct IGraphicsEffectSource : IInspectable
{
};

struct IGraphicsEffect : IInspectable
{
    virtual HRESULT STDMETHODCALLTYPE get_Name(HSTRING *name) = 0;
    virtual HRESULT STDMETHODCALLTYPE put_Name(HSTRING name) = 0;
};

// Windows.Graphics.Effects 的 D2D 互操作：合成器从这里读效果的 CLSID、按下标读属性和输入，编译成效果画刷。
struct IGraphicsEffectD2D1Interop : IUnknown
{
    virtual HRESULT STDMETHODCALLTYPE GetEffectId(GUID *id) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetNamedPropertyMapping(LPCWSTR name, UINT *index, int *mapping) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetPropertyCount(UINT *count) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetProperty(UINT index, IPropertyValue **value) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetSource(UINT index, IGraphicsEffectSource **source) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetSourceCount(UINT *count) = 0;
};

struct IVisual : IInspectable
{
    MSIMEUI_UNUSED_SLOT(get_AnchorPoint);
    MSIMEUI_UNUSED_SLOT(put_AnchorPoint);
    MSIMEUI_UNUSED_SLOT(get_BackfaceVisibility);
    MSIMEUI_UNUSED_SLOT(put_BackfaceVisibility);
    MSIMEUI_UNUSED_SLOT(get_BorderMode);
    MSIMEUI_UNUSED_SLOT(put_BorderMode);
    MSIMEUI_UNUSED_SLOT(get_CenterPoint);
    MSIMEUI_UNUSED_SLOT(put_CenterPoint);
    MSIMEUI_UNUSED_SLOT(get_Clip);
    virtual HRESULT STDMETHODCALLTYPE put_Clip(ICompositionClip *clip) = 0;
    MSIMEUI_UNUSED_SLOT(get_CompositeMode);
    MSIMEUI_UNUSED_SLOT(put_CompositeMode);
    MSIMEUI_UNUSED_SLOT(get_IsVisible);
    virtual HRESULT STDMETHODCALLTYPE put_IsVisible(boolean visible) = 0;
    MSIMEUI_UNUSED_SLOT(get_Offset);
    virtual HRESULT STDMETHODCALLTYPE put_Offset(Vector3 offset) = 0;
    MSIMEUI_UNUSED_SLOT(get_Opacity);
    virtual HRESULT STDMETHODCALLTYPE put_Opacity(float opacity) = 0;
    MSIMEUI_UNUSED_SLOT(get_Orientation);
    MSIMEUI_UNUSED_SLOT(put_Orientation);
    MSIMEUI_UNUSED_SLOT(get_Parent);
    MSIMEUI_UNUSED_SLOT(get_RotationAngle);
    MSIMEUI_UNUSED_SLOT(put_RotationAngle);
    MSIMEUI_UNUSED_SLOT(get_RotationAngleInDegrees);
    MSIMEUI_UNUSED_SLOT(put_RotationAngleInDegrees);
    MSIMEUI_UNUSED_SLOT(get_RotationAxis);
    MSIMEUI_UNUSED_SLOT(put_RotationAxis);
    MSIMEUI_UNUSED_SLOT(get_Scale);
    MSIMEUI_UNUSED_SLOT(put_Scale);
    MSIMEUI_UNUSED_SLOT(get_Size);
    virtual HRESULT STDMETHODCALLTYPE put_Size(Vector2 size) = 0;
};

struct IVisualCollection : IInspectable
{
    MSIMEUI_UNUSED_SLOT(get_Count);
    MSIMEUI_UNUSED_SLOT(InsertAbove);
    virtual HRESULT STDMETHODCALLTYPE InsertAtBottom(IVisual *visual) = 0;
    virtual HRESULT STDMETHODCALLTYPE InsertAtTop(IVisual *visual) = 0;
};

struct IContainerVisual : IInspectable
{
    virtual HRESULT STDMETHODCALLTYPE get_Children(IVisualCollection **children) = 0;
};

struct ISpriteVisual : IInspectable
{
    MSIMEUI_UNUSED_SLOT(get_Brush);
    virtual HRESULT STDMETHODCALLTYPE put_Brush(ICompositionBrush *brush) = 0;
};

struct ICompositionTarget : IInspectable
{
    MSIMEUI_UNUSED_SLOT(get_Root);
    virtual HRESULT STDMETHODCALLTYPE put_Root(IVisual *root) = 0;
};

struct ICompositionSurfaceBrush : IInspectable
{
    MSIMEUI_UNUSED_SLOT(get_BitmapInterpolationMode);
    MSIMEUI_UNUSED_SLOT(put_BitmapInterpolationMode);
    MSIMEUI_UNUSED_SLOT(get_HorizontalAlignmentRatio);
    virtual HRESULT STDMETHODCALLTYPE put_HorizontalAlignmentRatio(float ratio) = 0;
    MSIMEUI_UNUSED_SLOT(get_Stretch);
    virtual HRESULT STDMETHODCALLTYPE put_Stretch(int stretch) = 0;
    MSIMEUI_UNUSED_SLOT(get_Surface);
    MSIMEUI_UNUSED_SLOT(put_Surface);
    MSIMEUI_UNUSED_SLOT(get_VerticalAlignmentRatio);
    virtual HRESULT STDMETHODCALLTYPE put_VerticalAlignmentRatio(float ratio) = 0;
};

struct ICompositionRoundedRectangleGeometry : IInspectable
{
    MSIMEUI_UNUSED_SLOT(get_CornerRadius);
    virtual HRESULT STDMETHODCALLTYPE put_CornerRadius(Vector2 radius) = 0;
    MSIMEUI_UNUSED_SLOT(get_Offset);
    MSIMEUI_UNUSED_SLOT(put_Offset);
    MSIMEUI_UNUSED_SLOT(get_Size);
    virtual HRESULT STDMETHODCALLTYPE put_Size(Vector2 size) = 0;
};

struct ICompositionGraphicsDevice : IInspectable
{
    virtual HRESULT STDMETHODCALLTYPE CreateDrawingSurface(Size size, int pixelFormat, int alphaMode,
                                                           ICompositionDrawingSurface **surface) = 0;
};

struct ICompositionEffectBrush : IInspectable
{
    MSIMEUI_UNUSED_SLOT(GetSourceParameter);
    virtual HRESULT STDMETHODCALLTYPE SetSourceParameter(HSTRING name, ICompositionBrush *source) = 0;
};

struct ICompositionEffectFactory : IInspectable
{
    virtual HRESULT STDMETHODCALLTYPE CreateBrush(ICompositionEffectBrush **brush) = 0;
};

struct ICompositionEffectSourceParameterFactory : IInspectable
{
    virtual HRESULT STDMETHODCALLTYPE Create(HSTRING name, ICompositionEffectSourceParameter **parameter) = 0;
};

struct ICompositor : IInspectable
{
    MSIMEUI_UNUSED_SLOT(CreateColorKeyFrameAnimation);
    MSIMEUI_UNUSED_SLOT(CreateColorBrush);
    MSIMEUI_UNUSED_SLOT(CreateColorBrushWithColor);
    virtual HRESULT STDMETHODCALLTYPE CreateContainerVisual(IContainerVisual **visual) = 0;
    MSIMEUI_UNUSED_SLOT(CreateCubicBezierEasingFunction);
    virtual HRESULT STDMETHODCALLTYPE CreateEffectFactory(IGraphicsEffect *effect,
                                                          ICompositionEffectFactory **factory) = 0;
    MSIMEUI_UNUSED_SLOT(CreateEffectFactoryWithProperties);
    MSIMEUI_UNUSED_SLOT(CreateExpressionAnimation);
    MSIMEUI_UNUSED_SLOT(CreateExpressionAnimationWithExpression);
    MSIMEUI_UNUSED_SLOT(CreateInsetClip);
    MSIMEUI_UNUSED_SLOT(CreateInsetClipWithInsets);
    MSIMEUI_UNUSED_SLOT(CreateLinearEasingFunction);
    MSIMEUI_UNUSED_SLOT(CreatePropertySet);
    MSIMEUI_UNUSED_SLOT(CreateQuaternionKeyFrameAnimation);
    MSIMEUI_UNUSED_SLOT(CreateScalarKeyFrameAnimation);
    MSIMEUI_UNUSED_SLOT(CreateScopedBatch);
    virtual HRESULT STDMETHODCALLTYPE CreateSpriteVisual(ISpriteVisual **visual) = 0;
    MSIMEUI_UNUSED_SLOT(CreateSurfaceBrush);
    virtual HRESULT STDMETHODCALLTYPE CreateSurfaceBrushWithSurface(ICompositionSurface *surface,
                                                                    ICompositionSurfaceBrush **brush) = 0;
};

// Windows 10 1607 起：系统的 host backdrop，窗口后面已经模糊好的画面（亚克力的来源）。
struct ICompositor3 : IInspectable
{
    virtual HRESULT STDMETHODCALLTYPE CreateHostBackdropBrush(ICompositionBackdropBrush **brush) = 0;
};

// Windows 10 1903 起：形状几何，这里只要圆角矩形。
struct ICompositor5 : IInspectable
{
    MSIMEUI_UNUSED_SLOT(get_Comment);
    MSIMEUI_UNUSED_SLOT(put_Comment);
    MSIMEUI_UNUSED_SLOT(get_GlobalPlaybackRate);
    MSIMEUI_UNUSED_SLOT(put_GlobalPlaybackRate);
    MSIMEUI_UNUSED_SLOT(CreateBounceScalarAnimation);
    MSIMEUI_UNUSED_SLOT(CreateBounceVector2Animation);
    MSIMEUI_UNUSED_SLOT(CreateBounceVector3Animation);
    MSIMEUI_UNUSED_SLOT(CreateContainerShape);
    MSIMEUI_UNUSED_SLOT(CreateEllipseGeometry);
    MSIMEUI_UNUSED_SLOT(CreateLineGeometry);
    MSIMEUI_UNUSED_SLOT(CreatePathGeometry);
    MSIMEUI_UNUSED_SLOT(CreatePathGeometryWithPath);
    MSIMEUI_UNUSED_SLOT(CreatePathKeyFrameAnimation);
    MSIMEUI_UNUSED_SLOT(CreateRectangleGeometry);
    virtual HRESULT STDMETHODCALLTYPE CreateRoundedRectangleGeometry(ICompositionRoundedRectangleGeometry **geometry) = 0;
};

// Windows 10 1903 起：按几何裁剪视觉对象。
struct ICompositor6 : IInspectable
{
    MSIMEUI_UNUSED_SLOT(CreateGeometricClip);
    virtual HRESULT STDMETHODCALLTYPE CreateGeometricClipWithGeometry(ICompositionGeometry *geometry,
                                                                      ICompositionGeometricClip **clip) = 0;
};

// Windows 11 起：模糊的桌面壁纸（云母的来源）。
struct ICompositorWithBlurredWallpaperBackdropBrush : IInspectable
{
    virtual HRESULT STDMETHODCALLTYPE TryCreateBlurredWallpaperBackdropBrush(ICompositionBackdropBrush **brush) = 0;
};

struct ICompositorDesktopInterop : IUnknown
{
    virtual HRESULT STDMETHODCALLTYPE CreateDesktopWindowTarget(HWND hwnd, BOOL topmost,
                                                                IDesktopWindowTarget **target) = 0;
    virtual HRESULT STDMETHODCALLTYPE EnsureOnThread(DWORD threadId) = 0;
};

struct ICompositorInterop : IUnknown
{
    MSIMEUI_UNUSED_SLOT(CreateCompositionSurfaceForHandle);
    MSIMEUI_UNUSED_SLOT(CreateCompositionSurfaceForSwapChain);
    virtual HRESULT STDMETHODCALLTYPE CreateGraphicsDevice(IUnknown *renderingDevice,
                                                           ICompositionGraphicsDevice **device) = 0;
};

struct ICompositionDrawingSurfaceInterop : IUnknown
{
    virtual HRESULT STDMETHODCALLTYPE BeginDraw(const RECT *updateRect, REFIID iid, void **updateObject,
                                                POINT *updateOffset) = 0;
    virtual HRESULT STDMETHODCALLTYPE EndDraw() = 0;
    virtual HRESULT STDMETHODCALLTYPE Resize(SIZE sizePixels) = 0;
};

struct IPropertyValueStatics : IInspectable
{
    MSIMEUI_UNUSED_SLOT(CreateEmpty);
    MSIMEUI_UNUSED_SLOT(CreateUInt8);
    MSIMEUI_UNUSED_SLOT(CreateInt16);
    MSIMEUI_UNUSED_SLOT(CreateUInt16);
    MSIMEUI_UNUSED_SLOT(CreateInt32);
    virtual HRESULT STDMETHODCALLTYPE CreateUInt32(UINT32 value, IInspectable **boxed) = 0;
    MSIMEUI_UNUSED_SLOT(CreateInt64);
    MSIMEUI_UNUSED_SLOT(CreateUInt64);
    MSIMEUI_UNUSED_SLOT(CreateSingle);
    MSIMEUI_UNUSED_SLOT(CreateDouble);
    MSIMEUI_UNUSED_SLOT(CreateChar16);
    MSIMEUI_UNUSED_SLOT(CreateBoolean);
    MSIMEUI_UNUSED_SLOT(CreateString);
    MSIMEUI_UNUSED_SLOT(CreateInspectable);
    MSIMEUI_UNUSED_SLOT(CreateGuid);
    MSIMEUI_UNUSED_SLOT(CreateDateTime);
    MSIMEUI_UNUSED_SLOT(CreateTimeSpan);
    MSIMEUI_UNUSED_SLOT(CreatePoint);
    MSIMEUI_UNUSED_SLOT(CreateSize);
    MSIMEUI_UNUSED_SLOT(CreateRect);
    MSIMEUI_UNUSED_SLOT(CreateUInt8Array);
    MSIMEUI_UNUSED_SLOT(CreateInt16Array);
    MSIMEUI_UNUSED_SLOT(CreateUInt16Array);
    MSIMEUI_UNUSED_SLOT(CreateInt32Array);
    MSIMEUI_UNUSED_SLOT(CreateUInt32Array);
    MSIMEUI_UNUSED_SLOT(CreateInt64Array);
    MSIMEUI_UNUSED_SLOT(CreateUInt64Array);
    virtual HRESULT STDMETHODCALLTYPE CreateSingleArray(UINT32 count, const FLOAT *values, IInspectable **boxed) = 0;
};

struct IClosable : IInspectable
{
    virtual HRESULT STDMETHODCALLTYPE Close() = 0;
};

#undef MSIMEUI_UNUSED_SLOT

// IID，取自 Windows SDK 元数据。
constexpr GUID IID_ICompositor = {0xb403ca50, 0x7f8c, 0x4e83, {0x98, 0x5f, 0xcc, 0x45, 0x06, 0x00, 0x36, 0xd8}};
constexpr GUID IID_ICompositor3 = {0xc9dd8ef0, 0x6eb1, 0x4e3c, {0xa6, 0x58, 0x67, 0x5d, 0x9c, 0x64, 0xd4, 0xab}};
constexpr GUID IID_ICompositor5 = {0x48ea31ad, 0x7fcd, 0x4076, {0xa7, 0x9c, 0x90, 0xcc, 0x4b, 0x85, 0x2c, 0x9b}};
constexpr GUID IID_ICompositor6 = {0x7a38b2bd, 0xcec8, 0x4eeb, {0x83, 0x0f, 0xd8, 0xd0, 0x7a, 0xed, 0xeb, 0xc3}};
constexpr GUID IID_ICompositorWithBlurredWallpaperBackdropBrush = {
    0x0d8fb190, 0xf122, 0x5b8d, {0x9f, 0xdd, 0x54, 0x3b, 0x0d, 0x8e, 0xb7, 0xf3}};
constexpr GUID IID_ICompositorDesktopInterop = {
    0x29e691fa, 0x4567, 0x4dca, {0xb3, 0x19, 0xd0, 0xf2, 0x07, 0xeb, 0x68, 0x07}};
constexpr GUID IID_ICompositorInterop = {0x25297d5c, 0x3ad4, 0x4c9c, {0xb5, 0xcf, 0xe3, 0x6a, 0x38, 0x51, 0x23, 0x30}};
constexpr GUID IID_ICompositionTarget = {0xa1bea8ba, 0xd726, 0x4663, {0x81, 0x29, 0x6b, 0x5e, 0x79, 0x27, 0xff, 0xa6}};
constexpr GUID IID_IContainerVisual = {0x02f6bc74, 0xed20, 0x4773, {0xaf, 0xe6, 0xd4, 0x9b, 0x4a, 0x93, 0xdb, 0x32}};
constexpr GUID IID_IVisual = {0x117e202d, 0xa859, 0x4c89, {0x87, 0x3b, 0xc2, 0xaa, 0x56, 0x67, 0x88, 0xe3}};
constexpr GUID IID_ISpriteVisual = {0x08e05581, 0x1ad1, 0x4f97, {0x97, 0x57, 0x40, 0x2d, 0x76, 0xe4, 0x23, 0x3b}};
constexpr GUID IID_ICompositionBrush = {0xab0d7608, 0x30c0, 0x40e9, {0xb5, 0x68, 0xb6, 0x0a, 0x6b, 0xd1, 0xfb, 0x46}};
constexpr GUID IID_ICompositionClip = {0x1ccd2a52, 0xcfc7, 0x4ace, {0x99, 0x83, 0x14, 0x6b, 0xb8, 0xeb, 0x6a, 0x3c}};
constexpr GUID IID_ICompositionGeometry = {0xe985217c, 0x6a17, 0x4207, {0xab, 0xd8, 0x5f, 0xd3, 0xdd, 0x61, 0x2a, 0x9d}};
constexpr GUID IID_ICompositionSurface = {0x1527540d, 0x42c7, 0x47a6, {0xa4, 0x08, 0x66, 0x8f, 0x79, 0xa9, 0x0d, 0xfb}};
constexpr GUID IID_ICompositionDrawingSurfaceInterop = {
    0xfd04e6e3, 0xfe0c, 0x4c3c, {0xab, 0x19, 0xa0, 0x76, 0x01, 0xa5, 0x76, 0xee}};
constexpr GUID IID_ICompositionEffectSourceParameterFactory = {
    0xb3d9f276, 0xaba3, 0x4724, {0xac, 0xf3, 0xd0, 0x39, 0x74, 0x64, 0xdb, 0x1c}};
constexpr GUID IID_IGraphicsEffect = {0xcb51c0ce, 0x8fe6, 0x4636, {0xb2, 0x02, 0x86, 0x1f, 0xaa, 0x07, 0xd8, 0xf3}};
constexpr GUID IID_IGraphicsEffectSource = {0x2d8f9ddc, 0x4339, 0x4eb9, {0x92, 0x16, 0xf9, 0xde, 0xb7, 0x56, 0x58, 0xa2}};
constexpr GUID IID_IGraphicsEffectD2D1Interop = {
    0x2fc57384, 0xa068, 0x44d7, {0xa3, 0x31, 0x30, 0x98, 0x2f, 0xcf, 0x71, 0x77}};
constexpr GUID IID_IPropertyValue = {0x4bd682dd, 0x7554, 0x40e9, {0x9a, 0x9b, 0x82, 0x65, 0x4e, 0xde, 0x7e, 0x62}};
constexpr GUID IID_IPropertyValueStatics = {0x629bdbc8, 0xd932, 0x4ff4, {0x96, 0xb9, 0x8d, 0x96, 0xc5, 0xc1, 0xe8, 0x58}};
constexpr GUID IID_IClosable = {0x30d5a829, 0x7fa4, 0x4026, {0x83, 0xbb, 0xd7, 0x5b, 0xae, 0x4e, 0xa9, 0x9e}};
// Direct2D 内置效果：纯色填充与混合（d2d1effects.h 的 CLSID_D2D1Flood、CLSID_D2D1Blend）。
constexpr GUID CLSID_Flood = {0x61c23c20, 0xae69, 0x4d8e, {0x94, 0xcf, 0x50, 0x07, 0x8d, 0xf6, 0x38, 0xf2}};
constexpr GUID CLSID_Blend = {0x81c5b77b, 0x13f8, 0x4cdd, {0xad, 0x20, 0xc8, 0x90, 0x54, 0x7a, 0xc6, 0x5d}};
// D2D1_BLEND_MODE_COLOR 与 D2D1_BLEND_MODE_LUMINOSITY。
constexpr UINT32 kBlendModeColor = 22;
constexpr UINT32 kBlendModeLuminosity = 23;
} // namespace msimeui::composition
