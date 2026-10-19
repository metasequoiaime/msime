import fs from '@ohos.file.fs';
import common from '@ohos.app.ability.common';
import client from 'libmsimeclient.so';
import { KeyboardLog } from '../keyboard/KeyboardLog';
import { GlobalTheme, ResolvedTheme } from '../keyboard/skin/GlobalTheme';

interface ProbeSnapshot {
  format_version: number;
  preferences: Record<string, Object>;
  revision: number;
}

interface ProbeCatalog {
  ok: boolean;
  value?: ProbeCatalogValue;
}

interface ProbeCatalogValue {
  packages: ProbePackage[];
  issues: Object[];
}

interface ProbePackage {
  id: string;
  canvas: boolean;
}

interface ProbeResponse {
  ok: boolean;
  value?: ProbeSnapshot;
}

/** 只编译进独立 canvasprobe 测试包；发布工程里没有此文件或调用点。 */
export class CandidateCanvasDeviceFixture {
  static prepare(context: common.Context): void {
    const state: string = `${context.filesDir}/state`;
    if (!fs.accessSync(state)) fs.mkdirSync(state, true);
    let phase: number = 0;
    const marker: string = `${state}/canvas-probe-phase`;
    if (fs.accessSync(marker)) phase = Number(fs.readTextSync(marker));
    if (!Number.isSafeInteger(phase) || phase < 0) phase = 0;
    CandidateCanvasDeviceFixture.write(marker, `${phase + 1}`);
    const dark: boolean = phase % 2 === 1;
    // 每次重建扩展在浅/深色间切换；第 3/4 次使用坏图检查可读回退。
    const id: string = phase % 4 >= 2 ? 'probe-broken' : 'probe-animation';
    const directory: string = `${state}/skins/${id}`;
    if (!fs.accessSync(directory)) fs.mkdirSync(directory, true);
    for (const name of ['skin.toml', 'panel.apng', 'widget.apng']) {
      const bytes: Uint8Array = context.resourceManager.getRawFileContentSync(`canvas-probe/${id}/${name}`);
      const file: fs.File = fs.openSync(`${directory}/${name}`, fs.OpenMode.CREATE | fs.OpenMode.WRITE_ONLY | fs.OpenMode.TRUNC);
      try { fs.writeSync(file.fd, bytes.slice().buffer as ArrayBuffer); } finally { fs.closeSync(file); }
    }
    const catalog: ProbeCatalog = JSON.parse(client.skinCatalog(`${state}/skins`)) as ProbeCatalog;
    if (!catalog.ok || catalog.value === undefined
      || !catalog.value.packages.some((entry: ProbePackage): boolean => entry.id === id && entry.canvas)) {
      throw new Error(`probe catalog refused: ${JSON.stringify(catalog.value?.issues ?? [])}`);
    }
    const mode: string = dark ? 'dark' : 'light';
    const base: string = dark ? 'night' : 'shuishan';
    const loaded: ProbeResponse = JSON.parse(client.loadPreferences(state)) as ProbeResponse;
    if (!loaded.ok || loaded.value === undefined) throw new Error('probe preferences unavailable');
    const preferences: Record<string, Object> = loaded.value.preferences;
    preferences['theme'] = mode;
    preferences['candidate_theme'] = mode;
    preferences['global_theme'] = 'custom';
    preferences['scheme'] = 'quanpin';
    preferences['touch_keyboard_layout'] = 'twenty_six_key';
    preferences['usage_reporting'] = false;
    const custom: Record<string, Object> = preferences['custom_theme'] as Record<string, Object>;
    custom['base'] = base;
    custom['candidate_skin'] = id;
    custom['candidate_skin_dark'] = id;
    const revision: number = loaded.value.revision;
    loaded.value.revision = revision + 1;
    const saved: ProbeResponse = JSON.parse(client.savePreferences(state, revision,
      JSON.stringify(loaded.value))) as ProbeResponse;
    if (!saved.ok) throw new Error('probe preferences refused');
    const resolved: ResolvedTheme | null = GlobalTheme.parseResolved(client.resolveTheme(JSON.stringify({
      global_theme: 'custom', custom_theme: custom, dark: dark, layout: 'horizontal',
      skins_directory: `${state}/skins`, canvas_panel_drawn: false
    })));
    if (resolved?.candidate_skin !== id) throw new Error('probe theme lost canvas package');
    KeyboardLog.info(`canvas probe phase ${phase % 4} ready: ${mode}, ${id}`);
  }

  private static write(path: string, text: string): void {
    const file: fs.File = fs.openSync(path, fs.OpenMode.CREATE | fs.OpenMode.WRITE_ONLY | fs.OpenMode.TRUNC);
    try { fs.writeSync(file.fd, text); } finally { fs.closeSync(file); }
  }
}
