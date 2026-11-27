import UIKit
import WebKit

/// 键盘自己录音时的声音来源；测试里换成合成的输入。回调都在主线程。
@MainActor
protocol KeyboardAudioInput: AnyObject {
  /// 麦克风已经打开，声音马上就来。
  var onStarted: (() -> Void)? { get set }
  /// 一段 16 kHz 单声道 16 位小端 PCM。
  var onAudio: ((Data) -> Void)? { get set }
  /// 录音自己停了（音轨结束、被打断、页面没了）或没能开始，带上原因；`denied` 为真时是没有麦克风权限。
  var onStopped: ((_ reason: String, _ denied: Bool) -> Void)? { get set }
  func start()
  func stop()
}

/// 键盘扩展里的麦克风：在隐藏的 `WKWebView` 里跑 `getUserMedia`。键盘扩展自己的音频输入用不了（AVAudioEngine 在扩展里起不来，或被宿主 App 打断），WebKit 却在它自己的进程里、按主 App 的麦克风权限采集，键盘就能不打开 App 录音。这个网页只用来采集声音，不显示任何界面。
///
/// 需要键盘开着「允许完全访问」，主 App 也要先拿到过麦克风权限；拿不到时 `onStopped` 报 `denied`。
@MainActor
final class KeyboardMicrophone: NSObject, KeyboardAudioInput {
  static let sampleRate = 16_000

  var onStarted: (() -> Void)?
  var onAudio: ((Data) -> Void)?
  var onStopped: ((_ reason: String, _ denied: Bool) -> Void)?

  private let webView: WKWebView
  private var ready = false
  /// 网页还没载完时就要开始：载完立刻开始。
  private var startPending = false
  private var capturing = false
  private var heartbeat: Timer?

  override init() {
    let configuration = WKWebViewConfiguration()
    configuration.allowsInlineMediaPlayback = true
    configuration.mediaTypesRequiringUserActionForPlayback = []
    configuration.websiteDataStore = .nonPersistent()
    webView = WKWebView(frame: CGRect(x: 0, y: 0, width: 1, height: 1), configuration: configuration)
    super.init()
    configuration.userContentController.add(WeakScriptHandler(self), name: "capture")
    webView.uiDelegate = self
    webView.navigationDelegate = self
    webView.alpha = 0.01
    webView.isUserInteractionEnabled = false
    webView.accessibilityElementsHidden = true
  }

  /// WebKit 只给在视图层级里的网页采集声音，所以要挂在键盘的视图上。
  func attach(to view: UIView) {
    guard webView.superview !== view else { return }
    view.addSubview(webView)
    view.sendSubviewToBack(webView)
    load()
  }

  func start() {
    guard !capturing else { return }
    capturing = true
    guard ready else {
      startPending = true
      return
    }
    begin()
  }

  func stop() {
    startPending = false
    guard capturing else { return }
    webView.evaluateJavaScript("stopCapture(); 0")
    end()
  }

  private func begin() {
    startPending = false
    // 网页收不到这个心跳（键盘收起或崩溃）就自己关掉麦克风，不会一直开着。
    heartbeat = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
      MainActor.assumeIsolated { self?.webView.evaluateJavaScript("captureHeartbeat()") }
    }
    webView.evaluateJavaScript("startCapture(); 0") { [weak self] _, error in
      if let error { self?.interrupt("start: \(error.localizedDescription)", denied: false) }
    }
  }

  private func load() {
    ready = false
    // https 的基础地址让网页处在 getUserMedia 要求的安全上下文里；这个地址不会被访问。
    webView.loadHTMLString(Self.page, baseURL: URL(string: "https://keyboard-microphone.invalid/"))
  }

  private func end() {
    heartbeat?.invalidate()
    heartbeat = nil
    capturing = false
  }

  private func interrupt(_ reason: String, denied: Bool) {
    guard capturing else { return }
    startPending = false
    end()
    onStopped?(reason, denied)
  }

  fileprivate func handle(_ body: Any) {
    guard let message = body as? [String: Any], let type = message["type"] as? String else { return }
    switch type {
    case "ready":
      ready = true
      if startPending { begin() }
    case "started":
      guard capturing else { return }
      onStarted?()
    case "audio":
      guard capturing, let base64 = message["data"] as? String, let data = Data(base64Encoded: base64) else { return }
      onAudio?(data)
    case "interrupted", "failed":
      let reason = message["reason"] as? String ?? "unknown"
      interrupt("\(type): \(reason)", denied: reason.hasPrefix("NotAllowedError"))
    default:
      break
    }
  }

  /// 采集、降到 16 kHz 单声道 16 位、按块 Base64 发回原生一侧。`navigator.audioSession` 设成录音类型，结束时恢复原来的类型。
  private static let page = """
  <!doctype html><html><body><script>
  const targetRate = \(sampleRate);
  const post = message => window.webkit.messageHandlers.capture.postMessage(message);
  const encode = samples => {
    const bytes = new Uint8Array(samples.buffer, 0, samples.length * 2);
    let binary = '';
    for (let i = 0; i < bytes.length; i += 0x8000) binary += String.fromCharCode.apply(null, bytes.subarray(i, i + 0x8000));
    return btoa(binary);
  };
  let lastHeartbeat = performance.now();
  window.captureHeartbeat = () => { lastHeartbeat = performance.now(); };
  let stopCapture = () => {};
  window.stopCapture = () => stopCapture();
  window.startCapture = async () => {
    let stopping = false, stream, context, source, processor, sink, watchdog;
    const audioSession = navigator.audioSession;
    const previousType = audioSession?.type ?? 'auto';
    const cleanup = () => {
      if (stopping) return;
      stopping = true;
      clearInterval(watchdog);
      if (processor) { processor.onaudioprocess = null; processor.disconnect(); }
      source?.disconnect();
      sink?.disconnect();
      stream?.getTracks().forEach(track => track.stop());
      context?.close().catch(() => {});
      try { if (audioSession) audioSession.type = previousType; } catch (_) {}
    };
    const interrupted = reason => { if (!stopping) { cleanup(); post({ type: 'interrupted', reason }); } };
    stopCapture = cleanup;
    try {
      try { if (audioSession) audioSession.type = 'play-and-record'; } catch (_) {}
      stream = await navigator.mediaDevices.getUserMedia({
        audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true },
      });
      if (stopping) { stream.getTracks().forEach(track => track.stop()); return; }
      stream.getAudioTracks()[0].onended = () => interrupted('track-ended');
      // 先拿到麦克风再建 AudioContext：建它可能抢走音频焦点。
      context = new AudioContext({ latencyHint: 'interactive' });
      await context.resume();
      processor = context.createScriptProcessor(1024, 1, 1);
      sink = context.createGain();
      sink.gain.value = 0;
      processor.connect(sink);
      sink.connect(context.destination);
      source = context.createMediaStreamSource(stream);
      source.connect(processor);
      context.onstatechange = () => { if (context.state !== 'running') interrupted('context-' + context.state); };
      if (audioSession) audioSession.onstatechange = () => { if (audioSession.state === 'interrupted') interrupted('audio-session-interrupted'); };
      lastHeartbeat = performance.now();
      watchdog = setInterval(() => { if (performance.now() - lastHeartbeat > 3000) interrupted('heartbeat-expired'); }, 500);
      const step = context.sampleRate / targetRate;
      let position = 0, sum = 0, count = 0;
      processor.onaudioprocess = event => {
        const input = event.inputBuffer.getChannelData(0);
        const output = new Int16Array(Math.ceil(input.length / step) + 1);
        let written = 0;
        for (const sample of input) {
          sum += sample; count += 1; position += 1;
          if (position >= step) {
            position -= step;
            output[written++] = Math.max(-1, Math.min(1, sum / count)) * 32767;
            sum = 0; count = 0;
          }
        }
        if (written > 0) post({ type: 'audio', data: encode(output.subarray(0, written)) });
      };
      post({ type: 'started' });
    } catch (error) {
      if (stopping) return;
      cleanup();
      post({ type: 'failed', reason: error.name + ': ' + error.message });
    }
  };
  post({ type: 'ready' });
  </script></body></html>
  """
}

extension KeyboardMicrophone: WKUIDelegate, WKNavigationDelegate {
  /// 只放行麦克风，摄像头一律拒绝。
  func webView(_ webView: WKWebView, requestMediaCapturePermissionFor origin: WKSecurityOrigin,
               initiatedByFrame frame: WKFrameInfo, type: WKMediaCaptureType,
               decisionHandler: @escaping (WKPermissionDecision) -> Void) {
    decisionHandler(type == .microphone ? .grant : .deny)
  }

  func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
    interrupt("web content process terminated", denied: false)
    load()
  }
}

/// 不让 `WKUserContentController` 和麦克风互相持有。
private final class WeakScriptHandler: NSObject, WKScriptMessageHandler {
  weak var target: KeyboardMicrophone?
  init(_ target: KeyboardMicrophone) { self.target = target }

  func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
    MainActor.assumeIsolated { target?.handle(message.body) }
  }
}
