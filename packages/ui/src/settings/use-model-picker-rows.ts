import { useState } from "react";
import { modelPickerModel } from "./model-picker-model";

export interface UseModelPickerRowsOptions {
  models: readonly string[];
  model: string;
  onChange: (model: string) => void;
}

export interface ModelPickerRowsState {
  known: string[];
  custom: boolean;
  select: (value: string) => void;
  edit: (value: string) => void;
}

/** 模型下拉与自定义输入的受控状态；列表变化时清除上一次的手动展开意图。 */
export function useModelPickerRows({
  models,
  model,
  onChange,
}: UseModelPickerRowsOptions): ModelPickerRowsState {
  const current = modelPickerModel(models, model);
  const [requested, setRequested] = useState({ listKey: current.listKey, custom: false });
  const custom = (requested.listKey === current.listKey && requested.custom) || current.custom;
  const select = (value: string) => {
    setRequested({ listKey: current.listKey, custom: value === "" });
    if (value !== "") onChange(value);
  };
  const edit = (value: string) => {
    setRequested({ listKey: current.listKey, custom: true });
    onChange(value);
  };
  return { known: current.known, custom, select, edit };
}
