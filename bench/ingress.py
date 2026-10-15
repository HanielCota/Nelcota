"""Owned real Caddy with verified local HTTPS; no public DNS/ACME claim."""
import hashlib
import json
import os
from pathlib import Path
import ssl
import time
import urllib.error
import urllib.request

from campaign import free_port, owns_listener
from watchdog import LABEL, atomic_json
from disk_usage import host as host_disk_usage

CADDY = 'caddy:2-alpine'


def configuration(frontends, upstreams):
    if not frontends or len(frontends) != len(upstreams) or len(frontends) > 10:
        raise ValueError('Caddy fixture needs one frontend per owned project')
    if any(isinstance(port, bool) or not isinstance(port, int) or not 1024 <= port <= 65535 for port in [*frontends, *upstreams]):
        raise ValueError('Caddy fixture accepts only unprivileged loopback ports')
    if len(set(frontends)) != len(frontends) or len(set(upstreams)) != len(upstreams) or set(frontends) & set(upstreams):
        raise ValueError('Caddy frontend ports must be distinct from all backends')
    blocks = ['{\n admin unix//config/admin.sock\n auto_https disable_redirects\n skip_install_trust\n}\n']
    for port, upstream in zip(frontends, upstreams):
        blocks.append(f'https://localhost:{port} {{\n bind 127.0.0.1\n tls internal\n @compressible not path /storage/*\n encode @compressible zstd gzip\n reverse_proxy 127.0.0.1:{upstream}\n header {{\n  -Server\n  Strict-Transport-Security "max-age=31536000"\n  X-Content-Type-Options "nosniff"\n }}\n}}\n')
    return '\n'.join(blocks)


class Ingress:
    def __init__(self, identity, directory, owned, cpu):
        self.identity = identity
        self.directory = Path(directory) / 'caddy'
        self.owned = owned
        self.cpu = cpu
        self.name = identity + '-proxy'
        self.bundle = self.directory / 'verified-ca.pem'
        self.frontends = []

    def start(self, projects, output):
        image = json.loads(self.owned('image', 'inspect', CADDY))[0]
        self.directory.mkdir()
        for name in ['data', 'config', 'etc']:
            (self.directory / name).mkdir()
        self.frontends = [free_port() for _ in projects]
        content = configuration(self.frontends, [project.port for project in projects])
        path = self.directory / 'etc/Caddyfile'
        path.write_text(content)
        path.chmod(0o444)
        self.owned('run', '-d', '--name', self.name, '--label', LABEL+'='+self.identity,
                   '--pids-limit', '64', '--network', 'host', '--cpus', '.25', '--cpuset-cpus', str(self.cpu),
                   '--memory', '128m', '--memory-swap', '128m', '--user', f'{os.getuid()}:{os.getgid()}',
                   '--cap-drop', 'ALL', '--read-only', '--security-opt', 'no-new-privileges',
                   '--tmpfs', f'/tmp:rw,noexec,nosuid,size=16m,uid={os.getuid()},gid={os.getgid()}',
                   '-v', str(self.directory/'etc')+':/etc/caddy:ro',
                   '-v', str(self.directory/'data')+':/data', '-v', str(self.directory/'config')+':/config',
                   image['Id'], 'caddy', 'run', '--config', '/etc/caddy/Caddyfile', '--adapter', 'caddyfile')
        ca = self.directory / 'data/caddy/pki/authorities/local/root.crt'
        deadline = time.monotonic() + 30
        while not ca.is_file():
            if time.monotonic() > deadline:
                raise RuntimeError('Owned Caddy CA readiness deadline')
            time.sleep(.1)
        if ca.stat().st_size > 65536:
            raise RuntimeError('Caddy CA exceeded its declared size')
        trust = Path('/etc/ssl/certs/ca-certificates.crt').read_bytes()
        self.bundle.write_bytes(trust + b'\n' + ca.read_bytes())
        self.bundle.chmod(0o444)
        context = ssl.create_default_context(cafile=str(self.bundle))
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), urllib.request.HTTPSHandler(context=context))
        for project, port in zip(projects, self.frontends):
            base = f'https://localhost:{port}'
            while True:
                try:
                    if not owns_listener(self.owned.require(self.name), port):
                        raise RuntimeError('Caddy listener ownership not ready')
                    with opener.open(base+'/auth/v1/.well-known/jwks.json', timeout=2) as response:
                        if response.status != 200:
                            raise RuntimeError('Caddy positive HTTPS probe failed')
                        json.loads(response.read(65536))
                    break
                except (OSError, urllib.error.URLError, RuntimeError):
                    if time.monotonic() > deadline:
                        raise RuntimeError('Verified Caddy listener readiness deadline') from None
                    time.sleep(.1)
            project.fixture['base'] = base
        untrusted = urllib.request.build_opener(urllib.request.ProxyHandler({}), urllib.request.HTTPSHandler(context=ssl.create_default_context()))
        try:
            with untrusted.open(f'https://localhost:{self.frontends[0]}/auth/v1/.well-known/jwks.json', timeout=2):
                raise RuntimeError('Untrusted local Caddy certificate was accepted')
        except urllib.error.URLError as error:
            if not isinstance(error.reason, ssl.SSLCertVerificationError):
                raise RuntimeError('Untrusted CA control did not fail certificate verification') from None
        profile = {'kind': 'real Caddy with verified internal CA on loopback', 'cpus': .25,
                   'cpu_affinity': self.cpu, 'memory_mib': 128, 'pids_limit': 64,
                   'fixture_disk_mib': 64,
                   'image_digests': image['RepoDigests'], 'public_dns_acme_certified': False}
        atomic_json(Path(output)/'proxy.json', {**profile, 'ca_sha256': hashlib.sha256(ca.read_bytes()).hexdigest(),
                                               'config_sha256': hashlib.sha256(content.encode()).hexdigest(),
                                               'https_frontends': len(self.frontends), 'tls_verification': True,
                                               'untrusted_ca_rejected': True})
        return profile

    def observe(self):
        value = host_disk_usage(self.directory)
        if value['value'] > 64:
            raise RuntimeError('Caddy fixture disk budget exceeded')
        return value
