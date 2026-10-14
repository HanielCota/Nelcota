#!/usr/bin/env python3
"""Fail closed unless the latest main CI run passed for the exact release source."""
import json
import os
import pathlib
import subprocess


def validate(value: dict, source: str) -> int:
    runs = [run for run in value.get('workflow_runs', [])
            if run.get('head_sha') == source and run.get('head_branch') == 'main'
            and run.get('event') == 'push' and run.get('path') == '.github/workflows/ci.yml']
    if not runs:
        raise ValueError('release source has no main CI evidence')
    latest = max(runs, key=lambda run: run['id'])
    if latest.get('status') != 'completed' or latest.get('conclusion') != 'success':
        raise ValueError('latest main CI for release source has not passed')
    return int(latest['id'])


def main() -> None:
    source = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
    repository = os.environ['GITHUB_REPOSITORY']
    if repository != 'Nelcota/Nelcota':
        raise ValueError('only the official repository can publish readiness')
    value = json.loads(subprocess.check_output(['gh', 'api', f'/repos/{repository}/actions/workflows/ci.yml/runs?head_sha={source}&event=push&per_page=100'], text=True))
    run = validate(value, source)
    pathlib.Path('update-ci.json').write_text(json.dumps(dict(source_sha=source, run_id=run)) + '\n')


if __name__ == '__main__':
    main()
