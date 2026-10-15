"""Attribute generator observations to its lifecycle, never a reused name."""
import json
import subprocess
import time

from watchdog import LABEL


def inspect_container(name):
    result = subprocess.run(['docker', 'inspect', name], capture_output=True, text=True, timeout=10)
    if result.returncode:
        if any(message in result.stderr.lower() for message in ['no such object', 'no such container']):
            return None
        raise RuntimeError('Generator inspection failed: '+result.stderr[-600:])
    return json.loads(result.stdout)[0]


def generator_observation(context, current):
    if context is None or context is not current() or context['process'].poll() is not None:
        return None
    container_id = context.get('container_id')
    if container_id is None:
        state = inspect_container(context['name'])
        if context is not current() or context['process'].poll() is not None:
            return None
        if state is None or not state['State']['Running']:
            if time.monotonic()-context['started'] > 30:
                raise RuntimeError('Generator did not become observable within 30 seconds')
            return None
        if (state['Config'].get('Labels') or {}).get(LABEL) != context['identity']:
            raise RuntimeError('Generator ownership changed')
        container_id = state['Id']
        context['container_id'] = container_id
    result = subprocess.run(['docker', 'stats', '--no-stream', '--format', '{{json .}}', container_id], capture_output=True, text=True, timeout=10)
    if context is not current() or context['process'].poll() is not None:
        return None
    if result.returncode or not result.stdout.strip():
        state = inspect_container(container_id)
        if state is None or not state['State']['Running']:
            return None  # The driver validates the completed process's exit code.
        raise RuntimeError('Live generator statistics failed: '+result.stderr[-600:])
    context['last_observed'] = time.monotonic()
    value = json.loads(result.stdout)
    value['phase'] = context['phase']
    value['container_id'] = container_id
    return value
