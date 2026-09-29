//! The engine boundary: the trait the runtime drives, and the pinned C++ Session's
//! implementation of it.

use super::*;

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("candidate page size must be between 1 and 9")]
    InvalidPageSize,
    #[error("candidate belongs to an expired view or another session")]
    StaleCandidate,
    #[error("nine-key spelling belongs to an expired view or another session")]
    StaleNineKeySpelling,
    #[error("session identity exhausted")]
    IdentityExhausted,
    #[error("cannot replace an engine while composition is active")]
    CompositionActive,
    #[error("nine-key mode requires the quanpin scheme")]
    InvalidNineKeyScheme,
    #[error("punctuation action requires an ASCII punctuation character")]
    InvalidPunctuation,
    #[error("engine action failed: {0}")]
    Engine(String),
}

pub trait InputEngine {
    /// Ends the committed-word context the Engine learns and ranks against. Engines without one ignore it.
    fn reset_context(&mut self) {}
    /// Moves the caret prefix decoding answers for; `None` returns to the end of the composition. Engines without
    /// prefix decoding ignore it and report nothing pending below.
    fn set_caret(&mut self, _caret: Option<usize>) {}
    fn prefix_end(&self) -> usize {
        0
    }
    fn pending_suffix(&self) -> String {
        String::new()
    }
    fn reset_cache(&mut self) -> Result<(), RuntimeError> {
        Err(RuntimeError::Engine(
            "Engine cache reset is unsupported".into(),
        ))
    }
    fn set_paired_punctuation_enabled(&mut self, _enabled: bool) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn balance_paired_punctuation_after_auto_close(
        &mut self,
        _opening: u8,
    ) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn set_punctuation_lock(&mut self, _lock: u8) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn set_dedicated_english(&mut self, _enabled: bool) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn set_nine_key_enabled(&mut self, _enabled: bool) -> Result<(), RuntimeError> {
        Err(RuntimeError::Engine("Nine-key mode is unsupported".into()))
    }
    /// Ask for candidates withheld from the first answer, reporting whether the list grew. The
    /// default answers no, which is what an engine that already returns everything it has means.
    fn expand_initial_candidates(&mut self) -> Result<bool, RuntimeError> {
        Ok(false)
    }
    fn choose_nine_key_spelling(&mut self, _index: usize) -> Result<EngineResult, RuntimeError> {
        Err(RuntimeError::Engine(
            "Nine-key spelling selection is unsupported".into(),
        ))
    }
    fn snapshot(&self) -> Result<EngineSnapshot, RuntimeError>;
    fn character(&mut self, value: u8, shift: bool) -> Result<EngineResult, RuntimeError>;
    fn command(&mut self, command: Command) -> Result<EngineResult, RuntimeError>;
    fn segment_command(&mut self, command: SegmentCommand) -> Result<EngineResult, RuntimeError> {
        let snapshot = self.snapshot()?;
        let has_segment_boundaries = !snapshot.segment_raw_boundaries.is_empty();
        let fallback = match command {
            SegmentCommand::Backspace => Command::Backspace,
            SegmentCommand::MoveLeft => Command::MoveLeft,
            SegmentCommand::MoveRight => Command::MoveRight,
        };
        let caret = snapshot.caret_position as u64;
        let steps = match command {
            SegmentCommand::Backspace | SegmentCommand::MoveLeft => snapshot
                .segment_raw_boundaries
                .iter()
                .rev()
                .find(|&&boundary| boundary < caret)
                .map_or(1, |&boundary| (caret - boundary) as usize),
            SegmentCommand::MoveRight => snapshot
                .segment_raw_boundaries
                .iter()
                .find(|&&boundary| boundary > caret)
                .map_or(1, |&boundary| (boundary - caret) as usize),
        };
        let mut result = EngineResult {
            handled: false,
            has_commit: false,
            commit: String::new(),
            diagnostic: String::new(),
        };
        for _ in 0..steps {
            result = self.command(fallback)?;
        }
        if matches!(command, SegmentCommand::Backspace) && has_segment_boundaries {
            let after = self.snapshot()?;
            if needs_dangling_segment_delimiter_backspace(&after.editing_text, after.caret_position)
            {
                // The boundary is owned by Engine; remove only the separator
                // left adjacent to the new caret through Engine's command path.
                result = self.command(Command::Backspace)?;
            }
        }
        Ok(result)
    }
    fn select(&mut self, index: usize) -> Result<EngineResult, RuntimeError>;
    fn pin_candidate(&mut self, _index: usize) -> Result<EngineResult, RuntimeError> {
        Err(RuntimeError::Engine(
            "Candidate pinning is unsupported".into(),
        ))
    }
    fn remove_candidate(&mut self, _index: usize) -> Result<EngineResult, RuntimeError> {
        Err(RuntimeError::Engine(
            "Candidate removal is unsupported".into(),
        ))
    }
    fn fix_candidate_position(
        &mut self,
        _index: usize,
        _position: u8,
    ) -> Result<EngineResult, RuntimeError> {
        Err(RuntimeError::Engine(
            "Candidate position fixing is unsupported".into(),
        ))
    }
    fn clear_candidate_position(&mut self, _index: usize) -> Result<EngineResult, RuntimeError> {
        Err(RuntimeError::Engine(
            "Candidate position clearing is unsupported".into(),
        ))
    }
    fn select_edge(
        &mut self,
        index: usize,
        edge: CandidateEdge,
    ) -> Result<EngineResult, RuntimeError>;
    fn finish(&mut self, index: usize) -> Result<EngineResult, RuntimeError>;
    fn punctuation(&mut self, value: u8) -> Result<EngineResult, RuntimeError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentCommand {
    Backspace,
    MoveLeft,
    MoveRight,
}

impl InputEngine for Session {
    fn reset_context(&mut self) {
        Session::reset_context(self);
    }
    fn set_caret(&mut self, caret: Option<usize>) {
        Session::set_caret(self, caret);
    }
    fn prefix_end(&self) -> usize {
        Session::prefix_end(self)
    }
    fn pending_suffix(&self) -> String {
        Session::pending_suffix(self)
    }
    fn reset_cache(&mut self) -> Result<(), RuntimeError> {
        Session::reset_cache(self);
        Ok(())
    }
    fn set_paired_punctuation_enabled(&mut self, enabled: bool) -> Result<(), RuntimeError> {
        Session::set_paired_punctuation_enabled(self, enabled)
            .map_err(|e| RuntimeError::Engine(e.to_string()))
    }
    fn balance_paired_punctuation_after_auto_close(
        &mut self,
        opening: u8,
    ) -> Result<(), RuntimeError> {
        Session::balance_paired_punctuation_after_auto_close(self, opening)
            .map_err(|e| RuntimeError::Engine(e.to_string()))
    }
    fn set_punctuation_lock(&mut self, lock: u8) -> Result<(), RuntimeError> {
        Session::set_punctuation_lock(self, lock).map_err(|e| RuntimeError::Engine(e.to_string()))
    }
    fn set_dedicated_english(&mut self, enabled: bool) -> Result<(), RuntimeError> {
        Session::set_dedicated_english(self, enabled)
            .map_err(|e| RuntimeError::Engine(e.to_string()))
    }
    fn set_nine_key_enabled(&mut self, enabled: bool) -> Result<(), RuntimeError> {
        Session::set_nine_key_enabled(self, enabled)
            .map_err(|e| RuntimeError::Engine(e.to_string()))
    }
    fn expand_initial_candidates(&mut self) -> Result<bool, RuntimeError> {
        Session::expand_initial_candidates(self).map_err(|e| RuntimeError::Engine(e.to_string()))
    }
    fn choose_nine_key_spelling(&mut self, index: usize) -> Result<EngineResult, RuntimeError> {
        Session::choose_nine_key_spelling(self, index)
            .map_err(|e| RuntimeError::Engine(e.to_string()))
    }
    fn punctuation(&mut self, value: u8) -> Result<EngineResult, RuntimeError> {
        Session::punctuation(self, value).map_err(|error| RuntimeError::Engine(error.to_string()))
    }
    fn snapshot(&self) -> Result<EngineSnapshot, RuntimeError> {
        Session::snapshot(self).map_err(|error| RuntimeError::Engine(error.to_string()))
    }
    fn character(&mut self, value: u8, shift: bool) -> Result<EngineResult, RuntimeError> {
        Session::character(self, value, shift)
            .map_err(|error| RuntimeError::Engine(error.to_string()))
    }
    fn command(&mut self, command: Command) -> Result<EngineResult, RuntimeError> {
        Session::command(self, command).map_err(|error| RuntimeError::Engine(error.to_string()))
    }
    fn select(&mut self, index: usize) -> Result<EngineResult, RuntimeError> {
        Session::select(self, index).map_err(|error| RuntimeError::Engine(error.to_string()))
    }
    fn pin_candidate(&mut self, index: usize) -> Result<EngineResult, RuntimeError> {
        Session::pin_candidate(self, index).map_err(|e| RuntimeError::Engine(e.to_string()))
    }
    fn remove_candidate(&mut self, index: usize) -> Result<EngineResult, RuntimeError> {
        Session::remove_candidate(self, index).map_err(|e| RuntimeError::Engine(e.to_string()))
    }
    fn fix_candidate_position(
        &mut self,
        index: usize,
        position: u8,
    ) -> Result<EngineResult, RuntimeError> {
        Session::fix_candidate_position(self, index, position)
            .map_err(|e| RuntimeError::Engine(e.to_string()))
    }
    fn clear_candidate_position(&mut self, index: usize) -> Result<EngineResult, RuntimeError> {
        Session::clear_candidate_position(self, index)
            .map_err(|e| RuntimeError::Engine(e.to_string()))
    }
    fn finish(&mut self, index: usize) -> Result<EngineResult, RuntimeError> {
        Session::finish(self, index).map_err(|error| RuntimeError::Engine(error.to_string()))
    }
    fn select_edge(
        &mut self,
        index: usize,
        edge: CandidateEdge,
    ) -> Result<EngineResult, RuntimeError> {
        Session::select_edge(self, index, edge)
            .map_err(|error| RuntimeError::Engine(error.to_string()))
    }
}
