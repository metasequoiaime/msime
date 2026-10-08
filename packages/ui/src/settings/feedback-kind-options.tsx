/** 反馈页共用的反馈类别。写成返回 Fragment 的普通函数，鸿蒙手机的 `SelectRow` 才能从中读出面板选项。 */
export function feedbackKindOptions() {
  return (
    <>
      <option>功能异常</option>
      <option>候选词不对</option>
      <option>功能建议</option>
      <option>其他</option>
    </>
  );
}

/** 以组件形式提供同一组反馈类别，供原生 select 使用。 */
export function FeedbackKindOptions() {
  return feedbackKindOptions();
}
