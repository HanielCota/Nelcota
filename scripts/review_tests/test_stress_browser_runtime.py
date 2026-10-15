"""Installed package metadata must not redirect writes outside owned staging."""
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT/'bench'))
from browser_runtime import runtime_target


class BrowserRuntimeOwnership(unittest.TestCase):
    def test_parent_traversal_from_package_metadata_is_refused(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)/'runtime'
            root.mkdir()
            with self.assertRaises(ValueError):
                runtime_target(root, root/'../../../foreign.dist-info/METADATA')

    def test_symlink_alias_cannot_escape_to_another_fixture(self):
        with tempfile.TemporaryDirectory() as temporary:
            root, other = Path(temporary)/'runtime', Path(temporary)/'foreign'
            root.mkdir();other.mkdir()
            (root/'package').symlink_to(other, target_is_directory=True)
            with self.assertRaises(ValueError):
                runtime_target(root, root/'package/METADATA')
            self.assertEqual(list(other.iterdir()), [])

    def test_fresh_nested_owned_asset_is_accepted(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)/'runtime'
            root.mkdir()
            target = root/'python/lib/site-packages/package.dist-info/METADATA'
            self.assertEqual(runtime_target(root, target), target)


if __name__ == '__main__':
    unittest.main()
