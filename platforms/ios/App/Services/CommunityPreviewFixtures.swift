#if DEBUG && targetEnvironment(simulator)
import Foundation

enum CommunityPreviewFixtures {
  static var enabled: Bool { ProcessInfo.processInfo.arguments.contains("-communityPreview") }
  static var skins: [CommunitySkin] {
    CustomKeyboardSkin.templates.prefix(2).enumerated().map { index, template in
      CommunitySkin(id: "20000000-0000-4000-8000-00000000000\(index + 1)", name: template.0,
        description: "试用皮肤", author: "水杉精选", design: template.1, downloads: 0,
        rating_count: 0, rating_average: 0, owned: false, my_rating: 0,
        category: index == 0 ? .minimal : .nature)
    }
  }
  static let items: [CommunityResource] = [
    CommunityResource(id: "10000000-0000-4000-8000-000000000001", kind: .dictionary, name: "开发者常用词", description: "常见开发术语，输入更顺手", author: "水杉精选",
      content: .init(entries: [.init(PersonalWord(key: "dai ma", value: "代码")), .init(PersonalWord(key: "kai fa", value: "开发"))]), revision: 1, saves: 12, saved: false, owned: false, rating_count: 2, rating_average: 4.5, my_rating: 0),
    CommunityResource(id: "10000000-0000-4000-8000-000000000002", kind: .dictionary, name: "日常问候", description: "常用问候和礼貌表达", author: "水杉精选",
      content: .init(entries: [.init(PersonalWord(key: "ni hao", value: "你好"))]), revision: 2, saves: 8, saved: true, owned: false, rating_count: 0, rating_average: 0, my_rating: 0),
    CommunityResource(id: "10000000-0000-4000-8000-000000000003", kind: .reply, name: "职场有分寸", description: "简洁、专业，不擅自承诺", author: "水杉精选",
      content: .init(prompt: "请用简洁专业的语气回复，明确回应对方，不编造时间或承诺。"), revision: 1, saves: 16, saved: false, owned: false, rating_count: 0, rating_average: 0, my_rating: 0)
  ]
}
#endif
