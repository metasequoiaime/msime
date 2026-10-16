import XCTest

/// 现场拼出的最小搜狗细胞词库，内容都是自造的词。每组是（读音的音节, 同音的词）；拼音表由出现过的音节按首次出现的顺序组成，音节写成 `?` 时用一个不在表里的序号。布局见 client-services 的 `dictionary::scel`。
private func scel(_ groups: [([String], [String])]) -> Data {
  var table: [String] = []
  for (reading, _) in groups {
    for syllable in reading where syllable != "?" && !table.contains(syllable) { table.append(syllable) }
  }
  var bytes = [UInt8](repeating: 0, count: 0x2628)
  func put(_ value: [UInt8], at offset: Int) { bytes.replaceSubrange(offset..<offset + value.count, with: value) }
  func u16(_ value: Int) -> [UInt8] { [UInt8(value & 0xff), UInt8(value >> 8 & 0xff)] }
  func u32(_ value: Int) -> [UInt8] { u16(value & 0xffff) + u16(value >> 16) }
  func utf16(_ text: String) -> [UInt8] { text.utf16.flatMap { u16(Int($0)) } }
  put([0x40, 0x15, 0x00, 0x00, 0x44, 0x43, 0x53, 0x01, 0x01, 0x00, 0x00, 0x00], at: 0)
  put(u32(groups.count), at: 0x120)
  put(u32(groups.reduce(0) { $0 + $1.1.count }), at: 0x124)
  put(utf16("合成词库"), at: 0x130)
  put(u32(table.count), at: 0x1540)
  var offset = 0x1544
  for (index, syllable) in table.enumerated() {
    let row = u16(index) + u16(syllable.utf16.count * 2) + utf16(syllable)
    put(row, at: offset)
    offset += row.count
  }
  for (reading, words) in groups {
    bytes += u16(words.count) + u16(reading.count * 2)
    for syllable in reading { bytes += u16(table.firstIndex(of: syllable) ?? 999) }
    for (ordinal, word) in words.enumerated() {
      bytes += u16(word.utf16.count * 2) + utf16(word) + u16(10) + u32(ordinal + 1) + [UInt8](repeating: 0, count: 6)
    }
  }
  return Data(bytes)
}

/// 搜狗细胞词库经共享层转成标准文本，再像标准格式的拼音文件一样预览和入队。
final class PersonalDictionaryScelImportTests: XCTestCase {
  private let sample = scel([
    (["ce", "shi"], ["测试"]),
    (["shi", "li"], ["实例", "事例"]),
    (["?", "jie"], ["越界"]),
  ])

  func testCellDictionaryBecomesStandardRowsInWordOrder() throws {
    let converted = try PersonalDictionaryBridge.scelText(sample)
    XCTAssertEqual(converted, ScelText(name: "合成词库", category: "", entries: 4,
                                       text: "测试\tce'shi\n实例\tshi'li\n事例\tshi'li\n越界\t\n"))
  }

  func testCellDictionaryPreviewsAsPinyinWordsAndNamesTheUnreadableOne() throws {
    XCTAssertNotNil(PersonalDictionaryBridge.packagedResources, "the test host carries the Engine resources")
    let imported = try PersonalDictionaryImport.scel(PersonalDictionaryBridge.scelText(sample))
    XCTAssertEqual(imported.file.entries.map(\.value), ["测试", "实例", "事例"])
    XCTAssertEqual(imported.file.entries.map(\.key), ["ce'shi", "shi'li", "shi'li"])
    XCTAssertTrue(imported.file.entries.allSatisfy { $0.kind == .pinyin && $0.weight == 10_000 })
    XCTAssertTrue(imported.notice.hasPrefix("已读取搜狗细胞词库「合成词库」的 4 个词条。"), imported.notice)
    // 第 4 个词读不出读音，行号就是它在词库里的词序。
    XCTAssertTrue(imported.notice.contains("跳过 1 行，首先出现在第 4 行。"), imported.notice)
  }

  func testCellDictionaryIsReadFromTheChosenFile() throws {
    let url = FileManager.default.temporaryDirectory.appendingPathComponent("msime-scel-\(UUID().uuidString).scel")
    try sample.write(to: url)
    defer { try? FileManager.default.removeItem(at: url) }
    let imported = try PersonalDictionaryImport.scelFile(from: url)
    XCTAssertEqual(imported.file.entries.count, 3)
  }

  func testDamagedOrForeignFilesSayWhatToDo() {
    XCTAssertThrowsError(try PersonalDictionaryBridge.scelText(sample.prefix(sample.count - 1))) {
      XCTAssertEqual($0 as? ScelImportFailure, .malformed)
      XCTAssertEqual($0.localizedDescription, "细胞词库文件不完整或已损坏，请重新下载后再导入。")
    }
    XCTAssertThrowsError(try PersonalDictionaryBridge.scelText(Data("测试\tce'shi\n".utf8))) {
      XCTAssertEqual($0 as? ScelImportFailure, .unsupported)
    }
    var empty = [UInt8](sample)
    empty.replaceSubrange(0x120..<0x124, with: [0, 0, 0, 0])
    XCTAssertThrowsError(try PersonalDictionaryBridge.scelText(Data(empty))) {
      XCTAssertEqual($0 as? ScelImportFailure, .empty)
    }
  }

  func testOnlyTheRowsTheParserReadsAreSent() {
    let text = (1...1500).map { "词\($0)\tci\n" }.joined()
    let kept = PersonalDictionaryImport.leadingRows(text)
    XCTAssertEqual(kept.split(separator: "\n").count, 1001)
    XCTAssertTrue(text.hasPrefix(kept))
    let long = String(repeating: "长", count: 400_000) + "\tchang\n"
    XCTAssertEqual(PersonalDictionaryImport.leadingRows("词\tci\n" + long), "词\tci\n")
  }

  /// 词长的词库（诗词、俗语）1001 行就超过解析器的 64 KiB：只送放得下的整行，并且仍报告「文件过长」，而不是整份被拒绝。
  func testLongWordRowsStayWithinTheParsersTextLimit() throws {
    let row = String(repeating: "长", count: 20) + "\t" + Array(repeating: "chang", count: 20).joined(separator: "'") + "\n"
    let text = String(repeating: row, count: 1500)
    let kept = PersonalDictionaryImport.leadingRows(text)
    XCTAssertLessThanOrEqual(kept.utf8.count, 65_536)
    XCTAssertEqual(kept.utf8.count % row.utf8.count, 0)
    XCTAssertLessThan(kept.split(separator: "\n").count, 1001)

    var sent = ""
    let imported = try PersonalDictionaryImport.scel(
      ScelText(name: "", category: "", entries: 1500, text: text)
    ) { _, _, text in
      sent = text
      return ([PersonalWord(key: "chang", value: "长")], [:])
    }
    XCTAssertEqual(sent, kept)
    XCTAssertTrue(imported.notice.contains("文件过长"), imported.notice)
  }

  func testTheCellDictionaryFormatIsOffered() {
    XCTAssertEqual(PersonalDictionaryImport.fileFormats.last?.format, "scel")
  }
}
