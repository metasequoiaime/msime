export function validVoiceLanguage(value: string) {
  return (
    value.length > 0 &&
    value.length <= 64 &&
    !Array.from(value).some((character) => {
      const code = character.codePointAt(0) ?? 0;
      return code <= 0x1f || code === 0x7f;
    })
  );
}
