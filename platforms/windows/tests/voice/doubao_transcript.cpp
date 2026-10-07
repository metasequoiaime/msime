#include "DoubaoTranscript.h"

#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
void require(bool value, int line) {
  if (!value)
    throw std::runtime_error("doubao transcript failed at line " + std::to_string(line));
}
#define REQUIRE(value) require((value), __LINE__)
} // namespace

int main() {
  try {
    using nlohmann::json;
    // bigmodel_async: "result" is an object.
    REQUIRE(doubao_transcript(json::parse(R"({"result":{"text":"合成文本"}})")) == "合成文本");
    // bigmodel_nostream: "result" is a list of segments, joined in order. This shape used to yield nothing, so the whole recording was lost.
    REQUIRE(doubao_transcript(json::parse(
                R"({"result":[{"text":"第一段"},{"text":"第二段"}]})")) == "第一段第二段");
    REQUIRE(doubao_transcript(json::parse(
                R"({"result":[{"text":"a"},7,{"confidence":1},{"text":null},{"text":"b"}]})")) == "ab");
    REQUIRE(doubao_transcript(json::parse(R"({"result":[]})")).empty());
    // The legacy envelope, in both shapes.
    REQUIRE(doubao_transcript(json::parse(
                R"({"payload_msg":{"result":{"text":"旧"}}})")) == "旧");
    REQUIRE(doubao_transcript(json::parse(
                R"({"payload_msg":{"result":[{"text":"旧"},{"text":"版"}]}})")) == "旧版");
    REQUIRE(doubao_transcript(json::parse(
                R"({"result":{"text":"坏\u0000文本"}})"))
                .empty());
    REQUIRE(doubao_transcript(json::parse(
                R"({"result":[{"text":"安全"},{"text":"坏\u0007文本"}]})"))
                .empty());
    REQUIRE(doubao_transcript(json::parse(
                R"({"result":{"text":"坏\u0085文本"}})"))
                .empty());
    // Anything else is no transcript, never an exception.
    REQUIRE(doubao_transcript(json::parse(R"({"result":"text"})")).empty());
    REQUIRE(doubao_transcript(json::parse(R"({"result":{"text":3}})")).empty());
    REQUIRE(doubao_transcript(json::parse(R"([{"text":"x"}])")).empty());
    REQUIRE(doubao_transcript(json::parse(R"({"payload_msg":"x"})")).empty());
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  return 0;
}
