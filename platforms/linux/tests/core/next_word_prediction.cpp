#include "../src/core/NextWordPrediction.h"

#include <cassert>

using Json = nlohmann::json;
using msime::linux_host::prediction_key;
using msime::linux_host::PredictionKey;
using msime::linux_host::PredictionResetEcho;
using msime::linux_host::view_is_prediction;

int main() {
  const Json suggestion{{"prediction", true},
                        {"editing_text", ""},
                        {"candidates", Json::array({Json{{"text", "饭"}}})}};
  const Json composing{{"editing_text", "chi"}, {"candidates", Json::array({Json{{"text", "吃"}}})}};

  // 只有布尔真值的 prediction 才是联想；普通视图省略这个字段。会话关掉后视图是 null，不能抛。
  assert(view_is_prediction(suggestion));
  assert(!view_is_prediction(composing));
  assert(!view_is_prediction(Json{{"prediction", false}}));
  assert(!view_is_prediction(Json{{"prediction", 1}}));
  assert(!view_is_prediction(Json{{"prediction", "true"}}));
  assert(!view_is_prediction(Json()));
  assert(!view_is_prediction(Json::array({true})));

  // 没有联想时照常路由，选词键也一样；有联想时 Esc 收起并吃掉，选词的空格和数字选词，其余键先收起再照常路由。
  for (const bool picks : {false, true}) {
    assert(prediction_key(false, false, picks) == PredictionKey::Route);
    assert(prediction_key(false, true, picks) == PredictionKey::Route);
    assert(prediction_key(true, true, picks) == PredictionKey::Close);
  }
  assert(prediction_key(true, false, true) == PredictionKey::Pick);
  assert(prediction_key(true, false, false) == PredictionKey::DismissThenRoute);

  // 上屏产生联想之后窗口内的 reset 是回声，联想留着；窗口外、中间有按键、或者上屏没有产生联想，reset 照常收起。
  PredictionResetEcho echo;
  assert(!echo.keeps(true, 1000));
  echo.committed(true, 1000);
  assert(echo.keeps(true, 1000));
  assert(echo.keeps(true, 1000 + PredictionResetEcho::kWindowMs));
  assert(!echo.keeps(true, 1001 + PredictionResetEcho::kWindowMs));
  // 视图已经不是联想（比如刚被别的命令收起）时没有可留的东西。
  assert(!echo.keeps(false, 1001));
  // 同一个窗口里可以有不止一次回声。
  assert(echo.keeps(true, 1002) && echo.keeps(true, 1003));
  echo.forget();
  assert(!echo.keeps(true, 1004));
  echo.committed(true, 2000);
  echo.committed(false, 2001);
  assert(!echo.keeps(true, 2002));
  return 0;
}
