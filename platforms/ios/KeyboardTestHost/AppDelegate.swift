import UIKit

// A separate host lets keyboard integration tests run without loading the production app.
// The current iOS SDK requires a scene-based lifecycle before XCTest can attach.
@main
final class AppDelegate: UIResponder, UIApplicationDelegate {
  func application(_ application: UIApplication,
                   configurationForConnecting session: UISceneSession,
                   options: UIScene.ConnectionOptions) -> UISceneConfiguration {
    let configuration = UISceneConfiguration(name: nil, sessionRole: session.role)
    configuration.delegateClass = SceneDelegate.self
    return configuration
  }
}

final class SceneDelegate: UIResponder, UIWindowSceneDelegate {
  var window: UIWindow?

  func scene(_ scene: UIScene, willConnectTo session: UISceneSession,
             options: UIScene.ConnectionOptions) {
    guard let windowScene = scene as? UIWindowScene else { return }
    let window = UIWindow(windowScene: windowScene)
    window.rootViewController = ProcessInfo.processInfo.arguments.contains("--system-keyboard-editor")
      ? SystemKeyboardEditorViewController() : UIViewController()
    window.makeKeyAndVisible()
    self.window = window
  }
}

/// 独立编辑器只使用系统键盘，不在进程内构造产品的键盘控制器。
private final class SystemKeyboardEditorViewController: UIViewController {
  private let field = UITextField()

  override func viewDidLoad() {
    super.viewDidLoad()
    view.backgroundColor = .systemBackground
    field.accessibilityIdentifier = "systemKeyboardEditor"
    field.placeholder = "输入合成测试文字"
    field.borderStyle = .roundedRect
    field.autocapitalizationType = .none
    field.autocorrectionType = .no
    field.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(field)
    NSLayoutConstraint.activate([
      field.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 24),
      field.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 20),
      field.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -20),
      field.heightAnchor.constraint(equalToConstant: 48),
    ])
  }

  override func viewDidAppear(_ animated: Bool) {
    super.viewDidAppear(animated)
    field.becomeFirstResponder()
  }
}
