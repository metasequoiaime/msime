/** AI 联想与语音润色共用的三个可编辑提示词槽位。写成返回 Fragment 的普通函数，鸿蒙手机的 `SelectRow` 才能从中读出面板选项。 */
export function customPromptSlotOptions() {
  return (
    <>
      <option value="custom_1">自定义一</option>
      <option value="custom_2">自定义二</option>
      <option value="custom_3">自定义三</option>
    </>
  );
}
