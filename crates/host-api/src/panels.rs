//! Host-side validation and dispatch for platform panel capabilities.

use msime_client_core::panels::{
    HandwritingPlatform, HandwritingRecognitionRequest, HandwritingRecognitionResult,
    KeyboardInputRequest, KeyboardInputSink, PanelContractError,
};

pub fn remember_keyboard_input_target<S: KeyboardInputSink>(
    sink: &mut S,
) -> Result<(), PanelContractError> {
    sink.remember_input_target()
        .map_err(|_| PanelContractError::InvalidKeyboardInput)
}

pub fn send_keyboard_input<S: KeyboardInputSink>(
    sink: &mut S,
    request: &KeyboardInputRequest,
) -> Result<(), PanelContractError> {
    request.validate()?;
    sink.send_key(request)
        .map_err(|_| PanelContractError::InvalidKeyboardInput)
}

pub fn recognize_handwriting<H: HandwritingPlatform>(
    platform: &mut H,
    request: &HandwritingRecognitionRequest,
) -> Result<HandwritingRecognitionResult, PanelContractError> {
    request.validate()?;
    let result = platform
        .recognize(request)
        .map_err(|_| PanelContractError::InvalidStroke)?;
    result.validate()?;
    Ok(result)
}

pub fn submit_handwriting_candidate<H: HandwritingPlatform>(
    platform: &mut H,
    candidate: &str,
) -> Result<(), PanelContractError> {
    msime_client_core::panels::validate_candidate(candidate)?;
    platform
        .submit_candidate(candidate)
        .map_err(|_| PanelContractError::InvalidCandidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use msime_client_core::panels::{InkPoint, InkStroke, KeyboardModifiers};

    struct KeyboardStub {
        remembered: bool,
        sent: Vec<KeyboardInputRequest>,
    }

    impl KeyboardInputSink for KeyboardStub {
        type Error = ();

        fn remember_input_target(&mut self) -> Result<(), Self::Error> {
            self.remembered = true;
            Ok(())
        }

        fn send_key(&mut self, request: &KeyboardInputRequest) -> Result<(), Self::Error> {
            self.sent.push(request.clone());
            Ok(())
        }
    }

    struct HandwritingStub;

    impl HandwritingPlatform for HandwritingStub {
        type Error = ();

        fn recognize(
            &mut self,
            _: &HandwritingRecognitionRequest,
        ) -> Result<HandwritingRecognitionResult, Self::Error> {
            Ok(HandwritingRecognitionResult {
                candidates: vec!["水".into()],
            })
        }

        fn submit_candidate(&mut self, _: &str) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    #[test]
    fn dispatches_only_validated_keyboard_input() {
        let mut stub = KeyboardStub {
            remembered: false,
            sent: Vec::new(),
        };
        let request = KeyboardInputRequest {
            virtual_key: 0x41,
            shift: true,
            modifiers: KeyboardModifiers::default(),
            include_sticky_modifiers: true,
            grouped_letter: None,
        };
        assert!(remember_keyboard_input_target(&mut stub).is_ok());
        assert!(stub.remembered);
        assert!(send_keyboard_input(&mut stub, &request).is_ok());
        assert_eq!(stub.sent, vec![request]);
    }

    #[test]
    fn dispatches_handwriting_and_candidate_submission() {
        let request = HandwritingRecognitionRequest {
            language: "zh-CN".into(),
            strokes: vec![InkStroke {
                points: vec![InkPoint { x: 1.0, y: 1.0 }],
            }],
        };
        let mut stub = HandwritingStub;
        assert_eq!(
            recognize_handwriting(&mut stub, &request)
                .unwrap()
                .candidates,
            vec!["水"]
        );
        assert!(submit_handwriting_candidate(&mut stub, "水").is_ok());
    }
}
