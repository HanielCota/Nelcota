"""Release gating tests use synthetic CI input; they are not VPS acceptance evidence."""
import importlib.util
import pathlib
import tempfile
import unittest
from unittest.mock import patch
ROOT = pathlib.Path(__file__).resolve().parents[2]


def module(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'scripts' / f'{name}.py')
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


ci = module('assert-update-ci')
receipt = module('build-update-manifest')


class UpdateReleaseTests(unittest.TestCase):
    def test_latest_exact_source_main_run_must_have_passed(self):
        run = dict(id=1, head_sha='a' * 40, head_branch='main', event='push',
                   path='.github/workflows/ci.yml', status='completed', conclusion='success')
        self.assertEqual(ci.validate(dict(workflow_runs=[run]), 'a' * 40), 1)
        for change in [dict(id=2, conclusion='failure'), dict(id=2, status='in_progress'),
                       dict(head_branch='feature'), dict(head_sha='b' * 40), dict(event='pull_request')]:
            runs = [{**run, **change}]
            if change.get('id') == 2:
                runs.append(run)
            with self.assertRaises(ValueError):
                ci.validate(dict(workflow_runs=runs), 'a' * 40)

    def test_manifest_binds_both_binaries_image_migrations_and_source_ci(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            (root / 'Cargo.toml').write_text('[workspace.package]\nversion="0.5.0"\n')
            (root / 'crates/core/src').mkdir(parents=True)
            (root / 'crates/core/src/config.rs').write_text('deployment_id')
            (root / 'crates/server/src').mkdir(parents=True)
            (root / 'crates/server/src/deployment.rs').write_text('validation contract fixture')
            (root / 'migrations').mkdir()
            (root / 'migrations/V15__fixture.sql').write_text('SELECT 1;')
            artifacts = root / 'artifacts'
            artifacts.mkdir()
            for arch in ('x86_64', 'aarch64'):
                (artifacts / f'nelcota-{arch}-unknown-linux-musl').write_bytes(b'synthetic artifact')
            with patch.object(receipt.subprocess, 'check_output', return_value='a' * 40 + '\n'):
                value = receipt.build(root, artifacts, 'v0.5.0', 'sha256:' + 'b' * 64,
                                      dict(source_sha='a' * 40, run_id=123))
                self.assertEqual(value['migration_version'], 15)
                self.assertEqual(value['ci_run_id'], 123)
                self.assertEqual(len(value['artifacts']), 2)
                self.assertTrue(all(len(entry['sha256']) == 64 for entry in value['artifacts']))
                for tag, digest, evidence in [('v0.6.0', 'sha256:' + 'b' * 64, dict(source_sha='a' * 40, run_id=123)),
                                              ('v0.5.0', 'latest', dict(source_sha='a' * 40, run_id=123)),
                                              ('v0.5.0', 'sha256:' + 'b' * 64, dict(source_sha='c' * 40, run_id=123))]:
                    with self.assertRaises(ValueError):
                        receipt.build(root, artifacts, tag, digest, evidence)
                (artifacts / 'nelcota-aarch64-unknown-linux-musl').unlink()
                with self.assertRaises(FileNotFoundError):
                    receipt.build(root, artifacts, 'v0.5.0', 'sha256:' + 'b' * 64, dict(source_sha='a' * 40, run_id=123))


if __name__ == '__main__':
    unittest.main()
