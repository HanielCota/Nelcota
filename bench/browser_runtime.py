"""Stage public browser tooling in owned files, without mounting a real home."""
import hashlib
import importlib.metadata
import importlib.util
import json
import os
import platform
from pathlib import Path
import re
import shutil
import subprocess
import sys
import sysconfig

MAXIMUM_BYTES = 1536*1024*1024


def runtime_target(destination, target):
    root, path = Path(destination).resolve(), Path(target).resolve()
    if not path.is_relative_to(root) or path == root:
        raise ValueError('Browser runtime metadata cannot write outside its owned root')
    return path


def runtime_library(source, roots):
    """Validate the resolved library, including standard ELF loader aliases."""
    resolved = Path(source).resolve(strict=True)
    if not resolved.is_file() or not any(resolved.is_relative_to(Path(root).resolve(strict=True)) for root in roots):
        raise ValueError('Unexpected resolved browser runtime dependency path')
    return resolved


def prepare(destination, sdk):
    if platform.machine() != 'x86_64':
        raise RuntimeError('The native browser runtime profile is validated only on x86_64')
    destination = Path(destination).resolve()
    if destination.exists():
        raise ValueError('Preserve an existing browser runtime')
    chrome = Path('/usr/lib/chromium')
    python = Path(sys.prefix)
    executable = Path(sys.executable).resolve()
    sdk = Path(sdk).resolve(strict=True)
    if not (chrome/'chromium').is_file() or not (sdk/'index.js').is_file():
        raise RuntimeError('Native Chromium and freshly built SDK are required')
    destination.mkdir()
    records = []
    copied = 0
    def copy(source, target):
        nonlocal copied
        target = runtime_target(destination, target)
        source = source.resolve(strict=True)
        if not source.is_file() or source.stat().st_size > 512*1024*1024:
            raise ValueError('Browser runtime input exceeds its file boundary')
        copied += source.stat().st_size
        if copied > MAXIMUM_BYTES:
            raise ValueError('Browser runtime exceeds its 1536-MiB disk budget')
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)  # Do not inherit setuid bits or file caps.
        target.chmod(0o444)
        if source.stat().st_mode & 0o111:
            target.chmod(0o555)
        with target.open('rb') as stream:
            digest = hashlib.file_digest(stream, 'sha256').hexdigest()
        records.append({'file':str(target.relative_to(destination)), 'bytes':target.stat().st_size, 'sha256':digest})
    def tree(source, target, exclusions=()):
        for directory, names, files in os.walk(source):
            names[:] = sorted(name for name in names if name not in exclusions and not (Path(directory)/name).is_symlink())
            for name in sorted(files):
                item = Path(directory)/name
                if name.endswith(('.pyc', '.pyo')) or item.is_symlink():
                    continue
                copy(item, target/item.relative_to(source))
    tree(chrome, destination/'chrome')
    copy(executable, destination/'python/bin/python3')
    stdlib = Path(sysconfig.get_path('stdlib'))
    tree(stdlib, destination/'python/lib'/stdlib.name,
         ('site-packages', '__pycache__', 'test', 'tests', 'idlelib', 'ensurepip', 'turtledemo'))
    site = destination/'python/lib'/stdlib.name/'site-packages'
    for package in ['playwright', 'pyee', 'greenlet', 'typing_extensions']:
        spec = importlib.util.find_spec(package)
        if spec is None or spec.origin is None:
            raise RuntimeError('A required installed browser dependency is missing')
        source = Path(spec.origin)
        if source.name == '__init__.py':
            tree(source.parent, site/package, ('__pycache__', 'tests', 'test'))
        if source.name != '__init__.py':
            copy(source, site/source.name)
        distribution = importlib.metadata.distribution(package)
        for entry in distribution.files or []:
            if '.dist-info/' in str(entry) and distribution.locate_file(entry).is_file():
                copy(distribution.locate_file(entry), site/entry)
    for item in python.joinpath('lib').glob('libpython*.so*'):
        if item.is_file() and not item.is_symlink():
            copy(item, destination/'python/lib'/item.name)
    for item in sorted(sdk.rglob('*.js')):
        if item.is_symlink() or not item.resolve().is_relative_to(sdk):
            raise ValueError('SDK runtime cannot follow external asset links')
        copy(item, destination/'sdk'/item.relative_to(sdk))
    binaries = [chrome/'chromium', executable, Path(shutil.which('certutil') or '/missing-certutil'),
                *stdlib.glob('lib-dynload/*.so'),
                *[Path(shutil.which(name) or '/missing-tool') for name in ['sh', 'cat', 'ls', 'wc', 'du']]]
    binaries.extend((destination/'python').rglob('node'))
    libraries = set()
    for binary in binaries:
        result = subprocess.run(['ldd', str(binary)], capture_output=True, text=True, timeout=10)
        if result.returncode or 'not found' in result.stdout:
            raise RuntimeError('Native browser dependency closure is incomplete: '+binary.name)
        for name in re.findall(r'(/[^\s()]+)', result.stdout):
            source = Path(name)
            runtime_library(source, [Path('/lib'), Path('/usr/lib'), python, destination])
            libraries.add(source)
    for pattern in ['libnss*.so*', 'libsoftokn*.so*', 'libfreebl*.so*']:
        libraries.update(Path('/usr/lib/x86_64-linux-gnu').glob(pattern))
    names = {}
    for library in sorted(libraries):
        resolved = library.resolve(strict=True)
        if library.name in names and names[library.name] != resolved:
            raise ValueError('Ambiguous browser runtime library names')
        if library.name in names:
            continue
        names[library.name] = resolved
        copy(library, destination/'libs'/library.name)
    fonts = Path('/usr/share/fonts/truetype/dejavu')
    for name in ['DejaVuSans.ttf', 'DejaVuSans-Bold.ttf', 'DejaVuSansMono.ttf']:
        copy(fonts/name, destination/'fonts'/name)
    font_config = '<?xml version="1.0"?><fontconfig><dir>/runtime/fonts</dir><cachedir>/home/ubuntu/.cache/fontconfig</cachedir></fontconfig>\n'
    (destination/'fonts.conf').write_text(font_config)
    records.append({'file':'fonts.conf', 'bytes':len(font_config.encode()), 'sha256':hashlib.sha256(font_config.encode()).hexdigest()})
    copied += len(font_config.encode())
    if copied > MAXIMUM_BYTES:
        raise ValueError('Browser runtime exceeds its logical payload budget')
    profile = {'kind':'owned development browser tooling, not a production dependency',
               'bytes':copied, 'maximum_bytes':MAXIMUM_BYTES, 'files':len(records),
               'python_version':sys.version.split()[0], 'playwright_version':importlib.metadata.version('playwright'),
               'sdk_assets':len(list((destination/'sdk').rglob('*.js')))}
    manifest = json.dumps({'profile':profile, 'files':records}, sort_keys=True, indent=2)+'\n'
    (destination/'runtime-manifest.json').write_text(manifest)
    profile['manifest_sha256'] = hashlib.sha256(manifest.encode()).hexdigest()
    return profile
