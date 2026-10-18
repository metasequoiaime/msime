import Foundation

// 这里原来是 Swift 的账号释义 `BackendAccountClient.translate`，#8201 以后没有调用方（键盘扩展的账号释义在 `CandidateTranslationStore`，经 `msime_client_account_gloss` 拼请求、解回答，见迁移差异 M68），已经删掉。文件留着是因为 Tauri 外壳 iOS 键盘扩展的生成工程（`apps/desktop/src-tauri/gen/apple`）逐文件列着它，那份工程本机重新生成不了。
