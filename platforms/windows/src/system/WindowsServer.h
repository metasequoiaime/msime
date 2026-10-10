#pragma once
#include "PipeMainTransport.h"
#include "PipeService.h"
#include "SessionController.h"

namespace msime::windows {
struct WindowsServerOptions {
  PipeServiceOptions pipes; // Explicit names/capabilities, no product defaults.
  size_t registration_capacity = 64;
  size_t input_capacity = 256;
  DWORD write_timeout = 250;
  std::string
      preferences_directory; // Explicit shared store; empty disables polling.
  PreferenceMonitor::Published preferences_published;
};
// Starts an actual native service when constructed. The caller must explicitly
// choose names and implement native key/UI behavior; this never registers TSF.
// Callbacks may run before construction returns; captured dependencies must
// already exist, and must not access this server until construction completes.
class WindowsServer final {
public:
  WindowsServer(WindowsServerOptions options, std::string host_options,
                SessionPump::KeyHandler key, SessionPump::EventHandler event,
                SessionPump::Presentation presentation = {});
  ~WindowsServer();
  WindowsServer(const WindowsServer &) = delete;
  WindowsServer &operator=(const WindowsServer &) = delete;
  void request_stop() { controller_->request_stop(); }
  void stop() { controller_->stop(); }
  ControllerFailure failure() const { return controller_->failure(); }
  std::optional<CandidatePresentation> candidate_view() {
    return controller_->candidate_view();
  }
  void wait_candidate_render_for_key(const FocusLease &lease,
                                     const FanyImeNamedpipeData &packet) {
    controller_->wait_candidate_render_for_key(lease, packet);
  }
  void candidate_rendered(const FocusLease &lease, uint64_t generation) {
    controller_->candidate_rendered(lease, generation);
  }
  std::optional<ModePresentation> mode_view() { return controller_->mode_view(); }
  bool mode_active() { return controller_->mode_active(); }
  bool focus_current(const FocusLease &lease) {
    return controller_->focus_current(lease);
  }
  SelectionRequestResult request_selection(const FocusLease &lease,
      uint64_t session, uint64_t generation, size_t index) {
    return controller_->request_selection(lease, session, generation, index);
  }
  CandidateActionRequestResult request_candidate_action(
      const FocusLease &lease, uint64_t session, uint64_t generation,
      size_t index, CandidateAction action, uint8_t position = 0) {
    return controller_->request_candidate_action(lease, session, generation,
                                                 index, action, position);
  }
  CandidatePageRequestResult request_page(const CandidatePage &page) {
    return controller_->request_page(page);
  }
  // The Aux pipe's TerminalDeactivation fallback; see SessionController.
  bool deactivate_terminal(uint64_t client, uint64_t token) {
    return controller_->deactivate_terminal(client, token);
  }
  // Aux 管道的 KeySound；见 SessionController。
  void passthrough_key(uint64_t client, uint64_t token, uint32_t key_class) {
    controller_->passthrough_key(client, token, key_class);
  }
  // Dictionary maintenance handshake; see SessionController.
  bool quiesce_dictionaries() { return controller_->quiesce_dictionaries(); }
  bool resume_dictionaries() { return controller_->resume_dictionaries(); }
  ModeRequestResult request_mode(const FocusLease &lease, WorkerMode mode) {
    return controller_->request_mode(lease, mode);
  }
  std::optional<bool> dedicated_english_state(const FocusLease &lease) {
    return controller_->dedicated_english_state(lease);
  }
  bool exit_dedicated_english(const FocusLease &lease) {
    return controller_->exit_dedicated_english(lease);
  }
  bool set_dedicated_english(const FocusLease &lease, bool enabled) {
    return controller_->set_dedicated_english(lease, enabled);
  }
  std::vector<PipeTicket> current_tsf_tickets() {
    return transport_->current_tickets();
  }
  bool send_tsf_config(const TsfLocalConfig &config) {
    return controller_->send_tsf_config(config);
  }
  bool reset_cache() { return controller_->reset_cache(); }
  bool send_caps_lock(const FocusLease &lease, bool enabled) {
    return controller_->send_caps_lock(lease, enabled);
  }
  VoiceCompositionResult send_voice_composition(
      const FocusLease &lease, uint32_t message, std::wstring_view text,
      wchar_t generation) {
    return controller_->send_voice_composition(lease, message, text, generation);
  }
  std::optional<PreferenceMonitorStatus> preferences_status() const {
    return controller_->preferences_status();
  }

private:
  RegistrationInbox inbox_;
  std::unique_ptr<PipeService> service_;
  std::unique_ptr<PipeMainTransport> transport_;
  std::unique_ptr<SessionController> controller_;
};
} // namespace msime::windows
