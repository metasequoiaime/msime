import {
  CandidateSkinPackageIdentity,
  SkinFileStat,
} from "../entry/src/main/ets/keyboard/candidate/CandidateSkinPackageIdentity";

// 选中的皮肤包在原处被同名替换后，键盘下一次获得焦点要发现它：设置应用重装图库作品或皮肤编辑器覆盖保存后，偏好修订号不变，键盘只能靠包文件的身份知道要重新应用外观。

let failures = 0;
let checks = 0;

function group(name: string, body: () => void): void {
  try {
    body();
    console.log(`  ok  ${name}`);
  } catch (error) {
    failures++;
    console.log(`FAIL  ${name}`);
    console.log(`      ${error instanceof Error ? error.message : String(error)}`);
  }
}

function check(condition: boolean, message: string): void {
  checks++;
  if (!condition) throw new Error(message);
}

interface NodeStat {
  ino: bigint;
  mtimeNs: bigint;
  size: bigint;
}

interface NodeFs {
  mkdtempSync(prefix: string): string;
  mkdirSync(path: string, options: { recursive: boolean }): void;
  writeFileSync(path: string, data: string): void;
  renameSync(from: string, to: string): void;
  rmSync(path: string, options: { recursive: boolean; force: boolean }): void;
  statSync(path: string, options: { bigint: true }): NodeStat;
  readFileSync(path: string, encoding: "utf8"): string;
}

declare const __dirname: string;
declare function require(name: "fs"): NodeFs;
declare function require(name: "os"): { tmpdir(): string };

const fs: NodeFs = require("fs");

// 与键盘上 `KeyboardSession.statSkinFile` 取同样的字段：inode、纳秒修改时间和大小。
function statFile(path: string): SkinFileStat | null {
  try {
    const info: NodeStat = fs.statSync(path, { bigint: true });
    return { ino: info.ino.toString(), mtime: info.mtimeNs.toString(), size: Number(info.size) };
  } catch {
    return null;
  }
}

const SKIN_ID = "synthetic-replace";

function writePackage(folder: string, topInset: number, image: string): void {
  fs.mkdirSync(folder, { recursive: true });
  fs.writeFileSync(
    `${folder}/skin.toml`,
    `schema_version = 1\nid = "${SKIN_ID}"\nname = "Synthetic Replace"\nversion = "1"\nbase = "system"\n` +
      `[candidate_window.decoration]\nimage = "decoration.png"\ntop_inset_dip = ${topInset}\n`,
  );
  fs.writeFileSync(`${folder}/decoration.png`, image);
}

const root: string = fs.mkdtempSync(`${require("os").tmpdir()}/msime-harmony-skin-identity-`);
const skins = `${root}/skins`;
const target = `${skins}/${SKIN_ID}`;
const packagePaths = (): string[] =>
  CandidateSkinPackageIdentity.packagePaths(skins, [SKIN_ID, SKIN_ID]);
const imagePaths = (): string[] =>
  CandidateSkinPackageIdentity.imagePaths(skins, SKIN_ID, ["decoration.png", null]);
const packageIdentity = (): string =>
  CandidateSkinPackageIdentity.identity(packagePaths(), statFile);
const imageIdentity = (): string => CandidateSkinPackageIdentity.identity(imagePaths(), statFile);

try {
  writePackage(target, 26, "old decoration");

  group("both slots naming one package stat its folder and manifest once", () => {
    check(
      JSON.stringify(packagePaths()) === JSON.stringify([target, `${target}/skin.toml`]),
      `paths were ${JSON.stringify(packagePaths())}`,
    );
    check(
      JSON.stringify(imagePaths()) === JSON.stringify([`${target}/decoration.png`]),
      "an absent background adds no path",
    );
  });

  group("ids that are not package names never become paths", () => {
    const paths = CandidateSkinPackageIdentity.packagePaths(skins, [
      undefined,
      null,
      "",
      ".",
      "..",
      "../x",
      "a/b",
    ]);
    check(paths.length === 0, `paths were ${JSON.stringify(paths)}`);
    check(
      CandidateSkinPackageIdentity.imagePaths(skins, null, ["decoration.png"]).length === 0,
      "no drawn package means no image to stat",
    );
  });

  let packageBefore = packageIdentity();
  let imageBefore = imageIdentity();

  group("nothing changed reads the same identity", () => {
    check(
      packageIdentity() === packageBefore && imageIdentity() === imageBefore,
      "identity moved without a change",
    );
    check(!packageBefore.includes("-;"), "every selected path was readable");
  });

  group("the installer's staged rename over the package is seen", () => {
    // 照 client-services 的安装那样：新包写进暂存目录，旧目录改名让开，新目录改名进来，再删掉旧的。同名、同路径，内容全换。
    const staging = `${skins}/.staging/${SKIN_ID}`;
    writePackage(staging, 73, "new decoration, larger");
    fs.renameSync(target, `${skins}/.replaced-${SKIN_ID}`);
    fs.renameSync(staging, target);
    fs.rmSync(`${skins}/.replaced-${SKIN_ID}`, { recursive: true, force: true });
    fs.rmSync(`${skins}/.staging`, { recursive: true, force: true });
    check(packageIdentity() !== packageBefore, "a renamed-in package kept the old identity");
    packageBefore = packageIdentity();
    imageBefore = imageIdentity();
    check(packageIdentity() === packageBefore, "the identity is stable once recorded");
  });

  group("the shared state mirror replacing the whole skins tree is seen", () => {
    // `SharedStateMirror.copy` 把设置应用发布的 `skins` 整棵复制到旁边再用 `StagedResources.replaceDirectory` 换入：每个包目录都是新建的。
    const staging = `${skins}.import.staging`;
    writePackage(`${staging}/${SKIN_ID}`, 73, "new decoration, larger");
    fs.renameSync(skins, `${skins}.previous`);
    fs.renameSync(staging, skins);
    fs.rmSync(`${skins}.previous`, { recursive: true, force: true });
    check(
      packageIdentity() !== packageBefore,
      "a mirrored-in skins tree kept the old package identity",
    );
    packageBefore = packageIdentity();
    imageBefore = imageIdentity();
  });

  group("rewriting the decoration in place is seen through the image identity", () => {
    fs.writeFileSync(`${target}/decoration.png`, "rewritten decoration image in place");
    check(imageIdentity() !== imageBefore, "an in-place image rewrite kept the old identity");
    imageBefore = imageIdentity();
  });

  group("rewriting skin.toml in place is seen", () => {
    fs.writeFileSync(
      `${target}/skin.toml`,
      `schema_version = 1\nid = "${SKIN_ID}"\nname = "Edited by hand"\n`,
    );
    check(
      packageIdentity() !== packageBefore,
      "an in-place manifest rewrite kept the old identity",
    );
    packageBefore = packageIdentity();
  });

  group("removing the package and installing it again are both seen", () => {
    fs.rmSync(target, { recursive: true, force: true });
    const removed = packageIdentity();
    check(removed !== packageBefore && removed === "-;-;", `a removed package read as ${removed}`);
    writePackage(target, 73, "reinstalled");
    check(packageIdentity() !== removed, "a reinstalled package still read as missing");
  });
} finally {
  fs.rmSync(root, { recursive: true, force: true });
}

// 键盘一侧的接线：`applyAppearance` 在读包之前记下身份，获得焦点时在偏好修订号检查之后再查一次，换过就让视图重画。
group("the keyboard records the identity before reading and checks it on focus", () => {
  const source: string = fs.readFileSync(
    `${__dirname}/../../../platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets`,
    "utf8",
  );
  const apply: number = source.indexOf(
    "private applyAppearance(preferences: PreparedPreferences): void {",
  );
  const recorded: number = source.indexOf(
    "this.skinPackageIdentity = this.selectedSkinPackageIdentity(preferences);",
    apply,
  );
  const catalog: number = source.indexOf(
    "this.readCandidateSkinCatalog(this.stateDirectory)",
    apply,
  );
  check(
    apply >= 0 && recorded > apply && catalog > recorded,
    "applyAppearance must record the package identity before reading the catalog",
  );
  const reload: number = source.indexOf("    this.reloadChangedPreferences();\n");
  const replaced: number = source.indexOf(
    "if (this.reloadReplacedSkinPackages() && this.onViewPreferencesReloaded !== undefined)",
    reload,
  );
  check(
    reload >= 0 && replaced > reload && replaced - reload < 400,
    "attach must check for a replaced skin package right after the revision check",
  );
});

console.log(`${checks - failures} checks passed; ${failures} failed`);
if (failures !== 0) throw new Error(`${failures} candidate skin package identity checks failed`);
