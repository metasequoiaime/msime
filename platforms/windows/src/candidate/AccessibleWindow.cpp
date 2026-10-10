#include "AccessibleWindow.h"
#include <ole2.h>
#include <oleauto.h>
#include <uiautomation.h>
#include <atomic>
#include <map>
#include <mutex>
#include <new>
#include <string>
#include <utility>

namespace msime::windows {
class RootProvider;
class ElementProvider;

// 窗口和它交出去的提供者共用的状态。提供者各持有一份 shared_ptr；这里反过来持有根和每个子元素提供者的一个引用，好在 disconnect 时逐个断开，断开时这个环一并拆掉。
struct AccessibleShared {
  std::mutex mutex;
  HWND window = nullptr;
  bool connected = true;
  AccessibleTree tree;
  uint64_t serial = 0;
  RootProvider *root = nullptr;
  std::map<int, ElementProvider *> elements;
};

namespace {
BSTR utf8_bstr(const std::string &text) {
  if (text.empty() || text.size() > 65536)
    return nullptr;
  const int count = MultiByteToWideChar(CP_UTF8, 0, text.data(),
                                        static_cast<int>(text.size()), nullptr, 0);
  if (count <= 0)
    return nullptr;
  BSTR result = SysAllocStringLen(nullptr, static_cast<UINT>(count));
  if (!result)
    return nullptr;
  if (MultiByteToWideChar(CP_UTF8, 0, text.data(), static_cast<int>(text.size()), result,
                          count) != count) {
    SysFreeString(result);
    return nullptr;
  }
  return result;
}
// 空文字留成 VT_EMPTY，由 UI Automation 取默认值（根元素的话取窗口本身的）。
void put_string(VARIANT *out, const std::string &text) {
  if (BSTR value = utf8_bstr(text)) {
    out->vt = VT_BSTR;
    out->bstrVal = value;
  }
}
void put_bool(VARIANT *out, bool value) {
  out->vt = VT_BOOL;
  out->boolVal = value ? VARIANT_TRUE : VARIANT_FALSE;
}
void put_int(VARIANT *out, int value) {
  out->vt = VT_I4;
  out->lVal = value;
}
int control_type(AccessibleRole role) {
  switch (role) {
  case AccessibleRole::Button:
    return UIA_ButtonControlTypeId;
  case AccessibleRole::ListItem:
    return UIA_ListItemControlTypeId;
  case AccessibleRole::MenuItem:
    return UIA_MenuItemControlTypeId;
  case AccessibleRole::Image:
    return UIA_ImageControlTypeId;
  case AccessibleRole::Separator:
    return UIA_SeparatorControlTypeId;
  case AccessibleRole::Text:
    break;
  }
  return UIA_TextControlTypeId;
}
int control_type(AccessibleContainer container) {
  switch (container) {
  case AccessibleContainer::ToolBar:
    return UIA_ToolBarControlTypeId;
  case AccessibleContainer::Menu:
    return UIA_MenuControlTypeId;
  case AccessibleContainer::List:
    break;
  }
  return UIA_ListControlTypeId;
}
// 只在这次换算里切换调用线程的 DPI 上下文，出来时还原。
struct ThreadDpi {
  DPI_AWARENESS_CONTEXT previous;
  explicit ThreadDpi(DPI_AWARENESS_CONTEXT context)
      : previous(context ? SetThreadDpiAwarenessContext(context) : nullptr) {}
  ~ThreadDpi() {
    if (previous)
      SetThreadDpiAwarenessContext(previous);
  }
  ThreadDpi(const ThreadDpi &) = delete;
  ThreadDpi &operator=(const ThreadDpi &) = delete;
};
// 客户区大小按窗口自己的坐标系量（元素坐标就在这个坐标系里），窗口的屏幕位置按每显示器感知量（读屏要物理像素）。这些窗口都是无边框的 WS_POPUP，客户区左上角就是窗口左上角。
std::optional<AccessibleFrame> window_frame(HWND window) {
  if (!window || !IsWindow(window))
    return std::nullopt;
  RECT client{};
  {
    ThreadDpi scope(GetWindowDpiAwarenessContext(window));
    if (!GetClientRect(window, &client))
      return std::nullopt;
  }
  RECT screen{};
  {
    ThreadDpi scope(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    if (!GetWindowRect(window, &screen))
      return std::nullopt;
  }
  return accessible_frame(screen.left, screen.top, screen.right - screen.left,
                          screen.bottom - screen.top, client.right - client.left,
                          client.bottom - client.top);
}

// 以下两个函数要求调用方持有 shared.mutex。返回的提供者已经 AddRef；内存不够时返回空。
RootProvider *root_locked(const std::shared_ptr<AccessibleShared> &shared);
ElementProvider *element_locked(const std::shared_ptr<AccessibleShared> &shared, int id);
} // namespace

// 窗口本身：片段根。它的位置、RuntimeId 都由窗口的宿主提供者（UiaHostProviderFromHwnd）给出，这里只报告控件类型和名字，以及子元素。
class RootProvider final : public IRawElementProviderSimple,
                           public IRawElementProviderFragment,
                           public IRawElementProviderFragmentRoot {
public:
  explicit RootProvider(std::shared_ptr<AccessibleShared> shared) : shared_(std::move(shared)) {}
  RootProvider(const RootProvider &) = delete;
  RootProvider &operator=(const RootProvider &) = delete;

  HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void **out) override {
    if (!out)
      return E_POINTER;
    if (iid == __uuidof(IUnknown) || iid == __uuidof(IRawElementProviderSimple))
      *out = static_cast<IRawElementProviderSimple *>(this);
    else if (iid == __uuidof(IRawElementProviderFragment))
      *out = static_cast<IRawElementProviderFragment *>(this);
    else if (iid == __uuidof(IRawElementProviderFragmentRoot))
      *out = static_cast<IRawElementProviderFragmentRoot *>(this);
    else {
      *out = nullptr;
      return E_NOINTERFACE;
    }
    AddRef();
    return S_OK;
  }
  ULONG STDMETHODCALLTYPE AddRef() override { return ++references_; }
  ULONG STDMETHODCALLTYPE Release() override {
    const ULONG left = --references_;
    if (!left)
      delete this;
    return left;
  }

  // IRawElementProviderSimple
  HRESULT STDMETHODCALLTYPE get_ProviderOptions(ProviderOptions *out) override {
    if (!out)
      return E_POINTER;
    *out = ProviderOptions_ServerSideProvider;
    return S_OK;
  }
  HRESULT STDMETHODCALLTYPE GetPatternProvider(PATTERNID, IUnknown **out) override {
    if (!out)
      return E_POINTER;
    *out = nullptr;
    return S_OK;
  }
  HRESULT STDMETHODCALLTYPE GetPropertyValue(PROPERTYID property, VARIANT *out) override {
    if (!out)
      return E_POINTER;
    VariantInit(out);
    AccessibleContainer container = AccessibleContainer::List;
    std::string name;
    {
      std::lock_guard<std::mutex> lock(shared_->mutex);
      if (!shared_->connected)
        return UIA_E_ELEMENTNOTAVAILABLE;
      container = shared_->tree.container;
      name = shared_->tree.name;
    }
    switch (property) {
    case UIA_ControlTypePropertyId:
      put_int(out, control_type(container));
      break;
    case UIA_NamePropertyId:
      put_string(out, name);
      break;
    // 这些窗口从不接受焦点（WS_EX_NOACTIVATE），焦点留在用户正在打字的应用里。
    case UIA_IsKeyboardFocusablePropertyId:
      put_bool(out, false);
      break;
    default:
      break;
    }
    return S_OK;
  }
  HRESULT STDMETHODCALLTYPE get_HostRawElementProvider(IRawElementProviderSimple **out) override {
    if (!out)
      return E_POINTER;
    *out = nullptr;
    HWND window = nullptr;
    {
      std::lock_guard<std::mutex> lock(shared_->mutex);
      if (!shared_->connected)
        return UIA_E_ELEMENTNOTAVAILABLE;
      window = shared_->window;
    }
    return UiaHostProviderFromHwnd(window, out);
  }

  // IRawElementProviderFragment
  HRESULT STDMETHODCALLTYPE Navigate(NavigateDirection direction,
                                     IRawElementProviderFragment **out) override {
    if (!out)
      return E_POINTER;
    *out = nullptr;
    if (direction != NavigateDirection_FirstChild && direction != NavigateDirection_LastChild)
      return S_OK;
    std::lock_guard<std::mutex> lock(shared_->mutex);
    if (!shared_->connected)
      return UIA_E_ELEMENTNOTAVAILABLE;
    const auto &elements = shared_->tree.elements;
    if (elements.empty())
      return S_OK;
    const int id = direction == NavigateDirection_FirstChild ? elements.front().id
                                                             : elements.back().id;
    auto *element = element_locked(shared_, id);
    if (!element)
      return E_OUTOFMEMORY;
    *out = fragment(element);
    return S_OK;
  }
  HRESULT STDMETHODCALLTYPE GetRuntimeId(SAFEARRAY **out) override {
    if (!out)
      return E_POINTER;
    // 窗口承载的根不报告自己的 RuntimeId，由宿主提供者按窗口句柄给出。
    *out = nullptr;
    return S_OK;
  }
  HRESULT STDMETHODCALLTYPE get_BoundingRectangle(UiaRect *out) override {
    if (!out)
      return E_POINTER;
    *out = UiaRect{0.0, 0.0, 0.0, 0.0};
    return S_OK;
  }
  HRESULT STDMETHODCALLTYPE GetEmbeddedFragmentRoots(SAFEARRAY **out) override {
    if (!out)
      return E_POINTER;
    *out = nullptr;
    return S_OK;
  }
  HRESULT STDMETHODCALLTYPE SetFocus() override { return S_OK; }
  HRESULT STDMETHODCALLTYPE get_FragmentRoot(IRawElementProviderFragmentRoot **out) override {
    if (!out)
      return E_POINTER;
    *out = static_cast<IRawElementProviderFragmentRoot *>(this);
    AddRef();
    return S_OK;
  }

  // IRawElementProviderFragmentRoot
  HRESULT STDMETHODCALLTYPE ElementProviderFromPoint(double x, double y,
                                                     IRawElementProviderFragment **out) override {
    if (!out)
      return E_POINTER;
    *out = nullptr;
    HWND window = nullptr;
    {
      std::lock_guard<std::mutex> lock(shared_->mutex);
      if (!shared_->connected)
        return UIA_E_ELEMENTNOTAVAILABLE;
      window = shared_->window;
    }
    // 窗口位置在锁外量：它可能要等窗口所在的线程。
    const auto frame = window_frame(window);
    if (!frame)
      return S_OK;
    std::lock_guard<std::mutex> lock(shared_->mutex);
    if (!shared_->connected)
      return UIA_E_ELEMENTNOTAVAILABLE;
    const auto index = accessible_hit(shared_->tree, *frame, x, y);
    if (!index)
      return S_OK;
    auto *element = element_locked(shared_, shared_->tree.elements[*index].id);
    if (!element)
      return E_OUTOFMEMORY;
    *out = fragment(element);
    return S_OK;
  }
  HRESULT STDMETHODCALLTYPE GetFocus(IRawElementProviderFragment **out) override {
    if (!out)
      return E_POINTER;
    *out = nullptr;
    std::lock_guard<std::mutex> lock(shared_->mutex);
    if (!shared_->connected)
      return UIA_E_ELEMENTNOTAVAILABLE;
    const auto *focused = accessible_focus(shared_->tree);
    if (!focused)
      return S_OK;
    auto *element = element_locked(shared_, focused->id);
    if (!element)
      return E_OUTOFMEMORY;
    *out = fragment(element);
    return S_OK;
  }

private:
  ~RootProvider() = default;
  static IRawElementProviderFragment *fragment(ElementProvider *element);
  std::shared_ptr<AccessibleShared> shared_;
  std::atomic<ULONG> references_{1};
};

// 树里的一个元素。它只记着元素 id，每次被问都到当前的树里去找；找不到（树已经换了）或窗口已经断开时回答「元素不可用」。
class ElementProvider final : public IRawElementProviderSimple,
                              public IRawElementProviderFragment,
                              public IInvokeProvider,
                              public IToggleProvider,
                              public IValueProvider {
public:
  ElementProvider(std::shared_ptr<AccessibleShared> shared, int id)
      : shared_(std::move(shared)), id_(id) {}
  ElementProvider(const ElementProvider &) = delete;
  ElementProvider &operator=(const ElementProvider &) = delete;

  HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void **out) override {
    if (!out)
      return E_POINTER;
    if (iid == __uuidof(IUnknown) || iid == __uuidof(IRawElementProviderSimple))
      *out = static_cast<IRawElementProviderSimple *>(this);
    else if (iid == __uuidof(IRawElementProviderFragment))
      *out = static_cast<IRawElementProviderFragment *>(this);
    else if (iid == __uuidof(IInvokeProvider))
      *out = static_cast<IInvokeProvider *>(this);
    else if (iid == __uuidof(IToggleProvider))
      *out = static_cast<IToggleProvider *>(this);
    else if (iid == __uuidof(IValueProvider))
      *out = static_cast<IValueProvider *>(this);
    else {
      *out = nullptr;
      return E_NOINTERFACE;
    }
    AddRef();
    return S_OK;
  }
  ULONG STDMETHODCALLTYPE AddRef() override { return ++references_; }
  ULONG STDMETHODCALLTYPE Release() override {
    const ULONG left = --references_;
    if (!left)
      delete this;
    return left;
  }

  // IRawElementProviderSimple
  HRESULT STDMETHODCALLTYPE get_ProviderOptions(ProviderOptions *out) override {
    if (!out)
      return E_POINTER;
    *out = ProviderOptions_ServerSideProvider;
    return S_OK;
  }
  HRESULT STDMETHODCALLTYPE GetPatternProvider(PATTERNID pattern, IUnknown **out) override {
    if (!out)
      return E_POINTER;
    *out = nullptr;
    AccessibleElement element;
    if (!read(element))
      return UIA_E_ELEMENTNOTAVAILABLE;
    // 按钮和菜单项总带 Invoke（不可用时执行会被拒绝），候选行只在能点时带；勾选状态和值各自有了才带对应的模式。
    const bool invoke = element.role == AccessibleRole::Button ||
                        element.role == AccessibleRole::MenuItem ||
                        (element.role == AccessibleRole::ListItem && element.invokable);
    if (pattern == UIA_InvokePatternId && invoke)
      *out = static_cast<IInvokeProvider *>(this);
    else if (pattern == UIA_TogglePatternId && element.checked)
      *out = static_cast<IToggleProvider *>(this);
    else if (pattern == UIA_ValuePatternId && element.value)
      *out = static_cast<IValueProvider *>(this);
    if (*out)
      AddRef();
    return S_OK;
  }
  HRESULT STDMETHODCALLTYPE GetPropertyValue(PROPERTYID property, VARIANT *out) override {
    if (!out)
      return E_POINTER;
    VariantInit(out);
    AccessibleElement element;
    if (!read(element))
      return UIA_E_ELEMENTNOTAVAILABLE;
    switch (property) {
    case UIA_ControlTypePropertyId:
      put_int(out, control_type(element.role));
      break;
    case UIA_NamePropertyId:
      put_string(out, element.name);
      break;
    case UIA_HelpTextPropertyId:
      put_string(out, element.help);
      break;
    case UIA_AutomationIdPropertyId:
      put_string(out, element.automation_id);
      break;
    case UIA_AcceleratorKeyPropertyId:
      put_string(out, element.accelerator);
      break;
    case UIA_IsEnabledPropertyId:
      put_bool(out, element.enabled);
      break;
    // 只有托盘卡片的菜单项会报告键盘焦点（键盘导航停在上面时）；按钮和候选行用指针或读屏执行，不进键盘焦点。
    case UIA_IsKeyboardFocusablePropertyId:
      put_bool(out, element.role == AccessibleRole::MenuItem && element.enabled);
      break;
    case UIA_HasKeyboardFocusPropertyId:
      put_bool(out, element.focused);
      break;
    case UIA_IsContentElementPropertyId:
      put_bool(out, element.role != AccessibleRole::Separator);
      break;
    default:
      break;
    }
    return S_OK;
  }
  HRESULT STDMETHODCALLTYPE get_HostRawElementProvider(IRawElementProviderSimple **out) override {
    if (!out)
      return E_POINTER;
    *out = nullptr;
    return S_OK;
  }

  // IRawElementProviderFragment
  HRESULT STDMETHODCALLTYPE Navigate(NavigateDirection direction,
                                     IRawElementProviderFragment **out) override {
    if (!out)
      return E_POINTER;
    *out = nullptr;
    std::lock_guard<std::mutex> lock(shared_->mutex);
    if (!shared_->connected)
      return UIA_E_ELEMENTNOTAVAILABLE;
    const auto &elements = shared_->tree.elements;
    size_t index = 0;
    while (index < elements.size() && elements[index].id != id_)
      ++index;
    if (index == elements.size())
      return UIA_E_ELEMENTNOTAVAILABLE;
    if (direction == NavigateDirection_Parent) {
      auto *root = root_locked(shared_);
      if (!root)
        return E_OUTOFMEMORY;
      *out = static_cast<IRawElementProviderFragment *>(root_fragment(root));
      return S_OK;
    }
    std::optional<size_t> neighbour;
    if (direction == NavigateDirection_NextSibling && index + 1 < elements.size())
      neighbour = index + 1;
    else if (direction == NavigateDirection_PreviousSibling && index > 0)
      neighbour = index - 1;
    if (!neighbour)
      return S_OK;
    auto *element = element_locked(shared_, elements[*neighbour].id);
    if (!element)
      return E_OUTOFMEMORY;
    *out = static_cast<IRawElementProviderFragment *>(element);
    return S_OK;
  }
  HRESULT STDMETHODCALLTYPE GetRuntimeId(SAFEARRAY **out) override {
    if (!out)
      return E_POINTER;
    *out = nullptr;
    // 窗口的 RuntimeId 后面接上元素 id；同一个元素在前后两棵树里 id 相同，读屏看到的也就是同一个元素。
    SAFEARRAY *array = SafeArrayCreateVector(VT_I4, 0, 2);
    if (!array)
      return E_OUTOFMEMORY;
    LONG position = 0;
    int value = UiaAppendRuntimeId;
    SafeArrayPutElement(array, &position, &value);
    position = 1;
    value = id_;
    SafeArrayPutElement(array, &position, &value);
    *out = array;
    return S_OK;
  }
  HRESULT STDMETHODCALLTYPE get_BoundingRectangle(UiaRect *out) override {
    if (!out)
      return E_POINTER;
    *out = UiaRect{0.0, 0.0, 0.0, 0.0};
    AccessibleElement element;
    HWND window = nullptr;
    if (!read(element, &window))
      return UIA_E_ELEMENTNOTAVAILABLE;
    const auto frame = window_frame(window);
    if (!frame)
      return S_OK;
    const auto box = accessible_screen_bounds(element.bounds, *frame);
    *out = UiaRect{box.left, box.top, box.right - box.left, box.bottom - box.top};
    return S_OK;
  }
  HRESULT STDMETHODCALLTYPE GetEmbeddedFragmentRoots(SAFEARRAY **out) override {
    if (!out)
      return E_POINTER;
    *out = nullptr;
    return S_OK;
  }
  // 窗口不接受焦点，读屏要求聚焦时什么也不做。
  HRESULT STDMETHODCALLTYPE SetFocus() override { return S_OK; }
  HRESULT STDMETHODCALLTYPE get_FragmentRoot(IRawElementProviderFragmentRoot **out) override {
    if (!out)
      return E_POINTER;
    *out = nullptr;
    std::lock_guard<std::mutex> lock(shared_->mutex);
    if (!shared_->connected)
      return UIA_E_ELEMENTNOTAVAILABLE;
    auto *root = root_locked(shared_);
    if (!root)
      return E_OUTOFMEMORY;
    *out = root_fragment_root(root);
    return S_OK;
  }

  // IInvokeProvider
  HRESULT STDMETHODCALLTYPE Invoke() override { return post(); }

  // IToggleProvider：切换就是点它一下，结果由窗口按新状态重新发布。
  HRESULT STDMETHODCALLTYPE Toggle() override { return post(); }
  HRESULT STDMETHODCALLTYPE get_ToggleState(ToggleState *out) override {
    if (!out)
      return E_POINTER;
    AccessibleElement element;
    if (!read(element))
      return UIA_E_ELEMENTNOTAVAILABLE;
    *out = element.checked.value_or(false) ? ToggleState_On : ToggleState_Off;
    return S_OK;
  }

  // IValueProvider：只读。
  HRESULT STDMETHODCALLTYPE SetValue(LPCWSTR) override { return UIA_E_ELEMENTNOTENABLED; }
  HRESULT STDMETHODCALLTYPE get_Value(BSTR *out) override {
    if (!out)
      return E_POINTER;
    *out = nullptr;
    AccessibleElement element;
    if (!read(element))
      return UIA_E_ELEMENTNOTAVAILABLE;
    *out = utf8_bstr(element.value.value_or(std::string()));
    if (!*out)
      *out = SysAllocString(L"");
    return *out ? S_OK : E_OUTOFMEMORY;
  }
  HRESULT STDMETHODCALLTYPE get_IsReadOnly(BOOL *out) override {
    if (!out)
      return E_POINTER;
    *out = TRUE;
    return S_OK;
  }

private:
  ~ElementProvider() = default;
  bool read(AccessibleElement &element, HWND *window = nullptr, uint64_t *serial = nullptr) {
    std::lock_guard<std::mutex> lock(shared_->mutex);
    if (!shared_->connected)
      return false;
    const auto *found = accessible_element(shared_->tree, id_);
    if (!found)
      return false;
    element = *found;
    if (window)
      *window = shared_->window;
    if (serial)
      *serial = shared_->serial;
    return true;
  }
  // 执行请求投递回窗口，在窗口的 UI 线程上按序号核对后再执行，和鼠标点击走同一条路。
  HRESULT post() {
    AccessibleElement element;
    HWND window = nullptr;
    uint64_t serial = 0;
    if (!read(element, &window, &serial))
      return UIA_E_ELEMENTNOTAVAILABLE;
    if (!element.enabled || !element.invokable)
      return UIA_E_ELEMENTNOTENABLED;
    if (!PostMessageW(window, accessible_invoke_message, static_cast<WPARAM>(element.id),
                      static_cast<LPARAM>(serial)))
      return HRESULT_FROM_WIN32(GetLastError());
    return S_OK;
  }
  static IRawElementProviderFragment *root_fragment(RootProvider *root) {
    return static_cast<IRawElementProviderFragment *>(root);
  }
  static IRawElementProviderFragmentRoot *root_fragment_root(RootProvider *root) {
    return static_cast<IRawElementProviderFragmentRoot *>(root);
  }
  std::shared_ptr<AccessibleShared> shared_;
  int id_;
  std::atomic<ULONG> references_{1};
};

IRawElementProviderFragment *RootProvider::fragment(ElementProvider *element) {
  return static_cast<IRawElementProviderFragment *>(element);
}

namespace {
RootProvider *root_locked(const std::shared_ptr<AccessibleShared> &shared) {
  if (!shared->root) {
    shared->root = new (std::nothrow) RootProvider(shared);
    if (!shared->root)
      return nullptr;
  }
  shared->root->AddRef();
  return shared->root;
}
ElementProvider *element_locked(const std::shared_ptr<AccessibleShared> &shared, int id) {
  const auto found = shared->elements.find(id);
  if (found != shared->elements.end()) {
    found->second->AddRef();
    return found->second;
  }
  auto *element = new (std::nothrow) ElementProvider(shared, id);
  if (!element)
    return nullptr;
  // 元素 id 在每个窗口里是一小组固定的编号（候选行 1 到 9、按钮编号、托盘行号），这张表不会无限增长。
  try {
    shared->elements.emplace(id, element);
  } catch (const std::bad_alloc &) {
    element->Release();
    return nullptr;
  }
  element->AddRef();
  return element;
}
// 原样转成基类指针，事件函数要的是 IRawElementProviderSimple。
IRawElementProviderSimple *simple(RootProvider *root) {
  return static_cast<IRawElementProviderSimple *>(root);
}
IRawElementProviderSimple *simple(ElementProvider *element) {
  return static_cast<IRawElementProviderSimple *>(element);
}
} // namespace

AccessibleWindow::AccessibleWindow(HWND window)
    : window_(window), shared_(std::make_shared<AccessibleShared>()) {
  shared_->window = window;
}
AccessibleWindow::~AccessibleWindow() { disconnect(); }

std::optional<LRESULT> AccessibleWindow::answer(WPARAM wparam, LPARAM lparam) noexcept {
  if (static_cast<LONG>(lparam) != static_cast<LONG>(UiaRootObjectId))
    return std::nullopt;
  RootProvider *root = nullptr;
  {
    std::lock_guard<std::mutex> lock(shared_->mutex);
    if (!shared_->connected)
      return std::nullopt;
    root = root_locked(shared_);
  }
  if (!root)
    return std::nullopt;
  const LRESULT result = UiaReturnRawElementProvider(window_, wparam, lparam, simple(root));
  root->Release();
  return result;
}

void AccessibleWindow::publish(AccessibleTree tree) noexcept {
  RootProvider *root = nullptr;
  {
    std::lock_guard<std::mutex> lock(shared_->mutex);
    if (!shared_->connected || tree == shared_->tree)
      return;
    shared_->tree = std::move(tree);
    ++shared_->serial;
    // 还没有读屏要过这个窗口时没有根，也就没有谁需要知道树变了。
    if (shared_->root && UiaClientsAreListening()) {
      root = shared_->root;
      root->AddRef();
    }
  }
  if (!root)
    return;
  UiaRaiseStructureChangedEvent(simple(root), StructureChangeType_ChildrenInvalidated, nullptr, 0);
  root->Release();
}

bool AccessibleWindow::current(LPARAM token) const noexcept {
  std::lock_guard<std::mutex> lock(shared_->mutex);
  return shared_->connected && static_cast<LPARAM>(shared_->serial) == token;
}

void AccessibleWindow::focus(int id) noexcept {
  if (!UiaClientsAreListening())
    return;
  ElementProvider *element = nullptr;
  {
    std::lock_guard<std::mutex> lock(shared_->mutex);
    if (!shared_->connected)
      return;
    const auto *found = accessible_element(shared_->tree, id);
    if (!found || !found->focused)
      return;
    element = element_locked(shared_, id);
  }
  if (!element)
    return;
  UiaRaiseAutomationEvent(simple(element), UIA_AutomationFocusChangedEventId);
  element->Release();
}

namespace {
void raise_root(const std::shared_ptr<AccessibleShared> &shared, EVENTID event) {
  if (!UiaClientsAreListening())
    return;
  RootProvider *root = nullptr;
  {
    std::lock_guard<std::mutex> lock(shared->mutex);
    if (!shared->connected)
      return;
    root = root_locked(shared);
  }
  if (!root)
    return;
  UiaRaiseAutomationEvent(simple(root), event);
  root->Release();
}
} // namespace

void AccessibleWindow::menu_opened() noexcept { raise_root(shared_, UIA_MenuOpenedEventId); }
void AccessibleWindow::menu_closed() noexcept { raise_root(shared_, UIA_MenuClosedEventId); }

void AccessibleWindow::disconnect() noexcept {
  RootProvider *root = nullptr;
  std::map<int, ElementProvider *> elements;
  {
    std::lock_guard<std::mutex> lock(shared_->mutex);
    if (!shared_->connected)
      return;
    shared_->connected = false;
    shared_->tree = {};
    root = std::exchange(shared_->root, nullptr);
    elements.swap(shared_->elements);
  }
  // 先断开交出去的每个提供者，再告诉 UI Automation 这个窗口的提供者全部作废。
  for (auto &entry : elements) {
    UiaDisconnectProvider(simple(entry.second));
    entry.second->Release();
  }
  if (root) {
    UiaDisconnectProvider(simple(root));
    root->Release();
  }
  if (window_ && IsWindow(window_))
    UiaReturnRawElementProvider(window_, 0, 0, nullptr);
}
} // namespace msime::windows
