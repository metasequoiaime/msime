package app.msime.android

import android.app.Activity
import android.content.ClipData
import android.content.ClipboardManager
import android.content.ComponentName
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Vibrator
import android.provider.Settings
import android.view.inputmethod.InputMethodManager
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import org.json.JSONObject
import java.nio.file.Path
import java.util.concurrent.ExecutorService
import java.util.concurrent.RejectedExecutionException

@InvokeArg
class SaveAccountSessionArgs {
    lateinit var value: String
}

@InvokeArg
class SaveCloudClipboardKeystoreArgs {
    lateinit var value: String
}

@InvokeArg
class SaveFeedbackArgs {
    var soundEnabled: Boolean = true
    var hapticsEnabled: Boolean = false
    var hapticStrength: String = "medium"
}

@InvokeArg
class SetAppIconArgs {
    var style: String = ""
}

@InvokeArg
class CopyTextArgs {
    lateinit var text: String
}

@InvokeArg
class OpenExternalUrlArgs {
    lateinit var url: String
}

@InvokeArg
class EnqueueSnapshotArgs {
    lateinit var source: String
    lateinit var accountId: String
    var cloudRevision: Long = -1
    lateinit var expectedLocalVersion: String
    lateinit var fileSha256: String
}

@InvokeArg
class CancelSnapshotArgs {
    lateinit var accountId: String
}

@InvokeArg
class AiModelsArgs {
    lateinit var endpoint: String
    lateinit var token: String
}

@InvokeArg
class AiTestArgs {
    lateinit var endpoint: String
    lateinit var model: String
    lateinit var prompt: String
    lateinit var token: String
    lateinit var text: String
}

@TauriPlugin
class AccountPlugin(activity: Activity) : Plugin(activity) {
    private val hostActivity = activity
    private val storage = AndroidAccountSessionStorage(activity)
    private val clipboardKeystore = CloudClipboardKeystore.deferred(activity)
    private val bootstrapWorker: ExecutorService = AccountTaskExecutor.create()

    override fun onDestroy() {
        bootstrapWorker.shutdownNow()
        super.onDestroy()
    }

    /** 词库快照激活队列的目录。队列本身在 Rust 的 msime_client_snapshot_queue 里（#8201 迁移计划 5d），与原生页、键盘用的是同一份、同一把 flock。 */
    private fun snapshotQueueRoot(): Path = DictionarySnapshotQueue.directory(hostActivity)

    private fun privateSnapshotSource(value: String): Path {
        return DictionarySnapshotPathPolicy.privateSource(
            hostActivity.filesDir.toPath(), snapshotQueueRoot(), value)
    }

    /** Rust 的队列状态换成 Tauri 一侧原来读的形状：没有的字段不写，不带 Rust 的 version。 */
    private fun snapshotStateResponse(state: JSONObject): JSObject {
        val response = JSObject()
        DictionarySnapshotQueue.localVersion(state)?.let { response.put("localVersion", it) }
        val request = state.optJSONObject("request")
        if (request != null) {
            response.put("request", JSObject()
                .put("id", request.getString("id"))
                .put("accountId", request.getString("accountId"))
                .put("cloudRevision", request.getLong("cloudRevision"))
                .put("expectedLocalVersion", request.getString("expectedLocalVersion"))
                .put("fileSha256", request.getString("fileSha256"))
                .put("status", request.getString("status")))
        }
        return response
    }

    private val appIconAliases = linkedMapOf(
        "classic" to null,
        "forest" to "MainActivityForest",
        "sky" to "MainActivitySky",
        "dusk" to "MainActivityDusk",
        "vermilion" to "MainActivityVermilion",
    )

    private fun appIconComponent(className: String?): ComponentName {
        // The aliases are declared under the namespace, which differs from the package name in every edition but full: the package names the app, the namespace names the class.
        return className?.let { ComponentName(hostActivity.packageName, "${AppIconStyle.namespace()}.$it") }
            ?: ComponentName(hostActivity, hostActivity.javaClass)
    }

    private fun appIconSelected(): String {
        val packageManager = hostActivity.packageManager
        for ((style, alias) in appIconAliases) {
            if (alias == null) continue
            val state = packageManager.getComponentEnabledSetting(appIconComponent(alias))
            if (state == PackageManager.COMPONENT_ENABLED_STATE_ENABLED) return style
        }
        // The base activity is enabled by the manifest unless a custom alias
        // has been selected. It is also the safe answer for an interrupted
        // package-manager update or an icon ID from a future build.
        return "classic"
    }

    private fun setAppIcon(style: String) {
        if (!appIconAliases.containsKey(style)) {
            throw IllegalArgumentException("invalid app icon")
        }
        val packageManager = hostActivity.packageManager
        val selected = appIconAliases[style]
        // Enable the target before disabling the old launcher component so the
        // launcher never observes a package with no entry point.
        packageManager.setComponentEnabledSetting(
            appIconComponent(selected),
            PackageManager.COMPONENT_ENABLED_STATE_ENABLED,
            PackageManager.DONT_KILL_APP,
        )
        for ((_, alias) in appIconAliases) {
            if (alias != selected) {
                packageManager.setComponentEnabledSetting(
                    appIconComponent(alias),
                    PackageManager.COMPONENT_ENABLED_STATE_DISABLED,
                    PackageManager.DONT_KILL_APP,
                )
            }
        }
    }

    @Command
    fun loadSession(invoke: Invoke) {
        try {
            val response = JSObject()
            response.put("value", storage.load())
            invoke.resolve(response)
        } catch (_: Exception) {
            invoke.reject("secure_storage", "secure_storage")
        }
    }

    @Command
    fun saveSession(invoke: Invoke) {
        try {
            storage.save(invoke.parseArgs(SaveAccountSessionArgs::class.java).value)
            invoke.resolve()
        } catch (_: Exception) {
            invoke.reject("secure_storage", "secure_storage")
        }
    }

    // 退出登录（包括会话被服务端吊销后 Rust 清掉它）时，云剪贴板的本机密钥串随会话一起删除；先清会话再删密钥串：正在写密钥串的一方在密钥串的锁里核对会话，会话一清它就不会再写，而这里删除时不拿会话的锁，两把锁不会互相等。
    @Command
    fun clearSession(invoke: Invoke) {
        try {
            storage.clear()
            clipboardKeystore.delete()
            invoke.resolve()
        } catch (_: Exception) {
            invoke.reject("secure_storage", "secure_storage")
        }
    }

    // 云剪贴板的本机密钥串，与键盘（:ime 进程）和原生设置页读写的是同一个文件（CloudClipboardKeystore），见 docs/cloud-clipboard-e2e.md 的「密钥串的存放位置」。
    @Command
    fun loadCloudClipboardKeystore(invoke: Invoke) {
        try {
            val response = JSObject()
            response.put("value", clipboardKeystore.load())
            invoke.resolve(response)
        } catch (_: Exception) {
            invoke.reject("secure_storage", "secure_storage")
        }
    }

    // 只在有账号会话时写入，退出登录后迟到的一步不会把密钥串写回来；文件里已有另一个设备身份时拒绝，Rust 下一次读到的就是那一个，一个安装只登记成一台设备。
    @Command
    fun saveCloudClipboardKeystore(invoke: Invoke) {
        try {
            val value = invoke.parseArgs(SaveCloudClipboardKeystoreArgs::class.java).value
            clipboardKeystore.save(value) {
                if (storage.load() == null) throw IllegalStateException("signed_out")
            }
            invoke.resolve()
        } catch (_: Exception) {
            invoke.reject("secure_storage", "secure_storage")
        }
    }

    @Command
    fun loadFeedback(invoke: Invoke) {
        try {
            val settings = KeyboardFeedbackStore.load(hostActivity)
            val response = JSObject()
            response.put("soundEnabled", settings.soundEnabled())
            response.put("hapticsEnabled", settings.hapticsEnabled())
            response.put("hapticStrength", settings.hapticStrength().id())
            invoke.resolve(response)
        } catch (_: Exception) {
            invoke.reject("feedback_storage", "feedback_storage")
        }
    }

    @Command
    fun saveFeedback(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(SaveFeedbackArgs::class.java)
            if (!KeyboardFeedbackPreferences.known(args.hapticStrength)) {
                invoke.reject("invalid_feedback", "invalid_feedback")
                return
            }
            KeyboardFeedbackStore.save(
                hostActivity,
                KeyboardFeedbackStore.Settings(
                    args.soundEnabled,
                    args.hapticsEnabled,
                    KeyboardFeedbackPreferences.strength(args.hapticStrength),
                ),
            )
            invoke.resolve()
        } catch (_: Exception) {
            invoke.reject("feedback_storage", "feedback_storage")
        }
    }

    @Command
    fun previewFeedback(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(SaveFeedbackArgs::class.java)
            val strength = KeyboardFeedbackPreferences.strength(args.hapticStrength)
            if (!KeyboardFeedbackPreferences.known(args.hapticStrength)) {
                invoke.reject("invalid_feedback", "invalid_feedback")
                return
            }
            // 与键盘按键同一套振动（KeyboardHaptics），试的就是键盘上的那一下；「跟随系统」由页面根视图交给系统触感反馈，要在主线程上做。
            val vibrator = hostActivity.getSystemService(Vibrator::class.java)
            hostActivity.runOnUiThread {
                KeyboardHaptics.play(vibrator, hostActivity.window.decorView, strength)
            }
            invoke.resolve()
        } catch (_: Exception) {
            invoke.reject("feedback_preview", "feedback_preview")
        }
    }

    @Command
    fun appIconInfo(invoke: Invoke) {
        try {
            val response = JSObject()
            response.put("supported", true)
            response.put("selected", appIconSelected())
            invoke.resolve(response)
        } catch (_: Exception) {
            invoke.reject("app_icon", "app_icon")
        }
    }

    @Command
    fun setAppIcon(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(SetAppIconArgs::class.java)
            setAppIcon(args.style)
            val response = JSObject()
            response.put("supported", true)
            response.put("selected", appIconSelected())
            invoke.resolve(response)
        } catch (error: IllegalArgumentException) {
            invoke.reject("invalid_app_icon", error.message)
        } catch (_: Exception) {
            invoke.reject("app_icon", "app_icon")
        }
    }

    @Command
    fun copyText(invoke: Invoke) {
        try {
            val text = invoke.parseArgs(CopyTextArgs::class.java).text
            if (text.isEmpty() || text.length > 4000 || text.contains('\u0000') ||
                TextPolicy.hasControlExceptWhitespace(text) || !TextPolicy.validUnicode(text)) {
                invoke.reject("invalid_text", "invalid_text")
                return
            }
            val clipboard = hostActivity.getSystemService(ClipboardManager::class.java)
                ?: throw IllegalStateException("clipboard unavailable")
            clipboard.setPrimaryClip(ClipData.newPlainText("MSIME", text))
            invoke.resolve()
        } catch (_: IllegalArgumentException) {
            invoke.reject("invalid_text", "invalid_text")
        } catch (_: Exception) {
            invoke.reject("clipboard", "clipboard")
        }
    }

    @Command
    fun openExternalUrl(invoke: Invoke) {
        try {
            val url = invoke.parseArgs(OpenExternalUrlArgs::class.java).url
            if (url.length > 4096 || !url.startsWith("https://") ||
                url.any { it.isWhitespace() } || TextPolicy.hasControl(url)
                || !TextPolicy.validUnicode(url)) {
                invoke.reject("invalid_url", "invalid_url")
                return
            }
            val parsed = Intent.parseUri(url, Intent.URI_INTENT_SCHEME).data
                ?: throw IllegalArgumentException("missing URL")
            if (parsed.scheme != "https" || parsed.host.isNullOrEmpty()) {
                invoke.reject("invalid_url", "invalid_url")
                return
            }
            val intent = Intent(Intent.ACTION_VIEW, parsed).addCategory(Intent.CATEGORY_BROWSABLE)
            hostActivity.startActivity(intent)
            invoke.resolve()
        } catch (_: IllegalArgumentException) {
            invoke.reject("invalid_url", "invalid_url")
        } catch (_: Exception) {
            invoke.reject("external_url", "external_url")
        }
    }

    @Command
    fun openInputMethodSettings(invoke: Invoke) {
        try {
            hostActivity.startActivity(Intent(Settings.ACTION_INPUT_METHOD_SETTINGS))
            invoke.resolve()
        } catch (_: Exception) {
            invoke.reject("system_settings", "system_settings")
        }
    }

    @Command
    fun openKeyboardTryout(invoke: Invoke) {
        try {
            val intent = Intent().setClassName(
                hostActivity,
                "app.msime.android.home.KeyboardTryoutActivity",
            )
            hostActivity.startActivity(intent)
            invoke.resolve()
        } catch (_: Exception) {
            invoke.reject("keyboard_tryout", "keyboard_tryout")
        }
    }

    @Command
    fun showInputMethodPicker(invoke: Invoke) {
        try {
            val manager = hostActivity.getSystemService(InputMethodManager::class.java)
                ?: throw IllegalStateException("input method manager unavailable")
            manager.showInputMethodPicker()
            invoke.resolve()
        } catch (_: Exception) {
            invoke.reject("input_method_picker", "input_method_picker")
        }
    }

    @Command
    fun bootstrapStatus(invoke: Invoke) {
        try {
            val ready = File(hostActivity.filesDir, "runtime-options.json").isFile
            invoke.resolve(JSObject().put("ready", ready))
        } catch (_: Exception) {
            invoke.reject("bootstrap", "bootstrap")
        }
    }

    @Command
    fun prepareBootstrap(invoke: Invoke) {
        try {
            bootstrapWorker.execute {
                try {
                    Bootstrap.prepare(hostActivity.applicationContext)
                    val ready = File(hostActivity.filesDir, "runtime-options.json").isFile
                    invoke.resolve(JSObject().put("ready", ready))
                } catch (_: Exception) {
                    invoke.reject("bootstrap", "bootstrap")
                }
            }
        } catch (_: RejectedExecutionException) {
            invoke.reject("bootstrap_busy", "bootstrap")
        } catch (_: Exception) {
            invoke.reject("bootstrap", "bootstrap")
        }
    }

    @Command
    fun aiModels(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(AiModelsArgs::class.java)
            bootstrapWorker.execute {
                try {
                    val models = AiPolishModelCatalog.fetch(args.endpoint, args.token)
                    val response = JSObject()
                    val values = org.json.JSONArray()
                    models.forEach { values.put(it) }
                    response.put("models", values)
                    invoke.resolve(response)
                } catch (error: AiPolishClient.Failure) {
                    invoke.reject("ai_models_${error.reason().name.lowercase()}", "ai_models_failed")
                } catch (_: Exception) {
                    invoke.reject("ai_models_unavailable", "ai_models_failed")
                }
            }
        } catch (_: RejectedExecutionException) {
            invoke.reject("ai_models_busy", "ai_models_failed")
        } catch (_: Exception) {
            invoke.reject("ai_models_invalid", "ai_models_failed")
        }
    }

    @Command
    fun aiTest(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(AiTestArgs::class.java)
            bootstrapWorker.execute {
                try {
                    val configuration = AiPolishConfiguration(args.endpoint, args.model, args.prompt, args.token)
                    if (!AiPolishConfiguration.acceptableText(args.text)) {
                        throw AiPolishClient.Failure(AiPolishClient.Reason.INVALID)
                    }
                    val result = AiPolishHttpTransport().send(
                        configuration, args.text, AiPolishClient.Cancellation())
                    if (!AiPolishConfiguration.acceptableText(result)) {
                        throw AiPolishClient.Failure(AiPolishClient.Reason.INVALID)
                    }
                    invoke.resolve(JSObject().put("text", result))
                } catch (error: AiPolishClient.Failure) {
                    invoke.reject("ai_test_${error.reason().name.lowercase()}", "ai_test_failed")
                } catch (_: IllegalArgumentException) {
                    invoke.reject("ai_test_invalid", "ai_test_failed")
                } catch (_: Exception) {
                    invoke.reject("ai_test_unavailable", "ai_test_failed")
                }
            }
        } catch (_: RejectedExecutionException) {
            invoke.reject("ai_test_busy", "ai_test_failed")
        } catch (_: Exception) {
            invoke.reject("ai_test_invalid", "ai_test_failed")
        }
    }

    /** Rust-only bridge; WebView code never receives the private source path. */
    @Command
    fun enqueueSnapshot(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(EnqueueSnapshotArgs::class.java)
            val state = DictionarySnapshotQueue.enqueue(snapshotQueueRoot(),
                privateSnapshotSource(args.source), args.accountId, args.cloudRevision,
                args.expectedLocalVersion, args.fileSha256)
            invoke.resolve(snapshotStateResponse(state))
        } catch (error: DictionarySnapshotQueue.Failure) {
            invoke.reject(error.code, error.code)
        } catch (error: IllegalArgumentException) {
            invoke.reject("snapshot_invalid", error.message)
        } catch (_: Exception) {
            invoke.reject("snapshot_unavailable", "snapshot_unavailable")
        }
    }

    @Command
    fun snapshotState(invoke: Invoke) {
        try { invoke.resolve(snapshotStateResponse(DictionarySnapshotQueue.read(snapshotQueueRoot()))) }
        catch (error: DictionarySnapshotQueue.Failure) {
            invoke.reject(error.code, error.code)
        } catch (_: Exception) { invoke.reject("snapshot_unavailable", "snapshot_unavailable") }
    }

    @Command
    fun cancelSnapshot(invoke: Invoke) {
        try {
            val accountId = invoke.parseArgs(CancelSnapshotArgs::class.java).accountId
            invoke.resolve(snapshotStateResponse(DictionarySnapshotQueue.cancel(snapshotQueueRoot(), accountId)))
        } catch (error: DictionarySnapshotQueue.Failure) {
            invoke.reject(error.code, error.code)
        } catch (error: IllegalArgumentException) {
            invoke.reject("snapshot_invalid", error.message)
        } catch (_: Exception) { invoke.reject("snapshot_unavailable", "snapshot_unavailable") }
    }
}
