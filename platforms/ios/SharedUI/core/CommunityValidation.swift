import Foundation

/// 社区接口和本地社区资源共用的基础字段校验。
enum CommunityValidation {
  static let maximumJavaScriptInteger = 9_007_199_254_740_991

  static func validID(_ value: String) -> Bool {
    guard let id = UUID(uuidString: value) else { return false }
    return id.uuidString != "00000000-0000-0000-0000-000000000000"
  }

  static func matchesID(_ value: String, requested: String) -> Bool {
    guard validID(value), validID(requested),
          let valueID = UUID(uuidString: value), let requestedID = UUID(uuidString: requested)
    else { return false }
    return valueID == requestedID
  }

  static func validText(_ value: String, minimum: Int, maximum: Int,
                        multiline: Bool, trimmed: Bool = false) -> Bool {
    let scalars = value.unicodeScalars
    guard (minimum...maximum).contains(scalars.count),
          (!trimmed || value == value.trimmingCharacters(in: .whitespacesAndNewlines)) else {
      return false
    }
    return !scalars.contains { scalar in
      CharacterSet.controlCharacters.contains(scalar)
        && !(multiline && (scalar == "\n" || scalar == "\t"))
    }
  }

  static func validRating(count: Int, average: Double, mine: Int) -> Bool {
    count >= 0 && count <= maximumJavaScriptInteger && (0...5).contains(mine)
      && average.isFinite && (0...5).contains(average)
      && (count != 0 || average == 0)
  }
}
