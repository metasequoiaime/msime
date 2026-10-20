import XCTest
import UIKit

@MainActor
final class KeyboardStickerCatalogTests: XCTestCase {
  /// 仓库里的 `resources/stickers/shuishan`，键盘扩展打包的就是这个目录。
  private static let packaged = URL(fileURLWithPath: #filePath)
    .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    .appendingPathComponent("../../resources/stickers/shuishan", isDirectory: true).standardizedFileURL

  func testPackagedManifestListsEveryStickerInOrder() throws {
    let stickers = try KeyboardStickerCatalog.load(directory: Self.packaged)
    XCTAssertEqual(stickers.count, 66)
    XCTAssertEqual(stickers.first?.name, "开心")
    XCTAssertEqual(stickers.first?.url.lastPathComponent, "01.webp")
    XCTAssertEqual(stickers.last?.name, "求表扬")
    for sticker in stickers {
      XCTAssertTrue(FileManager.default.fileExists(atPath: sticker.url.path), sticker.url.lastPathComponent)
    }
  }

  func testManifestRejectsUnsafeOrMalformedEntries() {
    let directory = URL(fileURLWithPath: "/fixture", isDirectory: true)
    func parse(_ entries: [[String: Any]]) throws -> [KeyboardStickerCatalog.Sticker] {
      let data = try JSONSerialization.data(withJSONObject: ["name": "水杉娘", "stickers": entries])
      return try KeyboardStickerCatalog.parse(manifest: data, directory: directory)
    }
    XCTAssertEqual(try parse([["file": "01.webp", "name": " 开心 "]]).map(\.name), ["开心"])
    XCTAssertThrowsError(try parse([]), "an empty pack is not shown")
    XCTAssertThrowsError(try parse([["file": "../01.webp", "name": "开心"]]), "no path outside the pack")
    XCTAssertThrowsError(try parse([["file": "1.webp", "name": "开心"]]))
    XCTAssertThrowsError(try parse([["file": "01.png", "name": "开心"]]))
    XCTAssertThrowsError(try parse([["file": "01.webp", "name": ""]]))
    XCTAssertThrowsError(try parse([["file": "01.webp", "name": String(repeating: "长", count: 17)]]))
    XCTAssertThrowsError(try parse([["file": "01.webp", "name": "开心"], ["file": "01.webp", "name": "大笑"]]))
    XCTAssertThrowsError(try parse(Array(repeating: ["file": "01.webp", "name": "开心"],
                                         count: KeyboardStickerCatalog.maximumStickers + 1)))
    XCTAssertThrowsError(try KeyboardStickerCatalog.parse(manifest: Data("[]".utf8), directory: directory))
  }

  func testColumnsKeepFiveOnAPhoneAndGrowOnWiderKeyboards() {
    XCTAssertEqual(KeyboardStickerCatalog.columns(width: 320), 5)
    XCTAssertEqual(KeyboardStickerCatalog.columns(width: 390), 5)
    XCTAssertEqual(KeyboardStickerCatalog.columns(width: 844), 11)
  }

  func testStickerDecodesToThumbnailAndTransparentPNG() throws {
    let sticker = try XCTUnwrap(KeyboardStickerCatalog.load(directory: Self.packaged).first)
    let thumbnail = try XCTUnwrap(KeyboardStickerCatalog.thumbnail(sticker, side: 80, scale: 2))
    XCTAssertEqual(thumbnail.scale, 2)
    XCTAssertLessThanOrEqual(max(thumbnail.size.width, thumbnail.size.height), 80)
    let png = try XCTUnwrap(KeyboardStickerCatalog.pngData(sticker))
    XCTAssertEqual(Array(png.prefix(4)), [0x89, 0x50, 0x4E, 0x47])
    let image = try XCTUnwrap(UIImage(data: png)?.cgImage)
    XCTAssertEqual(image.width, 320)
    XCTAssertTrue([.first, .last, .premultipliedFirst, .premultipliedLast].contains(image.alphaInfo),
                  "the copied sticker keeps its transparent background")
    XCTAssertEqual(KeyboardStickerCatalog.copiedMessage(sticker), "已复制水杉娘「开心」，长按输入框粘贴发送")
  }

  func testPickerOpensOnStickersFirstAndHandsATappedOneToTheHost() throws {
    let stickers = try KeyboardStickerCatalog.load(directory: Self.packaged)
    var copied: KeyboardStickerCatalog.Sticker?
    let picker = KeyboardEmojiPickerView(
      resources: "/fixture",
      loader: { _, _ in KeyboardEmojiCatalog.Page(items: [], nextOffset: 0, complete: true) },
      showsHeader: false,
      onInsert: { _ in XCTFail("a sticker is not inserted as text") },
      onDelete: {},
      onClose: {},
      stickers: stickers,
      onSticker: { copied = $0 })
    picker.frame = CGRect(x: 0, y: 0, width: 390, height: 260)
    picker.layoutIfNeeded()

    let chips = descendants(picker).compactMap { $0 as? UIButton }
      .filter { $0.accessibilityIdentifier?.hasPrefix("emojiCategory-") == true }
    let chip = try XCTUnwrap(chips.first { $0.accessibilityLabel == "水杉娘表情包" })
    XCTAssertEqual(chip.accessibilityIdentifier, "emojiCategory-0", "the sticker tab comes first")
    XCTAssertNotNil(chip.configuration?.image, "the chip shows the first sticker")
    XCTAssertTrue(chip.accessibilityTraits.contains(.selected), "the panel opens on the stickers")

    let grid = try XCTUnwrap(descendants(picker).first { $0.accessibilityIdentifier == "emojiGrid" } as? UICollectionView)
    XCTAssertEqual(grid.numberOfItems(inSection: 0), 66)
    picker.collectionView(grid, didSelectItemAt: IndexPath(item: 1, section: 0))
    XCTAssertEqual(copied?.name, "大笑")
    XCTAssertFalse(KeyboardEmojiRecents.stored.contains("大笑"))
  }

  func testPickerWithoutAHostHandlerHasNoStickerTab() {
    let picker = KeyboardEmojiPickerView(
      resources: "/fixture",
      loader: { _, _ in KeyboardEmojiCatalog.Page(items: [], nextOffset: 0, complete: true) },
      showsHeader: false,
      onInsert: { _ in }, onDelete: {}, onClose: {})
    XCTAssertFalse(descendants(picker).contains { $0.accessibilityLabel == "水杉娘表情包" })
  }

  private func descendants(_ root: UIView) -> [UIView] {
    root.subviews + root.subviews.flatMap(descendants)
  }
}
