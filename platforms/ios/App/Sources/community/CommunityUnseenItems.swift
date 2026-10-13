import Foundation

/// 追加社区分页结果或导入词条时，仅排除当前列表已经拥有的 id，并保留新结果的顺序。
enum CommunityUnseenItems {
  static func filter<Item: Identifiable>(existing: [Item], incoming: [Item]) -> [Item] {
    let existingIDs = Set(existing.map(\.id))
    return incoming.filter { !existingIDs.contains($0.id) }
  }
}
