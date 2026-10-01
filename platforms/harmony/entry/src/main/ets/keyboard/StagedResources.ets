/**
 * Copying the packaged Engine resources out of the HAP, once per generation.
 *
 * One implementation, used by both the keyboard extension and the settings page. They used to have
 * their own: the extension wiped and re-copied unless a marker existed, the settings page copied
 * over whatever was already there and wrote no marker at all. Two rules for one directory is how the
 * directory came to hold a set that matched neither, which the shared verification then refused.
 */
import fs from '@ohos.file.fs';
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
      if (fs.accessSync(marker)) {
        fs.unlinkSync(marker);
      }
      StagedResources.removeDirectory(destination);
      fs.mkdirSync(destination, true);
      for (const artifact of artifacts) {
        const slash: number = artifact.name.lastIndexOf('/');
        const parent: string = `${destination}/${artifact.name.substring(0, slash)}`;
        if (slash > 0 && !fs.accessSync(parent)) {
          fs.mkdirSync(parent, true);
        }
        fs.copyFileSync(`${packaged}/${artifact.name}`, `${destination}/${artifact.name}`);
      }
      const handle = fs.openSync(marker,
        fs.OpenMode.CREATE | fs.OpenMode.WRITE_ONLY | fs.OpenMode.TRUNC);
      try {
        fs.writeSync(handle.fd, token);
      } finally {
        fs.closeSync(handle);
      }
      KeyboardLog.info(`staged ${destination}`);
      return true;
    } catch (error) {
      KeyboardLog.error(`staging failed: ${describeError(error)}`);
      return false;
    }
  }

  /**
   * The Cantonese and Zhuyin dictionaries, from `language-dictionaries` under `resourceDir` to `language-dictionaries` beside the staged Engine resources in `filesDir`, where host-api looks for them. A package built without them removes any an earlier one left, and those schemes are then not offered; either way the keyboard still starts.
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
      const stat = fs.statSync(`${root}/${path}`);
      if (stat.isDirectory()) {
        StagedResources.listFiles(root, path, into);
      } else {
        into.push({ name: path, size: stat.size });
      }
    }
  }

  private static readMarker(marker: string): string | null {
    try {
      return fs.accessSync(marker) ? fs.readTextSync(marker) : null;
    } catch (error) {
      return null;
    }
  }

  /** Remove a staged tree before replacing it; rmdirSync only accepts empty directories. */
  static removeDirectory(directory: string): void {
    if (!fs.accessSync(directory)) {
      return;
    }
    for (const entry of fs.listFileSync(directory)) {
      const child: string = `${directory}/${entry}`;
      if (fs.statSync(child).isDirectory()) {
        StagedResources.removeDirectory(child);
      } else {
        fs.unlinkSync(child);
      }
    }
    fs.rmdirSync(directory);
  }
}
