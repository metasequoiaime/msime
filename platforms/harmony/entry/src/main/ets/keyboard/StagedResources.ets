/**
 * Copying the packaged Engine resources out of the HAP, once per generation.
 *
 * One implementation, used by both the keyboard extension and the settings page. They used to have
 * their own: the extension wiped and re-copied unless a marker existed, the settings page copied
 * over whatever was already there and wrote no marker at all. Two rules for one directory is how the
 * directory came to hold a set that matched neither, which the shared verification then refused.
 */
import fs from '@ohos.file.fs';
import util from '@ohos.util';
import { KeyboardLog, describeError } from './KeyboardLog';
import { StagedArtifact, StagedResourcePolicy } from './StagedResourcePolicy';

export class StagedResources {
  /**
   * Make `destination` hold exactly what `packaged` holds.
   *
   * Returns whether the destination can be trusted afterwards. The destination is emptied first, so
   * an artifact dropped from a newer package does not survive as the extra file that makes the whole
   * directory fail verification, and the marker is written last, so a copy interrupted half way
   * looks unstaged rather than complete.
   */
  static stage(packaged: string, destination: string, marker: string): boolean {
    const staging: string = `${destination}.${util.generateRandomUUID(false)}.staging`;
    try {
      if (!fs.accessSync(packaged)) {
        KeyboardLog.error(`no packaged resources at ${packaged}`);
        return false;
      }
      const artifacts: StagedArtifact[] = [];
      StagedResources.listFiles(packaged, '', artifacts);
      const token: string = StagedResourcePolicy.generationToken(artifacts);
      if (!StagedResourcePolicy.needsStaging(StagedResources.readMarker(marker), token)) {
        return true;
      }
      // Build beside the live tree. Readers can continue using the previous complete generation
      // while a second process stages the new one.
      fs.mkdirSync(staging, true);
      for (const artifact of artifacts) {
        const slash: number = artifact.name.lastIndexOf('/');
        const parent: string = `${staging}/${artifact.name.substring(0, slash)}`;
        if (slash > 0 && !fs.accessSync(parent)) {
          fs.mkdirSync(parent, true);
        }
        fs.copyFileSync(`${packaged}/${artifact.name}`, `${staging}/${artifact.name}`);
      }
      StagedResources.replaceDirectory(staging, destination);
      if (fs.accessSync(marker)) {
        fs.unlinkSync(marker);
      }
      const markerStaging: string = `${marker}.${util.generateRandomUUID(false)}.tmp`;
      let renamed: boolean = false;
      try {
        const handle = fs.openSync(markerStaging,
          fs.OpenMode.CREATE | fs.OpenMode.WRITE_ONLY | fs.OpenMode.TRUNC);
        try {
          fs.writeSync(handle.fd, token);
        } finally {
          fs.closeSync(handle);
        }
        fs.renameSync(markerStaging, marker);
        renamed = true;
      } finally {
        if (!renamed) {
          try {
            if (fs.accessSync(markerStaging)) fs.unlinkSync(markerStaging);
          } catch {
            // A failed marker write leaves at most its uniquely named staging file.
          }
        }
      }
      KeyboardLog.info(`staged ${destination}`);
      return true;
    } catch (error) {
      KeyboardLog.error(`staging failed: ${describeError(error)}`);
      try {
        StagedResources.removeDirectory(staging);
      } catch {
        // The original failure is more useful than a best-effort cleanup failure.
      }
      return false;
    }
  }

  /**
   * The Cantonese, Zhuyin and Stroke dictionaries, from `language-dictionaries` under `resourceDir` to `language-dictionaries` beside the staged Engine resources in `filesDir`, where host-api looks for them. A package built without them removes any an earlier one left, and those schemes are then not offered; either way the keyboard still starts.
   */
  static stageLanguageDictionaries(resourceDir: string, filesDir: string): void {
    const packaged: string = `${resourceDir}/language-dictionaries`;
    const destination: string = `${filesDir}/language-dictionaries`;
    const marker: string = `${filesDir}/language-dictionaries.ready`;
    if (fs.accessSync(packaged)) {
      StagedResources.stage(packaged, destination, marker);
    } else {
      StagedResources.unstage(destination, marker);
    }
  }

  /** Removes a staged directory and its marker, for resources the installed package no longer carries. */
  static unstage(destination: string, marker: string): void {
    try {
      if (fs.accessSync(marker)) {
        fs.unlinkSync(marker);
      }
      StagedResources.removeDirectory(destination);
    } catch (error) {
      KeyboardLog.error(`unstaging failed: ${describeError(error)}`);
    }
  }

  /**
   * Every file below `root`, named by its path relative to it. The Engine's helpcodes/ directory is nested, so its tables are listed as helpcodes/<file> and a changed table changes the generation token like any other artifact.
   */
  private static listFiles(root: string, relative: string, into: StagedArtifact[]): void {
    const directory: string = relative.length === 0 ? root : `${root}/${relative}`;
    for (const name of fs.listFileSync(directory)) {
      const path: string = relative.length === 0 ? name : `${relative}/${name}`;
      const stat: fs.Stat = fs.lstatSync(`${root}/${path}`);
      if (stat.isSymbolicLink()) {
        throw new Error(`packaged resource is a symbolic link: ${path}`);
      }
      if (stat.isDirectory()) {
        StagedResources.listFiles(root, path, into);
      } else {
        into.push({ name: path, size: stat.size });
      }
    }
  }

  private static readMarker(marker: string): string | null {
    let handle: fs.File | null = null;
    try {
      const expected: fs.Stat = fs.lstatSync(marker);
      if (expected.isSymbolicLink() || !expected.isFile() || expected.size > 256) return null;
      handle = fs.openSync(marker, fs.OpenMode.READ_ONLY);
      const stat: fs.Stat = fs.statSync(handle.fd);
      if (!stat.isFile() || stat.size > 256 || stat.ino !== expected.ino) return null;
      const buffer: ArrayBuffer = new ArrayBuffer(stat.size + 1);
      const count: number = fs.readSync(handle.fd, buffer);
      if (count !== stat.size) return null;
      return util.TextDecoder.create('utf-8').decode(new Uint8Array(buffer, 0, count));
    } catch (error) {
      return null;
    } finally {
      if (handle !== null) fs.closeSync(handle);
    }
  }

  /** Remove a staged tree before replacing it; rmdirSync only accepts empty directories. */
  static removeDirectory(directory: string): void {
    let directoryStat: fs.Stat;
    try {
      directoryStat = fs.lstatSync(directory);
    } catch {
      return;
    }
    if (directoryStat.isSymbolicLink() || !directoryStat.isDirectory()) {
      // 清理阶段遇到外部链接或意外的普通文件时，只删除目录项，绝不跟随到目录外。
      fs.unlinkSync(directory);
      return;
    }
    for (const entry of fs.listFileSync(directory)) {
      const child: string = `${directory}/${entry}`;
      if (fs.lstatSync(child).isDirectory()) {
        StagedResources.removeDirectory(child);
      } else {
        fs.unlinkSync(child);
      }
    }
    fs.rmdirSync(directory);
  }

  /** Replace a live resource tree only after its replacement is complete. */
  private static replaceDirectory(staging: string, destination: string): void {
    const previous: string = `${destination}.${util.generateRandomUUID(false)}.previous`;
    let moved: boolean = false;
    let installed: boolean = false;
    try {
      if (fs.accessSync(destination)) {
        fs.renameSync(destination, previous);
        moved = true;
      }
      fs.renameSync(staging, destination);
      installed = true;
      if (moved) {
        StagedResources.removeDirectory(previous);
      }
    } catch (error) {
      if (!installed) {
        try {
          if (fs.accessSync(staging)) StagedResources.removeDirectory(staging);
        } catch {
          // Preserve the swap failure below.
        }
        try {
          if (moved && !fs.accessSync(destination)) fs.renameSync(previous, destination);
        } catch {
          // A failed rollback leaves the marker invalid, so the next stage will retry.
        }
      }
      throw error as Error;
    }
  }
}
