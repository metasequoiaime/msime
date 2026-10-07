#!/usr/bin/env python3
"""Bound iOS voice polish prompt copies at both Swift and C++ boundaries."""

from pathlib import Path

root = Path(__file__).resolve().parents[1]
swift = (root / "platforms/ios/App/Services/VoicePolish.swift").read_text(encoding="utf-8")
cpp = (root / "platforms/ios/App/Services/VoicePolishPrompt.cpp").read_text(encoding="utf-8")

for declaration in (
    "static let maximumPromptBytes = 8_192",
    "constexpr size_t kPolishPromptCustomLimit = 8 * 1024;",
):
    source = swift if declaration.startswith("static") else cpp
    if declaration not in source:
        raise SystemExit(f"missing iOS polish prompt bound: {declaration}")
if "guard values.allSatisfy({ $0.utf8.count <= Self.maximumPromptBytes }) else { return \"\" }" not in swift:
    raise SystemExit("iOS polish prompts must be bounded before strdup")
if "strnlen(value, limit + 1)" not in cpp or "if (length > limit) return false;" not in cpp:
    raise SystemExit("iOS polish bridge must bound C strings before std::string copies")
print("iOS polish prompt inputs are bounded before copies")
