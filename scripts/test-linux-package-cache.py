#!/usr/bin/env python3
"""检查打包镜像失效条件，以及临时容器外的缓存保留边界。"""

import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location(
    "package_image", ROOT / "platforms/linux/scripts/package-image.py")
image = importlib.util.module_from_spec(spec)
spec.loader.exec_module(image)


class ImageCache(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        (self.root / "rust-toolchain.toml").write_text('[toolchain]\nchannel="1.97.1"\n')
        self.recipe = self.root / "Dockerfile"
        self.recipe.write_text("FROM example@sha256:synthetic\nRUN true\n")
        self.images = {}
        self.builds = []
        self.fail = False
        for mock in (patch.object(image, "ROOT", self.root),
                     patch.object(image.subprocess, "run", side_effect=self.docker),
                     patch.object(image.subprocess, "check_output", return_value="linux/aarch64\n"),
                     patch.dict(image.os.environ, {"DOCKER_DEFAULT_PLATFORM": ""})):
            mock.start()
            self.addCleanup(mock.stop)

    def docker(self, command, **kwargs):
        if command[1:3] == ["image", "inspect"]:
            value = self.images.get(command[-1])
            return SimpleNamespace(returncode=0 if value else 1, stdout=value or "")
        if command[1:3] == ["image", "tag"]:
            self.images[command[-1]] = command[-2]
            return SimpleNamespace(returncode=0)
        self.assertEqual(command[1], "build")
        self.assertEqual(command[-1], "-")
        self.assertEqual(kwargs["input"], self.recipe.read_bytes())
        self.builds.append(command)
        if self.fail:
            raise subprocess.CalledProcessError(1, command)
        self.images[command[command.index("-t") + 1]] = f"sha256:synthetic-{len(self.builds)}"
        return SimpleNamespace(returncode=0)

    def ensure(self, **kwargs):
        return image.ensure_image("msime-linux-test", self.recipe, **kwargs)

    def test_repeat_and_source_changes_do_not_build(self):
        first = self.ensure()
        (self.root / "synthetic.cpp").write_text("int synthetic = 1;\n")
        self.assertEqual(self.ensure(), first)
        self.assertEqual(len(self.builds), 1)
        self.assertIn("linux/arm64", self.builds[0])

    def test_recipe_toolchain_and_platform_each_invalidate(self):
        previous = self.ensure()
        self.recipe.write_text(self.recipe.read_text() + "LABEL synthetic=changed\n")
        changed = self.ensure()
        self.assertNotEqual(previous, changed)
        (self.root / "rust-toolchain.toml").write_text('[toolchain]\nchannel="synthetic"\n')
        previous, changed = changed, self.ensure()
        self.assertNotEqual(previous, changed)
        self.assertNotEqual(changed, self.ensure(platform="linux/amd64"))
        self.assertEqual(len(self.builds), 4)

    def test_default_platform_and_arch_alias(self):
        first = self.ensure(platform="linux/arm64")
        self.assertEqual(self.ensure(platform="linux/aarch64"), first)
        with patch.dict(image.os.environ, {"DOCKER_DEFAULT_PLATFORM": "linux/amd64"}):
            self.assertNotEqual(self.ensure(), first)

    def test_parent_id_changes_invalidate_child(self):
        self.images["synthetic-parent:tag"] = "sha256:parent-one"
        args = ["MSIME_BUILD_GATE_IMAGE=synthetic-parent:tag"]
        first = self.ensure(build_args=args)
        self.assertEqual(self.ensure(build_args=args), first)
        self.images["synthetic-parent:tag"] = "sha256:parent-two"
        self.assertNotEqual(self.ensure(build_args=args), first)
        self.assertEqual(len(self.builds), 2)
        self.assertIn(args[0], self.builds[-1])

    def test_deleted_image_rebuilds_and_failure_propagates(self):
        first = self.ensure()
        self.images.clear()
        self.fail = True
        with self.assertRaises(subprocess.CalledProcessError):
            self.ensure()
        self.fail = False
        self.assertNotEqual(first, self.ensure())

    def test_parent_output_uses_tag_for_from(self):
        tag = self.ensure(tag_output=True)
        self.assertTrue(tag.startswith("msime-linux-test:"))
        self.assertEqual(self.ensure(), self.images[tag])
        self.assertEqual(len(self.builds), 1)
        self.assertEqual(tag, f"msime-linux-test:id-{self.ensure().removeprefix('sha256:')}")

    def test_local_files_cannot_escape_cache_key(self):
        self.recipe.write_text("FROM synthetic\nCOPY . /source\n")
        with self.assertRaises(ValueError):
            self.ensure()
        self.assertEqual(self.builds, [])


class BuildCache(unittest.TestCase):
    def test_same_image_preserves_objects_changed_image_keeps_downloads(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            script = ROOT / "platforms/linux/scripts/prepare-package-cache.sh"

            def prepare(identity):
                subprocess.run(["bash", str(script), str(root)], check=True,
                               env=dict(os.environ, MSIME_PACKAGE_IMAGE=identity))

            prepare("sha256:synthetic-one")
            for path in ("cargo/release/synthetic", "cmake/synthetic.o",
                         "cmake-wubi/synthetic.o", "cargo-home/registry/synthetic",
                         "voice-runtime/synthetic"):
                file = root / path
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_text("synthetic")
            prepare("sha256:synthetic-one")
            self.assertTrue((root / "cmake/synthetic.o").exists())
            self.assertTrue((root / "cmake-wubi/synthetic.o").exists())
            self.assertTrue((root / "cargo/release/synthetic").exists())
            prepare("sha256:synthetic-two")
            for path in ("cargo", "cmake", "cmake-wubi"):
                self.assertFalse((root / path).exists())
            self.assertTrue((root / "cargo-home/registry/synthetic").exists())
            self.assertTrue((root / "voice-runtime/synthetic").exists())
            self.assertEqual((root / ".package-image").read_text().strip(),
                             "sha256:synthetic-two")


class PackageEntry(unittest.TestCase):
    def test_deb_and_rpm_commands_reuse_images_and_mount_cargo_home(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            # 最小仓库只带实际打包入口及其镜像、版本表，不运行产品编译。
            for relative in (
                "platforms/linux/package-container.sh", "platforms/linux/version.txt",
                "platforms/linux/scripts/package-image.py",
                "platforms/linux/scripts/edition_linux.py", "shared/contracts/editions.json",
                "rust-toolchain.toml", "platforms/linux/tests/tools/Dockerfile.build-gate",
                "platforms/linux/tests/tools/Dockerfile.package",
                "platforms/linux/tests/tools/Dockerfile.package-rpm",
            ):
                destination = root / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(ROOT / relative, destination)
            binary = root / "bin"
            binary.mkdir()
            docker = binary / "docker"
            docker.write_text('''#!/usr/bin/env python3
import json, os, sys
from pathlib import Path
root = Path(os.environ["MSIME_TEST_DOCKER_ROOT"])
state = root / "state.json"
images = json.loads(state.read_text()) if state.exists() else {}
args = sys.argv[1:]
with (root / "calls.jsonl").open("a") as log:
    log.write(json.dumps(args) + "\\n")
if args[0] == "info":
    print("linux/aarch64")
elif args[:2] == ["image", "inspect"]:
    if args[-1] not in images:
        sys.exit(1)
    print(images[args[-1]])
elif args[:2] == ["image", "tag"]:
    images[args[-1]] = args[-2]
elif args[0] == "build":
    sys.stdin.read()
    tag = args[args.index("-t") + 1]
    images[tag] = "sha256:" + tag.split(":", 1)[1]
elif args[0] != "run":
    sys.exit(2)
state.write_text(json.dumps(images))
''', encoding="utf-8")
            docker.chmod(0o755)
            env = dict(os.environ, PATH=f"{binary}:{os.environ['PATH']}",
                       MSIME_TEST_DOCKER_ROOT=str(root), MSIME_PACKAGE_DESKTOP="0",
                       MSIME_PACKAGE_EDITIONS="full", DOCKER_DEFAULT_PLATFORM="")
            for format, expected_builds in (("deb", 2), ("rpm", 1)):
                env["MSIME_PACKAGE_FORMAT"] = format
                log = root / "calls.jsonl"
                log.unlink(missing_ok=True)
                for version in ("0.0.1", "0.0.2"):
                    result = subprocess.run(
                        ["bash", str(root / "platforms/linux/package-container.sh"), version],
                        env=env, capture_output=True, text=True)
                    self.assertEqual(result.returncode, 0, result.stderr)
                calls = [json.loads(line) for line in log.read_text().splitlines()]
                self.assertEqual(sum(call[0] == "build" for call in calls), expected_builds)
                runs = [call for call in calls if call[0] == "run"]
                self.assertEqual(len(runs), 2)
                for call in runs:
                    self.assertIn("CARGO_HOME=/build/cargo-home", call)
                    identity = next(arg.split("=", 1)[1] for arg in call
                                    if arg.startswith("MSIME_PACKAGE_IMAGE="))
                    self.assertIn(identity, call)
                    suffix = "-rpm" if format == "rpm" else ""
                    self.assertIn(f"{root}/target/linux-package{suffix}:/build", call)
                self.assertIn("MSIME_VERSION=0.0.2", runs[-1])


if __name__ == "__main__":
    unittest.main()
