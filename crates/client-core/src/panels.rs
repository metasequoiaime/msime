//! Host-independent contracts for the optional keyboard and handwriting panels.
//!
//! The panel UI owns presentation state. A platform host injects the operations
//! that can observe the previous foreground window, inject key events, perform
//! handwriting recognition, and submit a selected candidate.

use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAX_LANGUAGE_BYTES: usize = 32;
const MAX_STROKES: usize = 64;
const MAX_POINTS_PER_STROKE: usize = 4096;
const MAX_CANDIDATES: usize = 12;
const MAX_CANDIDATE_BYTES: usize = 256;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PanelContractError {
    #[error("virtual key is invalid")]
    InvalidVirtualKey,
    #[error("keyboard input contains an unsupported modifier")]
    InvalidKeyboardInput,
    #[error("handwriting language is invalid")]
    InvalidLanguage,
    #[error("handwriting stroke data is invalid")]
    InvalidStroke,
    #[error("too many handwriting strokes")]
    TooManyStrokes,
    #[error("handwriting stroke is too long")]
    StrokeTooLong,
    #[error("candidate list is too long")]
    TooManyCandidates,
    #[error("handwriting candidate is invalid")]
    InvalidCandidate,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct KeyboardModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub win: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ClientFocusLease {
    pub client: u64,
    pub epoch: u64,
    pub token: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ClientKeyEvent {
    pub lease: ClientFocusLease,
    pub virtual_key: u32,
    pub scan_code: u32,
    pub modifiers: u32,
    pub character: char,
    pub ui_less: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientKeyDispatchResult {
    Sent,
    DefinitelyNotSent,
    DeliveryAmbiguous,
}

impl ClientKeyDispatchResult {
    pub fn allows_fallback(self) -> bool {
        matches!(self, Self::DefinitelyNotSent)
    }
}

impl ClientKeyEvent {
    pub fn validate(&self) -> Result<(), PanelContractError> {
        if self.lease.client == 0
            || self.lease.epoch == 0
            || self.lease.token == 0
            || self.virtual_key > 0xff
            || self.modifiers & !0x0f != 0
        {
            return Err(PanelContractError::InvalidKeyboardInput);
        }
        Ok(())
    }
}

pub trait ClientKeyRouter {
    type Error;
    fn dispatch(&mut self, event: &ClientKeyEvent) -> Result<bool, Self::Error>;
    fn cancel(&mut self, lease: &ClientFocusLease) -> Result<bool, Self::Error>;
}

/// Guards a platform router against events from a previous focus/session.
/// Hosts update the lease on focus-in and invalidate it on focus-out.
pub struct LeasedClientKeyRouter<R> {
    router: R,
    lease: Option<ClientFocusLease>,
}

impl<R> LeasedClientKeyRouter<R> {
    pub fn new(router: R) -> Self {
        Self {
            router,
            lease: None,
        }
    }

    pub fn set_lease(&mut self, lease: ClientFocusLease) {
        self.lease = Some(lease);
    }

    pub fn into_inner(self) -> R {
        self.router
    }
}

impl<R: ClientKeyRouter> ClientKeyRouter for LeasedClientKeyRouter<R> {
    type Error = R::Error;

    fn dispatch(&mut self, event: &ClientKeyEvent) -> Result<bool, Self::Error> {
        if event.validate().is_err() || self.lease != Some(event.lease) {
            return Ok(false);
        }
        self.router.dispatch(event)
    }

    fn cancel(&mut self, lease: &ClientFocusLease) -> Result<bool, Self::Error> {
        if self.lease != Some(*lease) {
            return Ok(false);
        }
        self.lease = None;
        self.router.cancel(lease)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KeyboardInputRequest {
    /// Windows virtual-key value. Other hosts may map this value to their own
    /// native key event while keeping the panel contract stable.
    pub virtual_key: u16,
    pub shift: bool,
    pub modifiers: KeyboardModifiers,
    /// Commit/navigation keys must not inherit sticky Ctrl/Alt/Win state.
    pub include_sticky_modifiers: bool,
}

impl KeyboardInputRequest {
    pub fn validate(&self) -> Result<(), PanelContractError> {
        if self.virtual_key == 0 || self.virtual_key > 0xff {
            return Err(PanelContractError::InvalidVirtualKey);
        }
        Ok(())
    }
}

pub trait KeyboardInputSink {
    type Error;

    /// Record the foreground input window before the panel receives focus.
    fn remember_input_target(&mut self) -> Result<(), Self::Error>;

    /// Inject one complete key stroke into the remembered target.
    fn send_key(&mut self, request: &KeyboardInputRequest) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct InkPoint {
    pub x: f32,
    pub y: f32,
}

impl InkPoint {
    fn validate(&self) -> Result<(), PanelContractError> {
        if self.x.is_finite()
            && self.y.is_finite()
            && self.x.abs() <= 4096.0
            && self.y.abs() <= 4096.0
        {
            Ok(())
        } else {
            Err(PanelContractError::InvalidStroke)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InkStroke {
    pub points: Vec<InkPoint>,
}

impl InkStroke {
    fn validate(&self) -> Result<(), PanelContractError> {
        if self.points.is_empty() {
            return Err(PanelContractError::InvalidStroke);
        }
        if self.points.len() > MAX_POINTS_PER_STROKE {
            return Err(PanelContractError::StrokeTooLong);
        }
        self.points.iter().try_for_each(InkPoint::validate)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HandwritingRecognitionRequest {
    #[serde(default = "default_language")]
    pub language: String,
    pub strokes: Vec<InkStroke>,
}

fn default_language() -> String {
    "zh-CN".to_owned()
}

impl HandwritingRecognitionRequest {
    pub fn validate(&self) -> Result<(), PanelContractError> {
        if self.language.is_empty() || self.language.len() > MAX_LANGUAGE_BYTES {
            return Err(PanelContractError::InvalidLanguage);
        }
        if self.strokes.is_empty() {
            return Err(PanelContractError::InvalidStroke);
        }
        if self.strokes.len() > MAX_STROKES {
            return Err(PanelContractError::TooManyStrokes);
        }
        self.strokes.iter().try_for_each(InkStroke::validate)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HandwritingRecognitionResult {
    pub candidates: Vec<String>,
}

impl HandwritingRecognitionResult {
    pub fn validate(&self) -> Result<(), PanelContractError> {
        if self.candidates.len() > MAX_CANDIDATES {
            return Err(PanelContractError::TooManyCandidates);
        }
        for candidate in &self.candidates {
            validate_candidate(candidate)?;
        }
        Ok(())
    }
}

pub trait HandwritingPlatform {
    type Error;

    fn recognize(
        &mut self,
        request: &HandwritingRecognitionRequest,
    ) -> Result<HandwritingRecognitionResult, Self::Error>;

    fn submit_candidate(&mut self, candidate: &str) -> Result<(), Self::Error>;
}

pub fn validate_candidate(candidate: &str) -> Result<(), PanelContractError> {
    if candidate.is_empty() || !crate::text::is_bounded_text(candidate, MAX_CANDIDATE_BYTES) {
        Err(PanelContractError::InvalidCandidate)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Router(bool);
    impl ClientKeyRouter for Router {
        type Error = ();
        fn dispatch(&mut self, _: &ClientKeyEvent) -> Result<bool, Self::Error> {
            self.0 = true;
            Ok(true)
        }
        fn cancel(&mut self, _: &ClientFocusLease) -> Result<bool, Self::Error> {
            self.0 = false;
            Ok(true)
        }
    }

    #[test]
    fn leased_router_rejects_stale_events_and_clears_on_cancel() {
        let lease = ClientFocusLease {
            client: 1,
            epoch: 2,
            token: 3,
        };
        let mut router = LeasedClientKeyRouter::new(Router(false));
        router.set_lease(lease);
        let event = ClientKeyEvent {
            lease,
            virtual_key: 0x41,
            scan_code: 30,
            modifiers: 0,
            character: 'a',
            ui_less: false,
        };
        assert!(router.dispatch(&event).unwrap());
        let stale = ClientKeyEvent {
            lease: ClientFocusLease { epoch: 1, ..lease },
            ..event
        };
        assert!(!router.dispatch(&stale).unwrap());
        assert!(router.cancel(&lease).unwrap());
        assert!(!router.cancel(&lease).unwrap());
    }

    fn stroke() -> InkStroke {
        InkStroke {
            points: vec![InkPoint { x: 1.0, y: 2.0 }, InkPoint { x: 3.0, y: 4.0 }],
        }
    }

    #[test]
    fn client_key_event_validates_lease_key_and_modifiers() {
        let mut event = ClientKeyEvent {
            lease: ClientFocusLease {
                client: 1,
                epoch: 2,
                token: 3,
            },
            virtual_key: 0x41,
            scan_code: 30,
            modifiers: 0x0f,
            character: 'a',
            ui_less: false,
        };
        assert!(event.validate().is_ok());
        event.lease.token = 0;
        assert_eq!(
            event.validate(),
            Err(PanelContractError::InvalidKeyboardInput)
        );
        event.lease.token = 3;
        event.virtual_key = 0x100;
        assert_eq!(
            event.validate(),
            Err(PanelContractError::InvalidKeyboardInput)
        );
        event.virtual_key = 0x41;
        event.modifiers = 0x10;
        assert_eq!(
            event.validate(),
            Err(PanelContractError::InvalidKeyboardInput)
        );
    }

    #[test]
    fn only_definite_non_delivery_allows_fallback() {
        assert!(!ClientKeyDispatchResult::Sent.allows_fallback());
        assert!(ClientKeyDispatchResult::DefinitelyNotSent.allows_fallback());
        assert!(!ClientKeyDispatchResult::DeliveryAmbiguous.allows_fallback());
    }

    #[test]
    fn keyboard_request_rejects_invalid_keys_and_sticky_modifier_mismatch() {
        let mut request = KeyboardInputRequest {
            virtual_key: 0,
            shift: false,
            modifiers: KeyboardModifiers::default(),
            include_sticky_modifiers: true,
        };
        assert_eq!(
            request.validate(),
            Err(PanelContractError::InvalidVirtualKey)
        );
        request.virtual_key = 0x20;
        request.modifiers.ctrl = true;
        request.include_sticky_modifiers = false;
        assert!(request.validate().is_ok());
    }

    #[test]
    fn handwriting_request_and_result_are_bounded() {
        let request = HandwritingRecognitionRequest {
            language: "zh-CN".into(),
            strokes: vec![stroke()],
        };
        assert!(request.validate().is_ok());
        let result = HandwritingRecognitionResult {
            candidates: vec!["水".into(), "永".into()],
        };
        assert!(result.validate().is_ok());

        let mut invalid = request;
        invalid.strokes[0].points[0].x = f32::NAN;
        assert_eq!(invalid.validate(), Err(PanelContractError::InvalidStroke));
        assert_eq!(
            validate_candidate("line\nfeed"),
            Err(PanelContractError::InvalidCandidate)
        );
    }
}
