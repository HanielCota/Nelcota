"""Verify user installation and checksum protection before binary replacement."""

import hashlib
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class Installer(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.tools = self.root / "tools"
        self.tools.mkdir()
        self.binary = self.root / "binary"
        self.binary.write_text("#!/bin/sh\nprintf 'nelcota fixture-version\\n'\n")
        self.checksum = self.root / "checksum"
        self.checksum.write_text(hashlib.sha256(self.binary.read_bytes()).hexdigest() +
                                 "  nelcota-x86_64-unknown-linux-musl\n")
        self.tool("id", "#!/bin/sh\nprintf '1000\\n'\n")
        self.tool("uname", "#!/bin/sh\ncase \"$1\" in -s) echo Linux;; -m) echo x86_64;; esac\n")
        self.tool("sudo", "#!/bin/sh\necho unexpected-sudo >&2\nexit 99\n")
        self.tool("curl", """#!/bin/sh
set -eu
case "$2" in *.sha256) source_file=$FIXTURE_CHECKSUM;; *) source_file=$FIXTURE_BINARY;; esac
cp "$source_file" "$4"
""")
        self.environment = dict(os.environ, PATH=str(self.tools) + os.pathsep + os.environ["PATH"],
                                PREFIX=str(self.root / "user/bin"), NELCOTA_VERSION="fixture",
                                NELCOTA_SKIP_DOCKER="1", FIXTURE_BINARY=str(self.binary),
                                FIXTURE_CHECKSUM=str(self.checksum))

    def tool(self, name, source):
        path = self.tools / name
        path.write_text(source)
        path.chmod(0o755)

    def install(self):
        return subprocess.run(["sh", str(ROOT / "scripts/install.sh")], env=self.environment,
                              capture_output=True, text=True, timeout=10)

    def test_writable_prefix_needs_no_sudo_and_handles_spaces(self):
        self.environment["PREFIX"] = str(self.root / "user folder/bin")
        result = self.install()
        self.assertEqual(result.returncode, 0, result.stderr)
        destination = Path(self.environment["PREFIX"]) / "nelcota"
        self.assertEqual(destination.read_bytes(), self.binary.read_bytes())
        self.assertEqual(destination.stat().st_mode & 0o777, 0o755)

    def test_bad_checksum_preserves_existing_installation(self):
        destination = Path(self.environment["PREFIX"]) / "nelcota"
        destination.parent.mkdir(parents=True)
        destination.write_text("previous installation")
        self.checksum.write_text("0" * 64 + "  nelcota-x86_64-unknown-linux-musl\n")
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("checksum does not match", result.stderr)
        self.assertEqual(destination.read_text(), "previous installation")



if __name__ == "__main__":
    unittest.main()
