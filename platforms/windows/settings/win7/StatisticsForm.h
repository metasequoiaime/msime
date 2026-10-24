#pragma once

#include "ControlText.h"
#include "SettingsWindow.h"
#include "StatisticsData.h"
#include <commctrl.h>
#include <functional>
#include <map>

namespace msime::settings::win7 {

namespace statistics_model = msime::settings::typing;

class StatisticsForm {
public:
  using Row = std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>;
  struct Command {
    bool handled = false;
    std::optional<Json> action;
    bool upload = false;
    std::wstring confirmation;
  };
  int create(const Row& row) {
    int top = 0;
    const auto control = [&](int id, const wchar_t* type, const wchar_t* title, DWORD style, int height = 28) {
      const auto window = row(type, title, style, id, top, height);
      if (id >= 0) controls_[id] = window;
      top += (std::wstring_view(type) == WC_COMBOBOXW ? 28 : height) + 12;
      return window;
    };
    const auto combo = [&](int id, const wchar_t* label, const std::vector<const wchar_t*>& choices) {
      control(-1, L"STATIC", label, SS_LEFT);
      const auto window = control(id, WC_COMBOBOXW, label, WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, 220);
      for (const auto* choice : choices) SendMessageW(window, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(choice));
      SendMessageW(window, CB_SETCURSEL, 0, 0);
      return window;
    };
    control(statistics_enabled, L"BUTTON", L"记录打字统计", WS_TABSTOP | BS_AUTOCHECKBOX);
    control(statistics_privacy, L"STATIC", L"", SS_LEFT, 96);
    std::vector<const wchar_t*> retentions;
    for (const auto& choice : statistics_model::retention_choices) retentions.push_back(choice.label);
    combo(statistics_retention, L"自动清理", retentions);
    control(statistics_upload, L"BUTTON", L"上传到云端", WS_TABSTOP | BS_AUTOCHECKBOX);
    control(statistics_upload_notice, L"STATIC", L"正在读取上传设置…", SS_LEFT, 112);
    control(statistics_overview, L"STATIC", L"", SS_LEFT, 204);
    control(-1, L"STATIC", L"每日输入（最近一年）", SS_LEFT);
    control(statistics_calendar, L"STATIC", L"每日输入热力图", SS_OWNERDRAW | WS_BORDER, 200);
    control(-1, L"STATIC", L"最近 30 天速度（字 / 分钟）", SS_LEFT);
    control(statistics_speed, L"STATIC", L"每日速度", SS_OWNERDRAW | WS_BORDER, 200);
    combo(statistics_scope, L"查看范围", {L"整个保留期", L"最近 7 天", L"最近 30 天", L"选中的一天"});
    combo(statistics_chart_kind, L"图表", {L"按键", L"趋势", L"类型", L"模式", L"方案", L"候选"});
    control(statistics_chart, L"STATIC", L"统计图表", SS_OWNERDRAW | WS_BORDER, 320);
    control(statistics_chart_text, L"EDIT", L"", WS_TABSTOP | WS_BORDER | WS_VSCROLL | ES_MULTILINE | ES_READONLY, 240);
    control(-1, L"STATIC", L"逐日明细（选择一天后可切换范围）", SS_LEFT, 48);
    control(statistics_days, L"LISTBOX", L"逐日明细", WS_TABSTOP | WS_BORDER | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT, 180);
    control(statistics_day_detail, L"EDIT", L"", WS_TABSTOP | WS_BORDER | WS_VSCROLL | ES_MULTILINE | ES_READONLY, 160);
    control(statistics_notice, L"STATIC", L"", SS_LEFT, 80);
    control(statistics_reset, L"BUTTON", L"清空全部统计…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    return top;
  }
  void enable(bool enabled) {
    enabled_ = enabled;
    for (const auto& item : controls_) EnableWindow(item.second, enabled && status_.has_value());
    EnableWindow(get(statistics_upload), enabled && upload_.has_value() && (upload_->signed_in || upload_->enabled));
  }
  bool loaded() const { return status_.has_value(); }
  void completed(const Json& response) {
    const auto next = statistics_data::parse(response);
    if (!next) {
      load_error_ = true;
      SetWindowTextW(get(statistics_notice), L"无法读取或保存统计，请稍后重试。原有统计不会被自动重置。");
      restore(); return;
    }
    load_error_ = false;
    status_ = next;
    SendMessageW(get(statistics_enabled), BM_SETCHECK, status_->statistics.enabled ? BST_CHECKED : BST_UNCHECKED, 0);
    const auto retention = statistics_model::retention_value(status_->statistics.retention);
    for (size_t index = 0; index < statistics_model::retention_choices.size(); ++index)
      if (retention == statistics_model::retention_choices[index].value) SendMessageW(get(statistics_retention), CB_SETCURSEL, index, 0);
    update();
  }
  void upload_completed(const Json& response, bool changing) {
    const auto value = theme_data::value(response);
    if (!value || !value->is_object() || !value->contains("enabled") || !value->at("enabled").is_boolean() ||
        !value->contains("signed_in") || !value->at("signed_in").is_boolean()) {
      const auto found = response.find("error");
      const auto error = found != response.end() && found->is_string() ? control_text::wide(found->get<std::string>()) : L"";
      SetWindowTextW(get(statistics_upload_notice), statistics_model::upload_error_message(error,
          SendMessageW(get(statistics_upload), BM_GETCHECK, 0, 0) == BST_CHECKED).c_str());
      if (upload_) SendMessageW(get(statistics_upload), BM_SETCHECK, upload_->enabled ? BST_CHECKED : BST_UNCHECKED, 0);
      return;
    }
    statistics_model::UploadStatus status;
    status.enabled = value->at("enabled").get<bool>(); status.signed_in = value->at("signed_in").get<bool>();
    if (value->contains("last_upload_ms") && value->at("last_upload_ms").is_number_unsigned()) status.last_upload_ms = value->at("last_upload_ms").get<uint64_t>();
    upload_ = status;
    SendMessageW(get(statistics_upload), BM_SETCHECK, status.enabled ? BST_CHECKED : BST_UNCHECKED, 0);
    std::wstring notice(statistics_model::upload_description(status.signed_in, status.enabled));
    if (status.last_upload_ms) notice += L"\r\n上次上传：" + statistics_data::written_text(*status.last_upload_ms);
    const auto error = response.find("operation_error");
    if (error != response.end() && error->is_string()) notice += L"\r\n" + statistics_model::upload_error_message(control_text::wide(error->get<std::string>()),
        response.contains("operation_enabled") && response.at("operation_enabled").is_boolean() ? response.at("operation_enabled").get<bool>() : status.enabled);
    else if (changing) notice += L"\r\n" + std::wstring(statistics_model::upload_done_message(status.enabled));
    SetWindowTextW(get(statistics_upload_notice), notice.c_str());
    update();
  }
  Command command(WPARAM event) {
    const int id = LOWORD(event), notice = HIWORD(event);
    if (!controls_.count(id)) return {};
    Command result; result.handled = true;
    if (!enabled_ || !status_) return result;
    if (notice == CBN_SELCHANGE && (id == statistics_chart_kind || id == statistics_scope)) { update_chart(); return result; }
    if (id == statistics_days && notice == LBN_SELCHANGE) {
      const auto selected = SendMessageW(get(id), LB_GETCURSEL, 0, 0);
      if (selected >= 0 && static_cast<size_t>(selected) < days_.size()) selected_day_ = days_[static_cast<size_t>(selected)].key;
      update_day(); update_chart(); return result;
    }
    if (id == statistics_retention && notice == CBN_SELCHANGE) {
      const auto selected = SendMessageW(get(id), CB_GETCURSEL, 0, 0);
      if (selected < 0 || static_cast<size_t>(selected) >= statistics_model::retention_choices.size()) return result;
      const auto retention = statistics_model::retention_choices[static_cast<size_t>(selected)].value;
      if (retention == statistics_model::retention_value(status_->statistics.retention)) return result;
      result.action = Json{{"operation", "set_retention"}, {"retention", retention}, {"day", statistics_model::day_key(statistics_data::local_day())}};
      size_t current = 0;
      for (size_t index = 0; index < statistics_model::retention_choices.size(); ++index)
        if (statistics_model::retention_choices[index].value == statistics_model::retention_value(status_->statistics.retention)) current = index;
      if (selected > 0 && (current == 0 || static_cast<size_t>(selected) < current))
        result.confirmation = L"缩短保留期会删除期限之前的统计，且无法恢复。是否继续？";
      return result;
    }
    if (notice != BN_CLICKED) return result;
    if (id == statistics_enabled) result.action = Json{{"operation", "set_enabled"}, {"enabled", SendMessageW(get(id), BM_GETCHECK, 0, 0) == BST_CHECKED}};
    if (id == statistics_reset) {
      result.action = Json{{"operation", "reset"}};
      result.confirmation = L"清空全部本地打字统计？此操作无法恢复，不影响词库和学习记录。";
    }
    if (id == statistics_upload && upload_) {
      const bool on = SendMessageW(get(id), BM_GETCHECK, 0, 0) == BST_CHECKED;
      result.action = Json{{"operation", "typing_upload_set"}, {"enabled", on}}; result.upload = true;
      result.confirmation = on ? L"将每日输入聚合计数上传到水杉账号？不包含输入内容。" : L"关闭上传并删除账号云端的打字统计？";
    }
    return result;
  }
  void restore() {
    if (!status_) return;
    SendMessageW(get(statistics_enabled), BM_SETCHECK, status_->statistics.enabled ? BST_CHECKED : BST_UNCHECKED, 0);
    const auto retention = statistics_model::retention_value(status_->statistics.retention);
    for (size_t index = 0; index < statistics_model::retention_choices.size(); ++index)
      if (retention == statistics_model::retention_choices[index].value) SendMessageW(get(statistics_retention), CB_SETCURSEL, index, 0);
    if (upload_) SendMessageW(get(statistics_upload), BM_SETCHECK, upload_->enabled ? BST_CHECKED : BST_UNCHECKED, 0);
  }
  bool draw(const DRAWITEMSTRUCT& item) const;
private:
  HWND get(int id) const { const auto found = controls_.find(id); return found == controls_.end() ? nullptr : found->second; }
  std::optional<std::vector<std::string>> scope() const {
    const auto selected = SendMessageW(get(statistics_scope), CB_GETCURSEL, 0, 0);
    if (selected == 0) return std::nullopt;
    std::vector<std::string> keys;
    if (selected == 3) { if (selected_day_) keys.push_back(*selected_day_); return keys; }
    for (const auto& day : statistics_model::recent_days(selected == 1 ? 7 : 30, statistics_data::local_day())) keys.push_back(day.key);
    return keys;
  }
  void update_day() {
    std::wstring detail;
    if (selected_day_) for (const auto& row : days_) if (row.key == *selected_day_) {
      detail = control_text::wide(row.key) + L"\r\n总计：" + statistics_model::zh_number(row.total) + L" 字符\r\n汉字：" + statistics_model::zh_number(row.han) +
          L"\r\n拉丁字母：" + statistics_model::zh_number(row.latin) + L"\r\n数字：" + statistics_model::zh_number(row.number) +
          L"\r\n标点：" + statistics_model::zh_number(row.punctuation) + L"\r\n其他：" + statistics_model::zh_number(row.other);
      if (row.active_ms) detail += L"\r\n活跃时长：" + statistics_model::active_time(*row.active_ms);
      if (row.speed) detail += L"\r\n速度：" + statistics_model::zh_number(*row.speed) + L" 字 / 分钟";
    }
    SetWindowTextW(get(statistics_day_detail), detail.c_str());
  }
  void update() {
    if (!status_) return;
    const auto& statistics = status_->statistics;
    const bool upload = upload_ && upload_->enabled;
    SetWindowTextW(get(statistics_privacy), statistics_model::disabled_notice(upload).data());
    const auto today = statistics_model::day_key(statistics_data::local_day());
    if (!load_error_) SetWindowTextW(get(statistics_notice), statistics_model::availability_notice(statistics.enabled, statistics.total, status_->never_written,
        status_->last_written_ms ? statistics_data::written_text(*status_->last_written_ms) : L"").c_str());
    // 刷新保留已选日期；新日子和自动清理后的范围都来自同一份状态。
    days_ = statistics_model::daily_detail_rows(statistics, today, statistics.days.size());
    SendMessageW(get(statistics_days), LB_RESETCONTENT, 0, 0);
    bool found = false;
    for (size_t index = 0; index < days_.size(); ++index) {
      const auto text = control_text::wide(days_[index].key) + L" · " + statistics_model::zh_number(days_[index].total) + L" 字符";
      SendMessageW(get(statistics_days), LB_ADDSTRING, 0, reinterpret_cast<LPARAM>(text.c_str()));
      if (selected_day_ && *selected_day_ == days_[index].key) { SendMessageW(get(statistics_days), LB_SETCURSEL, index, 0); found = true; }
    }
    if (!found) selected_day_.reset();
    calendar_ = statistics_model::heatmap_weeks(statistics.days, statistics_data::local_day());
    calendar_labels_ = statistics_model::heatmap_month_labels(calendar_);
    std::vector<std::string> speed_keys;
    for (const auto& day : statistics_model::recent_days(statistics_model::desktop_trend_days, statistics_data::local_day())) speed_keys.push_back(day.key);
    speeds_ = statistics_model::daily_speeds(statistics, speed_keys);
    InvalidateRect(get(statistics_calendar), nullptr, TRUE);
    InvalidateRect(get(statistics_speed), nullptr, TRUE);
    update_day(); update_chart(); enable(enabled_);
  }
  void update_chart() {
    if (!status_) return;
    chart_rows_.clear();
    const auto kind = SendMessageW(get(statistics_chart_kind), CB_GETCURSEL, 0, 0);
    const auto& statistics = status_->statistics;
    const auto keys = scope();
    keyboard_ = statistics_model::keyboard_model(statistics_model::scoped_key_counts(statistics.daily_keys, keys));
    const auto detail = statistics_model::scoped_breakdown(statistics, keys);
    if (kind == 0) {
      for (const auto& key : statistics_model::scoped_key_counts(statistics.daily_keys, keys))
        chart_rows_.push_back({key.first, statistics_model::key_label(key.first), key.second, statistics_model::chart_palette[0], L""});
    } else if (kind == 1) {
      auto days = keys.value_or(std::vector<std::string>{});
      if (!keys) for (const auto& day : statistics_model::recent_days(statistics_model::desktop_trend_days, statistics_data::local_day())) days.push_back(day.key);
      for (const auto& day : days) chart_rows_.push_back({day, control_text::wide(day), statistics_model::value_of(statistics.days, day), statistics_model::chart_palette[0], L""});
    } else if (kind == 2) chart_rows_ = statistics_model::character_slices(detail);
    else if (kind == 3) chart_rows_ = statistics_model::language_slices(detail);
    else if (kind == 4) chart_rows_ = statistics_model::source_slices(detail);
    else if (kind == 5) {
      for (const auto& rank : statistics_model::candidate_ranks(statistics).rows) chart_rows_.push_back({"", rank.label, rank.count, statistics_model::chart_palette[0], L""});
    }
    const auto activity = statistics_model::activity_metrics(statistics, statistics_model::day_key(statistics_data::local_day()));
    const double scope_total = keys ? statistics_model::sum_values(statistics.days, *keys) : statistics.total;
    std::wstring overview;
    for (const auto& metric : statistics_model::overview_metrics(statistics, statistics_model::day_key(statistics_data::local_day()),
        keys ? L"范围输入" : L"累计输入", scope_total, activity))
      overview += metric.label + L"：" + metric.value + L" " + metric.unit + (metric.note.empty() ? L"" : L"（" + metric.note + L"）") + L"\r\n";
    SetWindowTextW(get(statistics_overview), overview.c_str());
    std::wstring text;
    for (const auto& row : chart_rows_) text += row.title + L"：" + statistics_model::zh_number(row.count) + L"\r\n";
    if (kind == 5) text = L"候选位次为整个保留期的计数。\r\n" + text;
    if (kind == 1) {
      const auto days = statistics_model::recent_days(statistics_model::desktop_trend_days, statistics_data::local_day());
      std::vector<std::string> day_keys; for (const auto& day : days) day_keys.push_back(day.key);
      const auto speeds = statistics_model::daily_speeds(statistics, day_keys);
      text += L"\r\n最近 30 天速度（字 / 分钟）\r\n";
      for (size_t index = 0; index < speeds.size(); ++index) if (speeds[index]) text += days[index].label + L"：" + statistics_model::zh_number(*speeds[index]) + L"\r\n";
    }
    SetWindowTextW(get(statistics_chart_text), text.empty() ? L"这个范围还没有统计数据。" : text.c_str());
    InvalidateRect(get(statistics_chart), nullptr, TRUE);
  }
  std::map<int, HWND> controls_;
  std::optional<statistics_data::Status> status_;
  std::optional<msime::settings::typing::UploadStatus> upload_;
  std::optional<std::string> selected_day_;
  std::vector<msime::settings::typing::DailyDetailRow> days_;
  std::vector<msime::settings::typing::Slice> chart_rows_;
  msime::settings::typing::KeyboardModel keyboard_;
  std::vector<std::array<statistics_model::HeatmapDay, 7>> calendar_;
  std::vector<std::wstring> calendar_labels_;
  std::vector<std::optional<double>> speeds_;
  bool enabled_ = false;
  bool load_error_ = false;
};

inline bool StatisticsForm::draw(const DRAWITEMSTRUCT& item) const {
  if (item.CtlID != statistics_chart && item.CtlID != statistics_calendar && item.CtlID != statistics_speed) return false;
  const auto dc = item.hDC;
  const int saved = SaveDC(dc);
  FillRect(dc, &item.rcItem, GetSysColorBrush(COLOR_WINDOW));
  SetBkMode(dc, TRANSPARENT); SetTextColor(dc, GetSysColor(COLOR_WINDOWTEXT));
  const auto font = reinterpret_cast<HFONT>(SendMessageW(get(statistics_chart), WM_GETFONT, 0, 0));
  if (font) SelectObject(dc, font);
  const auto fill = [&](RECT rect, COLORREF color) { const auto brush = CreateSolidBrush(color); FillRect(dc, &rect, brush); DeleteObject(brush); };
  const auto text = [&](std::wstring label, RECT rect) { DrawTextW(dc, label.c_str(), -1, &rect, DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX); };
  const auto rect = item.rcItem;
  const int width = rect.right - rect.left, height = rect.bottom - rect.top;
  if (width <= 0 || height <= 0) { RestoreDC(dc, saved); return true; }
  const auto kind = SendMessageW(get(statistics_chart_kind), CB_GETCURSEL, 0, 0);
  if (item.CtlID == statistics_calendar) {
    double maximum = 1;
    for (const auto& week : calendar_) for (const auto& day : week) maximum = std::max(maximum, day.count);
    const int count = static_cast<int>(calendar_.size());
    for (int week = 0; week < count; ++week) {
      const int left = rect.left + week * width / count;
      const int right = rect.left + (week + 1) * width / count;
      if (!calendar_labels_[static_cast<size_t>(week)].empty())
        text(calendar_labels_[static_cast<size_t>(week)], RECT{left, rect.top, std::min<LONG>(rect.right, left + 60), rect.top + 24});
      for (int row = 0; row < 7; ++row) {
        const auto& day = calendar_[static_cast<size_t>(week)][static_cast<size_t>(row)];
        if (day.future) continue;
        const auto level = statistics_model::day_heat_level(day.count, maximum);
        const int alpha = static_cast<int>(statistics_model::heat_opacity[static_cast<size_t>(level)] * 255);
        fill(RECT{left + 1, rect.top + 24 + row * (height - 24) / 7 + 1, right - 1, rect.top + 24 + (row + 1) * (height - 24) / 7 - 1},
            RGB(245 - alpha * 220 / 255, 247 - alpha * 80 / 255, 249 - alpha * 108 / 255));
      }
    }
  } else if (item.CtlID == statistics_speed) {
    double maximum = 1; bool measured = false;
    for (const auto& speed : speeds_) if (speed) { maximum = std::max(maximum, *speed); measured = true; }
    if (!measured) text(L"活跃满一分钟的日期才有速度记录。", rect);
    else {
      const int count = static_cast<int>(speeds_.size());
      for (int index = 0; index < count; ++index) if (speeds_[static_cast<size_t>(index)]) {
        const int bar_height = static_cast<int>(*speeds_[static_cast<size_t>(index)] / maximum * std::max(1, height - 28));
        const int left = rect.left + index * width / count;
        fill(RECT{left + 1, rect.bottom - 24 - bar_height, rect.left + (index + 1) * width / count - 1, rect.bottom - 24}, RGB(25,167,141));
      }
      text(L"30 天前 → 今天 · 最高 " + statistics_model::zh_number(maximum) + L" 字 / 分钟", RECT{rect.left, rect.bottom - 24, rect.right, rect.bottom});
    }
  } else if (kind == 0 && !keyboard_.rows.empty()) {
    auto rows = keyboard_.rows;
    if (keyboard_.nine) rows.insert(rows.end(), keyboard_.nine->begin(), keyboard_.nine->end());
    const int row_height = std::max(1, height / static_cast<int>(rows.size()));
    for (size_t index = 0; index < rows.size(); ++index) {
      const auto& row = rows[index];
      double total_weight = 0; for (const auto& key : row) total_weight += key.weight;
      double offset = 0;
      for (const auto& key : row) {
        const int left = rect.left + static_cast<int>(offset / total_weight * width);
        offset += key.weight;
        RECT cell{left + 1, rect.top + static_cast<int>(index) * row_height + 1,
            rect.left + static_cast<int>(offset / total_weight * width) - 1, rect.top + static_cast<int>(index + 1) * row_height - 1};
        const auto level = statistics_model::key_heat_level(key.count, keyboard_.maximum);
        const int alpha = static_cast<int>(statistics_model::heat_opacity[static_cast<size_t>(level)] * 255);
        fill(cell, RGB(245 - alpha * 220 / 255, 247 - alpha * 80 / 255, 249 - alpha * 108 / 255));
        text(key.label, cell);
      }
    }
  } else if (kind == 1 && !chart_rows_.empty()) {
    double maximum = 1; for (const auto& row : chart_rows_) maximum = std::max(maximum, row.count);
    const int count = static_cast<int>(chart_rows_.size());
    for (int index = 0; index < count; ++index) {
      const auto& row = chart_rows_[static_cast<size_t>(index)];
      const int left = rect.left + index * width / count;
      const int right = rect.left + (index + 1) * width / count;
      const int bar_height = static_cast<int>(row.count / maximum * std::max(1, height - 28));
      if (bar_height > 0) fill(RECT{left + 1, rect.bottom - 24 - bar_height, right - 1, rect.bottom - 24}, RGB(25,167,141));
      if (index % std::max(1, count / 5) == 0) text(statistics_model::day_label(row.id), RECT{left,rect.bottom - 24,std::min<LONG>(rect.right,left + 80),rect.bottom});
    }
  } else if (!chart_rows_.empty()) {
    double maximum = 1; for (const auto& row : chart_rows_) maximum = std::max(maximum, row.count);
    const auto count = std::min(chart_rows_.size(), size_t{32});
    const int row_height = std::max(1, height / static_cast<int>(count));
    for (size_t index = 0; index < count; ++index) {
      const auto& row = chart_rows_[index];
      const int top = rect.top + static_cast<int>(index) * row_height;
      const int label_width = width / 3;
      text(row.title, RECT{rect.left + 4, top, rect.left + label_width - 4, top + row_height});
      const int bar_width = static_cast<int>(row.count / maximum * (width - label_width - 8));
      if (bar_width > 0) fill(RECT{rect.left + label_width, top + 2, rect.left + label_width + bar_width, top + row_height - 2},
          RGB((row.color >> 16) & 255, (row.color >> 8) & 255, row.color & 255));
    }
  } else text(L"这个范围还没有统计数据。", rect);
  RestoreDC(dc, saved); return true;
}
} // namespace msime::settings::win7
