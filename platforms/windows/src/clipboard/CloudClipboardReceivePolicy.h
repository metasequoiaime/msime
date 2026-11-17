#pragma once

#include <cstdint>
#include <optional>
#include <string>

namespace msime::windows {

// 账号和中继提供 UTC 的 RFC3339 时间；归一化小数秒后再比较，拒绝无法确认的新旧关系。
inline std::optional<std::string> clipboard_remote_revision(std::string raw) {
  if (raw.size() >= 6 && raw.substr(raw.size() - 6) == "+00:00")
    raw.replace(raw.size() - 6, 6, "Z");
  if (raw.size() < 20 || raw.back() != 'Z') return std::nullopt;
  for (std::size_t i = 0; i < 19; ++i) {
    const char expected = i == 4 || i == 7 ? '-' : i == 10 ? 'T' : i == 13 || i == 16 ? ':' : '\0';
    if (expected ? raw[i] != expected : raw[i] < '0' || raw[i] > '9') return std::nullopt;
  }
  const int year = std::stoi(raw.substr(0, 4));
  const int month = std::stoi(raw.substr(5, 2));
  const int day = std::stoi(raw.substr(8, 2));
  const int days[] = {0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31};
  const bool leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
  if (year == 0 || month < 1 || month > 12 || day < 1 ||
      day > days[month] + (month == 2 && leap ? 1 : 0) ||
      std::stoi(raw.substr(11, 2)) > 23 || std::stoi(raw.substr(14, 2)) > 59 ||
      std::stoi(raw.substr(17, 2)) > 59) return std::nullopt;
  std::string fraction;
  if (raw.size() != 20) {
    if (raw[19] != '.' || raw.size() < 22 || raw.size() > 30) return std::nullopt;
    fraction = raw.substr(20, raw.size() - 21);
    if (fraction.find_first_not_of("0123456789") != std::string::npos) return std::nullopt;
  }
  fraction.resize(9, '0');
  return raw.substr(0, 19) + '.' + fraction;
}

// 只保留代次，不持有待上传正文；写入成功后才推进远端水位。
class CloudClipboardReceivePolicy {
public:
  void begin(std::uint32_t sequence, bool has_content) {
    sequence_ = sequence;
    baseline_ = has_content;
    latest_.clear();
  }
  bool receive(const std::optional<std::string> &revision, std::uint32_t sequence) {
    if (sequence != sequence_) { sequence_ = sequence; baseline_ = true; }
    if (baseline_) {
      if (revision && *revision > latest_) latest_ = *revision;
      baseline_ = false;
      return false;
    }
    return revision && *revision > latest_;
  }
  void received(const std::string &revision, std::uint32_t sequence) {
    if (revision > latest_) latest_ = revision;
    sequence_ = sequence;
    baseline_ = false;
  }
private:
  std::uint32_t sequence_ = 0;
  bool baseline_ = true;
  std::string latest_;
};

} // namespace msime::windows
