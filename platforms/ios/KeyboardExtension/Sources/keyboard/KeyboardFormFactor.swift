import UIKit

/// The physical keyboard the extension is drawing: a phone keyboard or a full-width tablet one.
///
/// It is decided from the traits the host hands the extension rather than from the device model, because an iPad does not always get the tablet keyboard: the floating keyboard and narrow Slide Over / Stage Manager windows report a compact width, and the system keyboard draws its phone layout there too. Following the same rule keeps this keyboard the same size as the one it replaces.
enum KeyboardFormFactor: Equatable {
  case phone
  case tablet

  static func resolve(
    idiom: UIUserInterfaceIdiom, horizontalSizeClass: UIUserInterfaceSizeClass
  ) -> KeyboardFormFactor {
    idiom == .pad && horizontalSizeClass != .compact ? .tablet : .phone
  }

  static func resolve(_ traits: UITraitCollection) -> KeyboardFormFactor {
    resolve(idiom: traits.userInterfaceIdiom, horizontalSizeClass: traits.horizontalSizeClass)
  }

  /// Keyboard height before the composition line, gloss lines and the user's own adjustment.
  ///
  /// Tablet keys are much wider than phone keys, so keeping the phone height there leaves them as flat slabs that are hard to aim at. Unlike the phone, an iPad keyboard gets taller in landscape, as the system keyboard does, because the screen gets wider there.
  ///
  /// The portrait tablet height gives the design's 54pt iPad keys (dc.html L1559, `keyH`) with the default margins and row spacing; landscape keeps the taller keys the system keyboard has there, which the design does not draw.
  ///
  /// The tablet digit row is a whole extra row of keys, so it adds a row's height rather than squeezing the letters.
  func baseHeight(landscape: Bool, handwriting: Bool, numberRow: Bool = false) -> CGFloat {
    switch self {
    case .phone:
      // Handwriting keeps a small allowance in landscape so the writing canvas stays usable.
      guard landscape else { return 260 }
      return handwriting ? 240 : 216
    case .tablet:
      let base: CGFloat = landscape ? 372 : 296
      return numberRow ? base + (landscape ? 68 : 61) : base
    }
  }

  /// Whether the third letter row carries the comma and full-stop keys, as the iPad system keyboard does.
  var showsLetterRowPunctuation: Bool { self == .tablet }

  /// Whether the keyboard can carry the digit row and Tab key of a full-size keyboard (`KeyboardLayoutPreference.tabletFullKeys`). A phone-width keyboard has no room for either.
  var canShowFullKeys: Bool { self == .tablet }
}
