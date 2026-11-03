import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]


class AndroidVoiceProjectConfigurationTests(unittest.TestCase):
    def test_native_apk_preflight_matches_gradle_compile_sdk(self):
        gradle = (ROOT / "platforms/android/gradle-app/app/build.gradle.kts").read_text()
        build = (ROOT / "platforms/android/build-apk.sh").read_text()

        self.assertIn("compileSdk = 36", gradle)
        self.assertIn('platforms/android-36/android.jar', build)
        self.assertNotIn('platforms/android-35/android.jar', build)

    def test_native_apk_ships_arm64_only_unless_abis_are_requested(self):
        gradle = (ROOT / "platforms/android/gradle-app/app/build.gradle.kts").read_text()
        build = (ROOT / "platforms/android/build-apk.sh").read_text()
        native = (ROOT / "platforms/android/build-native.sh").read_text()

        # abiFilters 把 ML Kit 的 x86/x86_64/armeabi-v7a 库挡在外面；默认只有 arm64。
        self.assertIn("ndk { abiFilters += nativeAbis }", gradle)
        self.assertIn('?: listOf("arm64-v8a")', gradle)
        self.assertIn('findProperty("msimeAbis")', gradle)
        # AGP 只有在配置了 NDK 时才会 strip 打包的 .so，而且必须是 build-native.sh 固定的那个 NDK。
        self.assertIn('ndkVersion = "28.2.13676358"', gradle)
        self.assertIn("28.2.13676358", native)
        self.assertIn("${MSIME_ANDROID_ABIS:-arm64-v8a}", build)
        self.assertIn('"-PmsimeAbis=$abi_list"', build)
        self.assertNotIn("for abi in arm64-v8a x86_64", build)
        # 构建出的 APK 要核对恰好带着请求的那些 ABI。
        self.assertIn('if [ "$packaged_abis" != "$expected_abis" ]', build)

    def test_voice_activity_tracks_its_request_and_releases_the_recognizer(self):
        activity = (
            ROOT
            / "platforms/android/java/app/msime/android/voice/VoiceRecognitionActivity.java"
        ).read_text()

        self.assertIn("public static void markLaunched(String requestId)", activity)
        self.assertIn("public static boolean isRequestActive(String requestId)", activity)
        self.assertIn("EXTRA_REQUEST_ID", activity)
        self.assertIn("markLaunched(requestId)", activity)
        self.assertIn("SpeechRecognizer", activity)
        self.assertIn("stopListening()", activity)
        self.assertIn("cancel()", activity)
        self.assertIn("android.permission.RECORD_AUDIO", (ROOT / "platforms/android/AndroidManifest.xml").read_text())
        self.assertIn('String requestId = "ime-"', (ROOT / "platforms/android/java/app/msime/android/core/MSIMEInputService.java").read_text())
        self.assertIn("public static void cancelActive()", activity)

    def test_a_configured_provider_reaches_the_host_without_taking_away_the_platform_recognizer(self):
        activity = (
            ROOT / "platforms/android/java/app/msime/android/voice/VoiceRecognitionActivity.java"
        ).read_text()

        # The activity runs whichever engine the request calls for.
        self.assertIn("private boolean usesProvider()", activity)
        self.assertIn("startProviderRecognition()", activity)
        self.assertIn("HttpAsrRecognizer", activity)
        self.assertIn("!usesProvider() && !available(this)", activity)
        # Cancelling has to release the microphone the recorder is holding.
        self.assertIn("provider.cancel()", activity)

    def test_polishing_runs_off_the_main_thread_and_never_loses_the_transcript(self):
        """The rewrite is a network round trip over text the user already has. Doing it on the
        thread that delivers SpeechRecognizer results would hang the window, and treating a failed
        rewrite as a failed recognition would throw away a sentence that was recognised fine."""
        activity = (
            ROOT / "platforms/android/java/app/msime/android/voice/VoiceRecognitionActivity.java"
        ).read_text()
        polisher = (
            ROOT / "platforms/android/java/app/msime/android/voice/VoicePolisher.java"
        ).read_text()

        # onResults arrives on the main thread, so it must hand off rather than polish inline.
        self.assertIn("deliver(values.get(0));", activity)
        self.assertIn("providerWorker.execute", activity)
        # A failed rewrite keeps what was recognised.
        self.assertIn("return polished == null ? text : polished;", activity)
        self.assertIn("return null;", polisher)
        # The response stream owns the HTTP socket until it is closed; disconnect() alone is not
        # the stream's close contract on every Android URLConnection implementation.
        self.assertIn("try (InputStream input = connection.getInputStream())", polisher)

    def test_polishing_does_not_forward_bearer_tokens_across_redirects(self):
        polisher = (
            ROOT / "platforms/android/java/app/msime/android/voice/VoicePolisher.java"
        ).read_text()
        # HttpURLConnection follows redirects by default. The polish request carries a bearer
        # token, so following one could replay that credential to an endpoint outside the user's
        # configured origin before the response is parsed.
        policy = (
            ROOT / "platforms/android/java/app/msime/android/HttpConnectionPolicy.java"
        ).read_text()
        self.assertIn("HttpConnectionPolicy.rejectRedirects(connection);", polisher)
        self.assertIn("connection.setInstanceFollowRedirects(false);", policy)

    def test_keyboard_voice_releases_recognizer_when_worker_submission_is_rejected(self):
        entry = (
            ROOT / "platforms/android/java/app/msime/android/core/ImeVoiceEntry.java"
        ).read_text()
        rejected = entry.split("} catch (RejectedExecutionException error) {", 1)[1]
        cleanup = rejected.split("return false;", 1)[0]
        self.assertIn("cancel();", cleanup)

    def test_on_device_recognition_keeps_the_audio_on_the_device(self):
        """A user who chose provider `local` must get the installed model or an error, never the platform recognizer, and the model must run through the shared runtime rather than a copy."""
        activity = (
            ROOT / "platforms/android/java/app/msime/android/voice/VoiceRecognitionActivity.java"
        ).read_text()
        recognizer = (
            ROOT / "platforms/android/java/app/msime/android/voice/LocalAsrRecognizer.java"
        ).read_text()
        service = (
            ROOT / "platforms/android/java/app/msime/android/core/MSIMEInputService.java"
        ).read_text()
        jni = (ROOT / "platforms/android/native/client_jni.cpp").read_text()
        build = (ROOT / "platforms/android/build-native.sh").read_text()

        self.assertIn("configured.localModel()", service)
        self.assertIn("startLocalRecognition()", activity)
        self.assertIn("local.cancel()", activity)
        # Decoding happens on the worker, never on the thread drawing the window.
        self.assertIn("providerWorker.execute(", activity)
        self.assertIn("NativeClient.localSpeechAccept(", recognizer)
        self.assertIn("NativeClient.voiceHotwords(", recognizer)
        self.assertIn("LocalAsrPolicy.IDLE_RELEASE_MILLIS", recognizer)
        self.assertIn("msime::voice::LocalAsrSession", jni)
        self.assertIn("msime_client_voice_hotwords", jni)
        self.assertIn("shared/voice/LocalAsr.cpp", build)
        self.assertIn("scripts/fetch_voice_runtime.py --platform android", build)

    def test_the_streaming_protocol_reaches_the_host_and_builds_no_frames_of_its_own(self):
        """Android has no platform WebSocket, so this host carries one for a single endpoint. What
        it must not carry is a second copy of the protocol: the frames and the authentication
        headers are the shared implementation's, and drifting from them is silent."""
        activity = (
            ROOT / "platforms/android/java/app/msime/android/voice/VoiceRecognitionActivity.java"
        ).read_text()
        recognizer = (
            ROOT / "platforms/android/java/app/msime/android/voice/DoubaoRecognizer.java"
        ).read_text()
        session = (
            ROOT / "platforms/android/java/app/msime/android/voice/DoubaoStreamSession.java"
        ).read_text()

        # The two protocols are chosen between, never both.
        self.assertIn("startStreamingRecognition()", activity)
        self.assertIn(
            "if (!usesLocal() && !usesStreaming() && !usesProvider() && !available(this))", activity
        )

        # Frames come from the shared builders; nothing here encodes the protocol.
        self.assertIn("NativeClient.doubaoStartFrame(", recognizer)
        self.assertIn("NativeClient.doubaoAudioFrame(", recognizer)
        self.assertIn("NativeClient.doubaoDecodeFrame(", recognizer)
        self.assertIn("MAX_INBOUND_FRAME_BYTES = 1_048_576 + 10", session)
        self.assertIn("if (pending == inbound.length) {", session)
        # 收发分开：读线程一直阻塞在 read 上。Conscrypt 的 available() 不会去网络上取数据，拿它判断有没有回包会让说话期间一帧也读不到（#9886）。
        self.assertNotIn(".available()", session)
        self.assertNotIn(".available()", recognizer)
        self.assertIn('new Thread(this::receive, "msime-doubao-reader")', session)
        # 整句流式的文档把 result 写成分句列表，iOS 和 Windows 两种形状都读；只读对象时 Android 上读出来是空的。
        self.assertIn("result instanceof JSONArray segments", recognizer)
        # 错误帧的错误码要交给失败分类，不能一律当拒绝请求。
        self.assertIn("VoiceFailurePolicy.doubaoError(", recognizer)
        # Cancelling has to release both the microphone and the socket.
        self.assertIn("streaming.cancel()", activity)
        self.assertIn("socket.close()", recognizer)

    def test_the_keyboards_own_voice_entry_reads_the_same_configuration_as_the_settings_app(self):
        """The keyboard has its own voice button and never goes through the settings app. It used
        to launch the platform recogniser unconditionally, so the same device transcribed one way
        from the keyboard and another from the panel. The resolution is shared; a second copy of
        the provider rules in Java is how the two would drift apart again."""
        service = (
            ROOT / "platforms/android/java/app/msime/android/core/MSIMEInputService.java"
        ).read_text()
        configuration = (
            ROOT / "platforms/android/java/app/msime/android/voice/VoiceConfiguration.java"
        ).read_text()
        shared = (ROOT / "crates/host-api/src/ffi/host.rs").read_text()
        header = (ROOT / "crates/host-api/include/msime_client.h").read_text()
        client_core = (ROOT / "crates/client-providers/src/voice/provider.rs").read_text()

        # 解析只有共享 crate 里这一份，键盘经 host-api 的 C 接口读它。
        self.assertIn("pub fn mobile_voice_provider_configuration(", client_core)
        self.assertIn("pub fn mobile_voice_polish_configuration(", client_core)
        self.assertIn("msime_client_mobile_voice_configuration", shared)
        self.assertIn("msime_client_mobile_voice_configuration", header)

        # The keyboard asks for it and passes the answer to its own voice entry.
        self.assertIn("VoiceConfiguration.read(preferencesDirectory, requestId)", service)
        self.assertIn("configured.providerName(), configured.requestFormat(), configured.endpoint()",
                      service)
        self.assertIn("configured.streaming(), configured.polish()", service)
        # A configured provider must not be refused for want of the platform recogniser.
        self.assertIn(
            "configured.provider() == null && !VoiceRecognitionActivity.available(this)", service
        )
        # Nothing configured stays the platform recogniser rather than a failure.
        self.assertIn("return none();", configuration)
        # The provider rules stay in the two policies rather than being restated here.
        self.assertIn("DoubaoAsrPolicy.usable(", configuration)
        self.assertIn("HttpAsrPolicy.usable(", configuration)
        # The keyboard's own copy must not name an engine the request may not use. It said
        # "系统语音识别" on the button that now honours a configured provider, which was wrong for
        # exactly the users who had configured one.
        self.assertNotIn("开始系统语音识别", service)
        self.assertNotIn("点击下方按钮使用系统语音识别", service)
        self.assertIn("开始语音识别", service)

    def test_the_recording_window_can_be_ended_without_losing_the_transcript(self):
        """This activity never set a content view, so every path put a blank dialog on screen. The
        platform recogniser stops itself when the speaker stops; a provider and the streaming
        socket run to a sixty-second cap, and Back cancels. Without a Done button there was no way
        to end a recording and keep what was said."""
        activity = (
            ROOT / "platforms/android/java/app/msime/android/voice/VoiceRecognitionActivity.java"
        ).read_text()
        self.assertIn("private void showRecordingControls()", activity)
        self.assertIn("setContentView(root)", activity)
        # Both paths that cannot end themselves show it.
        streaming = activity.index("private void startStreamingRecognition()")
        provider = activity.index("private void startProviderRecognition()")
        self.assertIn("showRecordingControls();", activity[streaming:streaming + 400])
        self.assertIn("showRecordingControls();", activity[provider:provider + 400])
        # Done ends the recording and keeps the result; Cancel is the one that discards.
        self.assertIn("stopRecognition();", activity)
        self.assertIn("ViewPolicy.bindClick(cancel, this::cancelRecognition);", activity)

    def test_configured_provider_can_start_without_platform_recognizer(self):
        service = (
            ROOT / "platforms/android/java/app/msime/android/core/MSIMEInputService.java"
        ).read_text()
        render_start = service.index("private void renderVoiceResult()")
        render_end = service.index("private void renderLayoutSettingsState()", render_start)
        render = service[render_start:render_end]
        # A configured cloud or local provider is a complete voice path even on devices without
        # Android's optional SpeechRecognizer service.
        self.assertIn("VoiceConfiguration.read(", render)
        self.assertIn("configured.provider() != null", render)
        self.assertIn("VoiceRecognitionActivity.available(this)", render)

    def test_cancelling_an_upload_disconnects_the_in_flight_request(self):
        recognizer = (
            ROOT / "platforms/android/java/app/msime/android/voice/HttpAsrRecognizer.java"
        ).read_text()
        # Activity destruction can cancel after recording has handed the audio to the network.
        # Keep the connection visible to cancel() so it does not wait for the 60-second read
        # timeout while the isolated IME process is being torn down.
        self.assertIn("private volatile HttpURLConnection connection;", recognizer)
        self.assertIn("HttpURLConnection active = connection;", recognizer)
        self.assertIn("if (active != null) active.disconnect();", recognizer)
        self.assertIn("connection = opened;", recognizer)
        self.assertIn("if (connection == opened) connection = null;", recognizer)

    def test_upload_does_not_forward_bearer_tokens_across_redirects(self):
        recognizer = (
            ROOT / "platforms/android/java/app/msime/android/voice/HttpAsrRecognizer.java"
        ).read_text()
        # HttpURLConnection follows redirects by default. The upload carries a bearer token, so
        # following one could replay that credential to an endpoint outside the user's configured
        # origin before the response is parsed.
        policy = (
            ROOT / "platforms/android/java/app/msime/android/HttpConnectionPolicy.java"
        ).read_text()
        self.assertIn("HttpConnectionPolicy.rejectRedirects(opened);", recognizer)
        self.assertIn("connection.setInstanceFollowRedirects(false);", policy)

    def test_stopping_the_voice_activity_cancels_active_capture(self):
        activity = (
            ROOT / "platforms/android/java/app/msime/android/voice/VoiceRecognitionActivity.java"
        ).read_text()
        stop_start = activity.index("@Override protected void onStop()")
        destroy_start = activity.index("@Override protected void onDestroy()", stop_start)
        stopped = activity[stop_start:destroy_start]
        self.assertIn("!isChangingConfigurations()", stopped)
        self.assertIn("!isFinishing()", stopped)
        self.assertIn("recognizer != null || provider != null || streaming != null || local != null", stopped)
        self.assertIn("cancelRecognition();", stopped)


if __name__ == "__main__":
    unittest.main()
