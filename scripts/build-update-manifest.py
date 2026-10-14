#!/usr/bin/env python3
"""Create the release readiness receipt after both binaries and the OCI index exist."""
import argparse
import hashlib
import json
import pathlib
import re
import subprocess
import tomllib


def build(root: pathlib.Path, artifacts: pathlib.Path, tag: str, image: str, ci: dict) -> dict:
    version = tomllib.loads((root / 'Cargo.toml').read_text())['workspace']['package']['version']
    if tag != f'v{version}' or not re.fullmatch(r'\d+\.\d+\.\d+', version):
        raise ValueError('tag must match a stable workspace version')
    if tuple(map(int, version.split('.'))) < (0, 5, 0):
        raise ValueError('panel update readiness requires the first controller release or later')
    if not re.fullmatch(r'sha256:[a-f0-9]{64}', image):
        raise ValueError('expected a complete OCI index digest')
    source = subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip()
    if ci.get('source_sha') != source or not isinstance(ci.get('run_id'), int) or ci['run_id'] <= 0:
        raise ValueError('missing exact-source CI receipt')
    if not re.fullmatch(r'[a-f0-9]{40}', source):
        raise ValueError('invalid source revision')
    # Older tags must never acquire a validation capability by workflow dispatch.
    if 'deployment_id' not in (root / 'crates/core/src/config.rs').read_text() or not (root / 'crates/server/src/deployment.rs').is_file():
        raise ValueError('source does not support deployment validation')
    migrations = [int(path.name.split('__')[0][1:]) for path in (root / 'migrations').glob('V*__*.sql')]
    entries = []
    for architecture in ('x86_64', 'aarch64'):
        name = f'nelcota-{architecture}-unknown-linux-musl'
        binary = artifacts / name
        size = binary.stat().st_size
        if not 0 < size <= 256 * 1024 * 1024:
            raise ValueError('invalid binary size')
        with binary.open('rb') as file:
            digest = hashlib.file_digest(file, 'sha256').hexdigest()
        entries.append(dict(architecture=architecture, filename=name, bytes=size,
                            sha256=digest,
                            image_digest=image.removeprefix('sha256:')))
    return dict(schema=1, ci_run_id=ci['run_id'], version=version, source_sha=source, minimum_controller='0.5.0',
                protocol=1, minimum_app='0.4.0', validation_mode=True,
                migration_version=max(migrations), artifacts=entries)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--artifacts', type=pathlib.Path, required=True)
    parser.add_argument('--tag', required=True)
    parser.add_argument('--ci', type=pathlib.Path, required=True)
    parser.add_argument('--image-digest', required=True)
    parser.add_argument('--output', type=pathlib.Path, default=pathlib.Path('update-manifest.json'))
    args = parser.parse_args()
    manifest = build(pathlib.Path(__file__).resolve().parents[1], args.artifacts, args.tag, args.image_digest, json.loads(args.ci.read_text()))
    args.output.write_text(json.dumps(manifest, indent=2) + '\n')


if __name__ == '__main__':
    main()
