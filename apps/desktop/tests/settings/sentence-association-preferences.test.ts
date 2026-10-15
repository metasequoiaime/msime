import { expect, test } from "vitest";
import {
  sentenceAssociationControlState,
  sentenceAssociationLevel,
  sentenceAssociationWithLevel,
  type SentenceAssociationLevel,
} from "../../../../packages/ui/src/settings/sentence-association-preferences";

test("缺键文档默认开启词网格，两个宿主的神经联想各自默认关闭", () => {
  for (const value of [undefined, Object.freeze({})]) {
    for (const mobile of [false, true]) {
      expect(sentenceAssociationControlState(value, { mobile })).toEqual({
        wordLatticeChecked: true,
        neuralChecked: false,
        neuralVisible: true,
      });
    }
    expect(sentenceAssociationLevel(value)).toBe(1);
  }
});

test("桌面和键盘开关独立，显式关闭词网格不清除神经选择", () => {
  const value = Object.freeze({
    word_lattice: false,
    neural_desktop: true,
    neural_keyboard: false,
    show_next_on_duplicate: false,
  });
  expect(sentenceAssociationControlState(value, { mobile: false })).toEqual({
    wordLatticeChecked: false,
    neuralChecked: true,
    neuralVisible: true,
  });
  expect(sentenceAssociationControlState(value, { mobile: true })).toEqual({
    wordLatticeChecked: false,
    neuralChecked: false,
    neuralVisible: true,
  });
  expect(sentenceAssociationLevel(value)).toBe(0);
});

test("版本能力只隐藏键盘神经控件，不影响桌面或改写已存选择", () => {
  const value = Object.freeze({ neural_keyboard: true });
  expect(sentenceAssociationControlState(value, { mobile: true, neuralKeyboard: false })).toEqual({
    wordLatticeChecked: true,
    neuralChecked: true,
    neuralVisible: false,
  });
  expect(sentenceAssociationControlState(value, { mobile: false, neuralKeyboard: false })).toEqual({
    wordLatticeChecked: true,
    neuralChecked: false,
    neuralVisible: true,
  });
  expect(sentenceAssociationLevel(value)).toBe(2);
  expect(value).toEqual({ neural_keyboard: true });
});

test("级别读取覆盖词网格与键盘神经开关的所有组合", () => {
  for (const word_lattice of [undefined, false, true]) {
    for (const neural_keyboard of [undefined, false, true]) {
      expect(sentenceAssociationLevel({ word_lattice, neural_keyboard })).toBe(
        word_lattice === false ? 0 : neural_keyboard ? 2 : 1,
      );
    }
  }
});

test.each<SentenceAssociationLevel>([0, 1, 2])(
  "级别 %s 写回保留其他字段且不改动原对象",
  (level) => {
    const value = Object.freeze({ neural_desktop: true, show_next_on_duplicate: false });
    const next = sentenceAssociationWithLevel(value, level);
    expect(next).toEqual({
      neural_desktop: true,
      show_next_on_duplicate: false,
      word_lattice: level > 0,
      neural_keyboard: level > 1,
    });
    expect(next).not.toBe(value);
    expect(sentenceAssociationLevel(next)).toBe(level);
    expect(value).toEqual({ neural_desktop: true, show_next_on_duplicate: false });
  },
);

test("从缺键文档选择级别只新增词网格和键盘神经两个字段", () => {
  expect(sentenceAssociationWithLevel(undefined, 2)).toEqual({
    word_lattice: true,
    neural_keyboard: true,
  });
});
