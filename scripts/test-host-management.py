"""Run owned Docker fixtures; provenance, DNS, TLS and proxy reload are explicit doubles."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import uuid

ROOT = Path(__file__).resolve().parents[1]
TEST = 'updates::management::tests::docker_lifecycle_preserves_other_projects_and_verified_restore_writes'


def run(args, **kwargs):
    return subprocess.run(args, check=True, **kwargs)


def docker_plugin(name):
    candidates = [
        Path('/usr/libexec/docker/cli-plugins'),
        Path('/usr/lib/docker/cli-plugins'),
        Path('/usr/local/lib/docker/cli-plugins'),
        Path('/usr/local/libexec/docker/cli-plugins'),
        Path(os.environ.get('DOCKER_CONFIG', str(Path.home() / '.docker'))) / 'cli-plugins',
    ]
    for directory in candidates:
        plugin = directory / f'docker-{name}'
        if plugin.is_file():
            return plugin.resolve(strict=True)
    raise RuntimeError(f'Docker {name} plugin is required for this acceptance test')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/debug/nelcota')
    args = parser.parse_args()
    reports = ROOT / '.tools/reports/host-management'
    reports.mkdir(parents=True, exist_ok=True)
    (reports / 'latest.json').write_text(json.dumps({
        'status': 'running', 'test': TEST, 'production_acceptance': False
    }, indent=2) + '\n')
    binary = args.binary.resolve(strict=True)
    compiled = subprocess.run(['cargo', 'test', '--locked', '-p', 'nelcota-cli', '--lib', '--no-run', '--message-format=json'], cwd=ROOT, text=True, stdout=subprocess.PIPE)
    if compiled.returncode:
        for line in compiled.stdout.splitlines():
            if line.startswith('{'):
                message = json.loads(line)
                if message.get('reason') == 'compiler-message':
                    print(message['message'].get('rendered', 'Compilation failed'))
        raise RuntimeError('Fixture compilation failed')
    artifacts = [json.loads(line) for line in compiled.stdout.splitlines() if line.startswith('{')]
    test_binary = Path(next(item['executable'] for item in artifacts if item.get('executable') and item.get('target', {}).get('name') == 'nelcota_cli'))
    temporary = tempfile.TemporaryDirectory(dir=reports)
    root = Path(temporary.name).resolve()
    context = root / 'context'
    context.mkdir()
    shutil.copyfile(binary, context / 'nelcota')
    shutil.copyfile(test_binary, context / 'fixture-test')
    shutil.copyfile('/etc/ssl/certs/ca-certificates.crt', context / 'ca-certificates.crt')
    (context / 'Dockerfile').write_text('FROM debian:trixie-slim\nCOPY --chmod=755 nelcota /usr/local/bin/nelcota\nCOPY --chmod=755 fixture-test /usr/local/bin/fixture-test\nCOPY ca-certificates.crt /etc/ssl/certs/ca-certificates.crt\nUSER 65532:65532\nENTRYPOINT ["/usr/local/bin/nelcota"]\n')
    tag = f'nelcota-host-fixture:{uuid.uuid4().hex}'
    docker_config = reports / 'docker-config'
    docker_config.mkdir(exist_ok=True)
    compose = docker_plugin('compose')
    buildx = docker_plugin('buildx')
    docker = Path(shutil.which('docker') or '').resolve(strict=True)
    os.environ['DOCKER_CONFIG'] = str(docker_config)
    uid = os.getuid()
    gid = os.getgid()
    built = False
    try:
        run(['docker', 'build', '-q', '-t', tag, str(context)], stdout=subprocess.DEVNULL)
        built = True
        image = run(['docker', 'image', 'inspect', tag, '--format', '{{.Id}}'], text=True, stdout=subprocess.PIPE).stdout.strip()
        run(['docker', 'run', '--rm', '--user', '0:0', '--mount', f'type=bind,src={root},dst={root}', '--entrypoint', 'chown', tag, '0:0', str(root)])
        command = [
            'docker', 'run', '--rm', '--read-only', '--user', '0:0',
            '--network', 'host', '--memory', '1g', '--pids-limit', '128',
            '--cap-drop', 'ALL', '--cap-add', 'CHOWN', '--cap-add', 'FOWNER',
            '--cap-add', 'DAC_OVERRIDE', '--tmpfs', '/tmp:rw,mode=1777',
            '--mount', f'type=bind,src={root},dst={root}',
            '--mount', 'type=bind,src=/var/run/docker.sock,dst=/var/run/docker.sock',
            '--mount', f'type=bind,src={docker},dst=/usr/local/bin/docker,readonly',
            '--mount', f'type=bind,src={compose},dst=/usr/libexec/docker/cli-plugins/docker-compose,readonly',
            '--mount', f'type=bind,src={buildx},dst=/usr/libexec/docker/cli-plugins/docker-buildx,readonly',
            '-e', 'DOCKER_CONFIG=/tmp/docker-config',
            '-e', f'NELCOTA_HOST_FIXTURE_ROOT={root}',
            '-e', f'NELCOTA_HOST_TEST_IMAGE={image}',
            '--entrypoint', '/usr/local/bin/fixture-test', tag, TEST,
            '--exact', '--ignored', '--test-threads=1',
        ]
        run(command, timeout=600)
        (reports / 'latest.json').write_text(json.dumps({'status': 'passed', 'test': TEST, 'real': ['Docker project creation', 'isolated PostgreSQL', 'paired database/files backup', 'restore', 'post-verification recovery', 'missing safety backup protection', 'partial removal/recreation', 'worker policy reload', 'Docker namespace collision protection', 'old project JWT rejection', 'safe removal'], 'doubles': ['DNS and image provenance', 'public TLS', 'Caddy reload', 'unused PITR image extension'], 'production_acceptance': False}, indent=2) + '\n')
    finally:
        # Return only this owned fixture directory to the caller for cleanup.
        if built and root.exists():
            subprocess.run(['docker', 'run', '--rm', '--user', '0:0', '--mount', f'type=bind,src={root},dst=/fixture', '--entrypoint', 'chown', tag, '-R', f'{uid}:{gid}', '/fixture'], check=False)
        if built:
            subprocess.run(['docker', 'image', 'rm', tag], stdout=subprocess.DEVNULL, check=False)
        temporary.cleanup()


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        reports = ROOT / '.tools/reports/host-management'
        reports.mkdir(parents=True, exist_ok=True)
        (reports / 'latest.json').write_text(json.dumps({
            'status': 'failed', 'test': TEST, 'error_type': type(error).__name__,
            'production_acceptance': False,
        }, indent=2) + '\n')
        raise
