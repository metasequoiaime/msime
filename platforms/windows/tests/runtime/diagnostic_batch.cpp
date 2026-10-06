#include "DiagnosticBatch.h"
#include <iostream>
#include <stdexcept>
#include <string>
#include <vector>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Diagnostic batch test failed at line " +
                           std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)
void put(std::vector<uint8_t> &frame, uint32_t value) {
  for (int i = 0; i < 4; ++i)
    frame.push_back(static_cast<uint8_t>(value >> (8 * i)));
}
std::vector<uint8_t> frame(const std::string &payload, uint32_t records,
                           uint32_t dropped = 0, uint32_t process = 4242,
                           uint32_t magic = FANY_IME_TSF_DIAGNOSTIC_MAGIC,
                           uint32_t version = FANY_IME_TSF_DIAGNOSTIC_VERSION,
                           uint32_t header = 28,
                           bool honest_length = true) {
  std::vector<uint8_t> bytes;
  put(bytes, magic);
  put(bytes, version);
  put(bytes, header);
  put(bytes, honest_length ? static_cast<uint32_t>(payload.size())
                           : static_cast<uint32_t>(payload.size() + 1));
  put(bytes, records);
  put(bytes, dropped);
  put(bytes, process);
  bytes.insert(bytes.end(), payload.begin(), payload.end());
  return bytes;
}
} // namespace
int main() {
  try {
    const auto utf8 = diagnostic_utf8(u"ASCII 测试 🌲");
    require(utf8 && *utf8 == "ASCII 测试 🌲");
    const auto encoded_frame = frame(*utf8, 1);
    require(parse_diagnostic_batch(encoded_frame.data(), encoded_frame.size()));
    const std::string raw_utf16_bytes{"A\0", 2};
    const auto rejected_utf16 = frame(raw_utf16_bytes, 1);
    require(!parse_diagnostic_batch(rejected_utf16.data(), rejected_utf16.size()));
    std::u16string high = u"dangling high ";
    high.push_back(0xd800);
    std::u16string low = u"dangling low ";
    low.push_back(0xdc00);
    require(!diagnostic_utf8(high));
    require(!diagnostic_utf8(low));

    // A well-formed batch decodes, including the dropped count - a gap in the
    // log has to be visible rather than silently absent.
    const auto good = frame("composition committed", 3, 7);
    const auto batch = parse_diagnostic_batch(good.data(), good.size());
    require(batch.has_value());
    require(batch->record_count == 3);
    require(batch->dropped_count == 7);
    require(batch->source_process_id == 4242);
    require(batch->payload == "composition committed");

    // An empty batch is legitimate: no records and no payload.
    const auto empty = frame("", 0);
    const auto decoded_empty = parse_diagnostic_batch(empty.data(), empty.size());
    require(decoded_empty && decoded_empty->payload.empty());
    require(decoded_empty->record_count == 0);

    // Everything the contract pins is checked, because this frame comes from
    // another process.
    const auto bad_magic = frame("x", 1, 0, 1, 0xDEADBEEF);
    require(!parse_diagnostic_batch(bad_magic.data(), bad_magic.size()));
    const auto bad_version = frame("x", 1, 0, 1, FANY_IME_TSF_DIAGNOSTIC_MAGIC, 99);
    require(!parse_diagnostic_batch(bad_version.data(), bad_version.size()));
    // A different header size means a different layout, so every offset below
    // it would be reading the wrong field.
    const auto bad_header =
        frame("x", 1, 0, 1, FANY_IME_TSF_DIAGNOSTIC_MAGIC,
              FANY_IME_TSF_DIAGNOSTIC_VERSION, 32);
    require(!parse_diagnostic_batch(bad_header.data(), bad_header.size()));

    // A declared payload longer than the frame would read past it.
    const auto lying = frame("x", 1, 0, 1, FANY_IME_TSF_DIAGNOSTIC_MAGIC,
                             FANY_IME_TSF_DIAGNOSTIC_VERSION, 28, false);
    require(!parse_diagnostic_batch(lying.data(), lying.size()));

    // Records without payload, or payload without records, is malformed both
    // ways round.
    const auto no_payload = frame("", 5);
    require(!parse_diagnostic_batch(no_payload.data(), no_payload.size()));
    const auto no_records = frame("text", 0);
    require(!parse_diagnostic_batch(no_records.data(), no_records.size()));

    // Diagnostics are text. A control byte must not reach a log verbatim.
    const auto control = frame(std::string("a\x01" "b", 3), 1);
    require(!parse_diagnostic_batch(control.data(), control.size()));
    // Tabs and newlines are ordinary in a log line and stay allowed.
    const auto whitespace = frame("line one\nline two\tend", 2);
    require(parse_diagnostic_batch(whitespace.data(), whitespace.size()));

    // Truncated, empty and oversized frames are all refused.
    require(!parse_diagnostic_batch(good.data(), 8));
    require(!parse_diagnostic_batch(nullptr, 64));
    require(!parse_diagnostic_batch(good.data(), 0));
    const auto huge = frame(std::string(FANY_IME_TSF_DIAGNOSTIC_MAX_FRAME_BYTES, 'x'), 1);
    require(!parse_diagnostic_batch(huge.data(), huge.size()));

    std::cout << "Diagnostic batch: every contract field is checked\n";
  } catch (const std::exception &failure) {
    std::cerr << failure.what() << '\n';
    return 1;
  } catch (...) {
    std::cerr << "Diagnostic batch test failed with an unknown error\n";
    return 1;
  }
}
