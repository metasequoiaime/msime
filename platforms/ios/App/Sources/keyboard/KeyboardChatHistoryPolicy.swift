import Foundation

struct KeyboardChatHistoryPolicy {
  static let maxMessages = 16
  static let maxContentBytes = 64 * 1024
  /// 每次请求开头的系统提示词，与共享设置页的 packages/ui/src/chat/chat-prompt.ts 和 Android 的 ChatFragment 逐字相同，由 scripts/test-chat-system-prompt-parity.py 检查。
  static let systemPrompt = "你是水杉输入法（MSIME，Metasequoia IME）里的 AI 助手。用户在水杉输入法的应用里和你对话，多半是一边聊一边试键盘：看候选、皮肤和手感。\n\n介绍自己时说你是水杉输入法的 AI 助手，不要主动报出背后的模型名称或开发公司；用户直接问用的是什么模型时，如实回答。\n\n用户发来的常常是试打的内容：一个词、半句话、没转成汉字的拼音（比如 shuishan）、错字或随手敲的字母。这时简短自然地接话，看得出是拼音就顺带说出可能想打的字，不要追问用意，也不要长篇解释。\n\n关于水杉输入法：它是开源的中文输入法，支持 Android、iOS、macOS、Linux、Windows 和 HarmonyOS，官网 msime.app。你没有它完整的功能和设置资料，被问到具体功能、设置项或操作步骤时只说你确定的部分，其余建议用户在设置里查看或到官网了解，不要编造功能、菜单名称或步骤。你看不到用户的屏幕，也不能更改任何设置。\n\n隐私：这段对话会发给 AI 服务来生成回复，不计入输入统计；用户平时用输入法打的字不会发给你。\n\n回答：默认用简体中文；用户整句用其他语言时，用那种语言回答。回答简洁，先说结论，适合在小屏幕上读，少用标题和表格。"

  /// 发给模型的完整消息：系统提示词在前，后面是最近的对话，条数和字节都给系统提示词留出位置。
  static func request(_ messages: [(role: String, text: String)]) -> [(role: String, text: String)] {
    [(role: "system", text: systemPrompt)]
      + bounded(messages, maxMessages: maxMessages - 1,
                maxContentBytes: maxContentBytes - systemPrompt.utf8.count)
  }

  static func bounded(_ messages: [(role: String, text: String)], maxMessages: Int = maxMessages,
                      maxContentBytes: Int = maxContentBytes) -> [(role: String, text: String)] {
    var result: [(role: String, text: String)] = []
    var bytes = 0
    for message in messages.reversed() {
      let count = message.text.utf8.count
      guard result.count < maxMessages, bytes + count <= maxContentBytes else { break }
      result.insert(message, at: 0)
      bytes += count
    }
    return result
  }
}
