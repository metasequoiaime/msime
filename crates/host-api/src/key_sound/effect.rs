//! Typing effects: each session's combo, and the one integer `msime_client_typing_effect` answers a host with.
//!
//! The host draws; this only counts. Everything here is integer arithmetic on the session's own `Cell`, so the call costs the key path no lock, no allocation and no disk. The only thing it may queue is the tier-up sound, through the same bounded `try_send` the key sounds use.

use super::{tier_up, KeyClass, SessionSound, SoundSettings};
use msime_client_services::plugins::{EffectStyle, COMBO_IDLE_RESET_MILLIS, COMBO_MILESTONES};
use std::cell::Cell;
use std::time::{Duration, Instant};

/// Event codes, in the low byte of the call's `event`. 0-3 are `KeyClass` codes, as `msime_client_key_sound` takes them.
pub(crate) const EVENT_COMMIT: u32 = 4;
pub(crate) const EVENT_BACKSPACE: u32 = 5;
/// The low byte of `event` that holds the code.
const EVENT_CODE: u32 = 0xFF;
/// Flag: the key is an auto-repeat of a held key. It is drawn but not counted.
pub(crate) const FLAG_AUTO_REPEAT: u32 = 1 << 8;
/// Flag: the host's own rules keep sounds quiet right now (a full-screen foreground application on Windows). The tier-up sound is neither queued nor reported due.
pub(crate) const FLAG_MUTED: u32 = 1 << 9;

/// Answer bits 0-15: the combo count, saturating at 65535; 0 while the combo counter is off.
pub(crate) const ANSWER_COUNT: u32 = 0xFFFF;
/// Answer bit 16: this key moved the combo up a tier.
pub(crate) const ANSWER_TIER_UP: u32 = 1 << 16;
/// Answer bits 17-19: `EffectStyle::code` of the resolved effect, the selected effect pack's style when there is one.
pub(crate) const ANSWER_STYLE_SHIFT: u32 = 17;
/// Answer bit 20: the tier-up sound is due. The macOS, Windows and Linux player has already queued it; a host that plays packs itself plays the key pack's commit sample raised by `TIER_SEMITONES` per tier.
pub(crate) const ANSWER_TIER_SOUND: u32 = 1 << 20;

/// Semitones the tier-up sound rises per tier, so the fourth tier is an octave up.
pub(crate) const TIER_SEMITONES: u8 = 3;

/// A run of keys without a pause or a backspace.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Combo {
    count: u32,
    last: Option<Instant>,
    /// How many of `COMBO_MILESTONES` the count has reached.
    tier: u8,
}

impl Combo {
    /// Start over when the keyboard has been quiet for `COMBO_IDLE_RESET_MILLIS`.
    fn expire(&mut self, now: Instant) {
        let idle = Duration::from_millis(COMBO_IDLE_RESET_MILLIS);
        if self
            .last
            .is_some_and(|last| now.saturating_duration_since(last) >= idle)
        {
            *self = Self::default();
        }
    }

    /// Count one key. True when the key reached a milestone.
    fn key(&mut self, now: Instant) -> bool {
        self.count = self.count.saturating_add(1);
        self.last = Some(now);
        let tier = COMBO_MILESTONES
            .iter()
            .filter(|milestone| self.count >= **milestone)
            .count() as u8;
        let reached = tier > self.tier;
        self.tier = tier;
        reached
    }
}

/// Count `event` into the session's combo and pack what the host draws, queueing the tier-up sound when it is due. 0 when the typing effect and the combo counter are both off, and for an event code it does not know.
pub(crate) fn typing_effect(sound: &SessionSound, event: u32, now: Instant) -> u32 {
    let answer = advance(&sound.settings, &sound.combo, event, now);
    if answer & ANSWER_TIER_SOUND != 0 {
        tier_up(sound, sound.combo.get().tier);
    }
    answer
}

/// `typing_effect` without the sound: count `event` into `combo` under `settings` and pack the answer.
fn advance(settings: &SoundSettings, combo: &Cell<Combo>, event: u32, now: Instant) -> u32 {
    let code = event & EVENT_CODE;
    if code > EVENT_BACKSPACE {
        return 0;
    }
    if settings.effect.style == EffectStyle::Off && !settings.combo_counter {
        combo.set(Combo::default());
        return 0;
    }
    let mut current = combo.get();
    current.expire(now);
    let mut reached = false;
    if !settings.combo_counter || code == KeyClass::Backspace as u32 || code == EVENT_BACKSPACE {
        current = Combo::default();
    } else if code != EVENT_COMMIT && event & FLAG_AUTO_REPEAT == 0 {
        reached = current.key(now);
    }
    combo.set(current);
    let mut answer =
        current.count.min(ANSWER_COUNT) | settings.effect.style.code() << ANSWER_STYLE_SHIFT;
    if reached {
        answer |= ANSWER_TIER_UP;
        if settings.tier_sound() && event & FLAG_MUTED == 0 {
            answer |= ANSWER_TIER_SOUND;
        }
    }
    answer
}

#[cfg(test)]
mod tests {
    use super::*;
    use msime_client_services::plugins::effect_pack::TypingEffect;

    /// Settings and a combo, standing in for a session: a real `SessionSound` publishes process-wide settings and a due tier-up sound would start the process's player under the other tests.
    struct Session {
        settings: SoundSettings,
        combo: Cell<Combo>,
    }

    fn session(style: EffectStyle, counter: bool, tier_sound: bool) -> Session {
        Session {
            settings: SoundSettings {
                effect: TypingEffect::from_preferences(style, 50),
                combo_counter: counter,
                combo_tier_sound: tier_sound,
                ..SoundSettings::default()
            },
            combo: Cell::default(),
        }
    }

    fn typing_effect(session: &Session, event: u32, now: Instant) -> u32 {
        advance(&session.settings, &session.combo, event, now)
    }

    fn count(answer: u32) -> u32 {
        answer & ANSWER_COUNT
    }

    #[test]
    fn effects_off_answer_nothing_and_keep_no_combo() {
        let sound = session(EffectStyle::Off, false, true);
        let now = Instant::now();
        for code in 0..=EVENT_BACKSPACE {
            assert_eq!(typing_effect(&sound, code, now), 0, "{code}");
        }
        assert_eq!(sound.combo.get(), Combo::default());
        let flash = session(EffectStyle::Flash, true, false);
        assert_eq!(typing_effect(&flash, 6, now), 0, "unknown event");
        assert_eq!(typing_effect(&flash, 0xFF, now), 0, "unknown event");
    }

    #[test]
    fn the_style_is_drawn_without_a_count_while_the_counter_is_off() {
        let sound = session(EffectStyle::PowerMode, false, true);
        let now = Instant::now();
        for _ in 0..20 {
            let answer = typing_effect(&sound, 0, now);
            assert_eq!(answer, 3 << ANSWER_STYLE_SHIFT);
        }
        assert_eq!(
            typing_effect(&sound, EVENT_COMMIT, now),
            3 << ANSWER_STYLE_SHIFT
        );
    }

    #[test]
    fn keys_count_and_backspace_or_a_pause_start_over() {
        let sound = session(EffectStyle::Sparks, true, false);
        let start = Instant::now();
        let at = |millis| start + Duration::from_millis(millis);
        for index in 1..=4u32 {
            let answer = typing_effect(&sound, index % 3, at(u64::from(index) * 100));
            assert_eq!(count(answer), index);
            assert_eq!(answer >> ANSWER_STYLE_SHIFT & 0b111, 2);
        }
        // A commit and an auto-repeated key are drawn but not counted.
        assert_eq!(count(typing_effect(&sound, EVENT_COMMIT, at(500))), 4);
        assert_eq!(count(typing_effect(&sound, FLAG_AUTO_REPEAT, at(600))), 4);
        // Backspace, by its key class or its own event, starts over.
        assert_eq!(count(typing_effect(&sound, 3, at(700))), 0);
        assert_eq!(count(typing_effect(&sound, 0, at(800))), 1);
        assert_eq!(count(typing_effect(&sound, EVENT_BACKSPACE, at(900))), 0);
        assert_eq!(count(typing_effect(&sound, 0, at(1_000))), 1);
        // Just under the idle reset carries on; a pause of the full length starts again.
        assert_eq!(count(typing_effect(&sound, 0, at(1_000 + 2_999))), 2);
        assert_eq!(count(typing_effect(&sound, 0, at(3_999 + 3_000))), 1);
        // A commit after a pause reports the combo already over.
        assert_eq!(count(typing_effect(&sound, EVENT_COMMIT, at(20_000))), 0);
    }

    #[test]
    fn milestones_move_the_combo_up_a_tier_once_each() {
        let sound = session(EffectStyle::Flash, true, false);
        let start = Instant::now();
        let mut tier_ups = Vec::new();
        for index in 1..=120u32 {
            let answer = typing_effect(&sound, 0, start + Duration::from_millis(u64::from(index)));
            assert_eq!(count(answer), index);
            assert_eq!(answer & ANSWER_TIER_SOUND, 0, "the tier sound is off");
            if answer & ANSWER_TIER_UP != 0 {
                tier_ups.push(index);
            }
        }
        assert_eq!(tier_ups, COMBO_MILESTONES);
        assert_eq!(sound.combo.get().tier, 4);
        // Starting over earns every tier again.
        typing_effect(&sound, EVENT_BACKSPACE, start + Duration::from_millis(200));
        let tier_ups = (1..=10u64)
            .filter(|index| {
                typing_effect(&sound, 0, start + Duration::from_millis(200 + index))
                    & ANSWER_TIER_UP
                    != 0
            })
            .count();
        assert_eq!(tier_ups, 1);
    }

    #[test]
    fn the_tier_sound_is_due_only_when_switched_on_and_not_muted() {
        let start = Instant::now();
        let tenth = |sound: &Session, flags: u32| {
            for index in 1..10u64 {
                typing_effect(sound, 0, start + Duration::from_millis(index));
            }
            typing_effect(sound, flags, start + Duration::from_millis(10))
        };
        let on = session(EffectStyle::Flash, true, true);
        let answer = tenth(&on, 0);
        assert_eq!(count(answer), 10);
        assert_ne!(answer & ANSWER_TIER_UP, 0);
        assert_ne!(answer & ANSWER_TIER_SOUND, 0);
        let muted = session(EffectStyle::Flash, true, true);
        let answer = tenth(&muted, FLAG_MUTED);
        assert_ne!(answer & ANSWER_TIER_UP, 0, "still drawn");
        assert_eq!(answer & ANSWER_TIER_SOUND, 0, "but not heard");
        // The counter alone still counts and reports tiers with the style off.
        let counter_only = session(EffectStyle::Off, true, false);
        let answer = tenth(&counter_only, 0);
        assert_eq!(answer, 10 | ANSWER_TIER_UP);
    }

    #[test]
    fn a_settings_change_keeps_the_combo() {
        let mut sound = SessionSound::new(SoundSettings {
            effect: TypingEffect::from_preferences(EffectStyle::Flash, 50),
            combo_counter: true,
            ..SoundSettings::default()
        });
        let now = Instant::now();
        super::typing_effect(&sound, 0, now);
        super::typing_effect(&sound, 0, now);
        sound.update(SoundSettings {
            effect: TypingEffect::from_preferences(EffectStyle::Sparks, 50),
            combo_counter: true,
            ..SoundSettings::default()
        });
        let answer = super::typing_effect(&sound, 0, now);
        assert_eq!(count(answer), 3);
        assert_eq!(answer >> ANSWER_STYLE_SHIFT, 2);
    }

    #[test]
    fn a_count_beyond_sixteen_bits_saturates_without_touching_the_flags() {
        let sound = session(EffectStyle::Flash, true, false);
        let now = Instant::now();
        sound.combo.set(Combo {
            count: 70_000,
            last: Some(now),
            tier: 4,
        });
        assert_eq!(
            typing_effect(&sound, 0, now),
            ANSWER_COUNT | 1 << ANSWER_STYLE_SHIFT
        );
    }
}
