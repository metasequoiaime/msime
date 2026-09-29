package app.msime.android

import android.os.Bundle
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.activity.OnBackPressedCallback
import app.msime.android.TauriActivity

class MainActivity : TauriActivity() {
  private var settingsWebView: WebView? = null
  private var pendingMobilePanel: String? = null
  private var pendingSettingsPage: String? = null

  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    settingsWebView = webView
    WindowLayout.fitSystemBars(webView)
    webView.post { webView.requestApplyInsets() }
    dispatchPendingMobilePanel(webView)
    dispatchPendingSettingsPage(webView)
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    pendingMobilePanel = intent.getStringExtra("msime_mobile_panel")
    pendingSettingsPage = intent.getStringExtra("msime_settings_page")
    super.onCreate(savedInstanceState)
    // Preparation used to sit behind a button on a development launcher screen. That screen is
    // gone, so each launcher triggers it; the call is idempotent and never overwrites an existing
    // configuration. Without this the bundle would ship a keyboard that cannot reach the Engine.
    FirstRunPreparation.startIfNeeded(this)
    onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
      override fun handleOnBackPressed() {
        val webView = settingsWebView
        if (webView?.canGoBack() == true) {
          // The shared mobile settings UI owns the history entries. Going back
          // here dispatches popstate so nested pages and panels restore their
          // state instead of finishing the Activity.
          webView.goBack()
          return
        }
        // No app-owned history remains; let Android finish the Activity. The
        // callback must be disabled first to avoid recursively re-entering it.
        isEnabled = false
        onBackPressedDispatcher.onBackPressed()
        isEnabled = true
      }
    })
  }

  override fun onNewIntent(intent: android.content.Intent?) {
    super.onNewIntent(intent)
    setIntent(intent)
    pendingMobilePanel = intent?.getStringExtra("msime_mobile_panel")
    pendingSettingsPage = intent?.getStringExtra("msime_settings_page")
    settingsWebView?.let { dispatchPendingMobilePanel(it) }
    settingsWebView?.let { dispatchPendingSettingsPage(it) }
  }

  private fun dispatchPendingMobilePanel(webView: WebView) {
    val panel = pendingMobilePanel ?: return
    if (panel != "cloud-dictionary") return
    pendingMobilePanel = null
    webView.postDelayed({
      webView.evaluateJavascript(
        "window.dispatchEvent(new CustomEvent('msime-mobile-panel',{detail:'cloud-dictionary'}));",
        null,
      )
    }, 250)
  }

  private fun dispatchPendingSettingsPage(webView: WebView) {
    val page = pendingSettingsPage ?: return
    if (page != "dictionary" && page != "account" && page != "about"
        && page != "help" && page != "feedback" && page != "vocabulary") return
    pendingSettingsPage = null
    webView.postDelayed({
      webView.evaluateJavascript(
        "window.dispatchEvent(new CustomEvent('msime-settings-page',{detail:'$page'}));",
        null,
      )
    }, 250)
  }

  override fun onDestroy() {
    settingsWebView = null
    super.onDestroy()
  }
}
