# Agent Note: Windows translation source filtering

Status: implemented

## Problem

The Windows translation worker reconstructed every candidate with `source: 0` before asking the offline gloss API or the user translation plan. Emoji, kaomoji, mixed Japanese and plugin rows therefore bypassed the shared source policy. Translation answers are keyed by text, so a special row could also inherit the answer for an ordinary row with the same text.

## Decision

The host translation query carries each candidate's source. Windows preserves that source and skips sources 6, 7, 13 and 14 in both offline gloss and provider planning. The shared runtime suppresses translation display for those sources, preventing same-text answer bleed between rows.

共享计划对普通来源 11 的放行见[键盘模型整句翻译](2026-10-15-neural-candidate-translation-plan.md)，Windows 保留的原始来源直接交给该计划。

## Consequences

Only eligible rows reach translation providers, and special rows never display a text-keyed answer inherited from an ordinary row. The `/fy` sentence path remains unchanged because it carries no candidate source.

## Alternatives considered

Filtering only in the Windows worker would stop new requests but could not prevent same-text answers from appearing on special rows. Changing the translation ABI to key answers by candidate ID would be larger than the source metadata and runtime display gate needed here.

## Verification

- Host API translation query tests cover source metadata and mixed Japanese filtering.
- Input runtime tests cover duplicate text with ordinary and special sources.
- All 249 input-runtime unit tests pass.
- The Windows candidate policy translation test compiles with Clang syntax checking.
- `scripts/verify-local.sh --quick` reaches all non-Windows gates; the unrelated macOS linker phase fails because this worktree lacks a linkable macOS build artifact. Windows native C++ tests require the documented MinGW/Windows build.
