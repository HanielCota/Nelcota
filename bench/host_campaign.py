"""Bounded independent 1/5/10-project clusters; local Docker ingress only.

This measures application/database isolation and shared CPU interference.
It does not certify the production controller, Caddy, public TLS or a VPS.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import secrets
import statistics
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request

from campaign import K6, ResourceCoverage, docker, effective_cpus, free_port, nominal_violations, owns_listener, validate_disk_headroom, validate_storage_filesystem
from metrics import NativeMetrics, PROMETHEUS
from observations import generator_observation
from projects import Project
from watchdog import LABEL, atomic_json, cleanup, host_reserves, process_identity, stop_generator
from host_resources import snapshot as host_snapshot, owned_cgroup_snapshot, require_no_oom
from ownership import LOG_PROFILE,OwnedDocker
from disk_usage import container as container_disk_usage
from fleet_webhooks import FleetWebhooks
from ingress import Ingress

ROOT = Path(__file__).resolve().parents[1]


def fairness_violations(baseline, candidate, offered, seconds):
    failures = []
    for label, report in candidate.items():
        if label == 'project-0':
            continue
        current = report['metrics']
        previous = baseline[label]['metrics']
        rates = [current['successful_operations']['rate'],previous['successful_operations']['rate']]
        if any(isinstance(value,bool) or not isinstance(value,(int,float)) or not math.isfinite(value) or value <= 0 for value in rates):
            failures.append({'project':label,'throughput':'invalid evidence'})
            continue
        if current['successful_operations']['rate'] < previous['successful_operations']['rate'] * .9:
            failures.append({'project': label, 'throughput': 'below 90% of balanced baseline'})
        for name in ['read', 'rls', 'panel', 'storage', 'login', 'refresh', 'write']:
            key = 'latency_' + name
            values = [current.get(key,{}).get('p(95)'),previous.get(key,{}).get('p(95)')]
            if any(isinstance(value,bool) or not isinstance(value,(int,float)) or not math.isfinite(value) or value <= 0 for value in values):
                failures.append({'project':label,'workload':name,'p95':'invalid evidence'})
                continue
            if values[0] > values[1] * 1.2:
                failures.append({'project': label, 'workload': name, 'p95': 'missing or more than 20% worse'})
        failures.extend({'project': label, **item} for item in nominal_violations(report, 'mixed'))
        if current['iterations']['count'] < offered * seconds * .99 or current['dropped_iterations']['count'] > offered * seconds * .01 or current['overload_rejections']['count']:
            failures.append({'project': label, 'delivered': 'nominal delivery or rejection gate failed'})
    return failures


def median_baseline(samples):
    if len(samples) != 3 or not samples[0] or any(set(sample) != set(samples[0]) for sample in samples):
        raise ValueError('Fairness baseline requires three matching project samples')
    result = {}
    for label in samples[0]:
        metrics = {'successful_operations': {'rate': statistics.median(sample[label]['metrics']['successful_operations']['rate'] for sample in samples)}}
        for name in ['read','rls','panel','storage','login','refresh','write']:
            key = 'latency_'+name
            metrics[key] = {'p(95)':statistics.median(sample[label]['metrics'][key]['p(95)'] for sample in samples)}
        result[label] = {'metrics':metrics}
    return result


def campaign(args):
    output = args.output.resolve()
    if output.exists() and any(output.iterdir()):
        raise RuntimeError('Preserve previous evidence; use a fresh output directory')
    output.mkdir(parents=True, exist_ok=True)
    cpus = sorted(os.sched_getaffinity(0))
    if len(cpus) < 4 or effective_cpus(cpus) < 3.5 or host_reserves(output):
        raise RuntimeError('Host cannot cover the declared CPU, memory and disk reserves')
    if args.caddy and (len(cpus) < 5 or effective_cpus(cpus) < 4):
        raise RuntimeError('Real Caddy profile requires five affinity slots and four effective CPUs including reserves')
    validate_disk_headroom(output,args.disk_mib,startup_mib=args.projects*400+512)
    identity = 'nelcota-fleet-' + secrets.token_hex(16)
    owned=OwnedDocker(identity,docker)
    telemetry, generator = identity + '-metrics', identity + '-load'
    telemetry_volume = identity + '-metrics-data'
    directory = Path(tempfile.mkdtemp(prefix='fixture-', dir=output))
    filesystem = subprocess.check_output(['stat', '-f', '-c', '%T', str(directory)], text=True).strip()
    validate_storage_filesystem('disk', filesystem)
    projects = [Project(identity, index, args.projects, args.binary.resolve(), directory, cpus) for index in range(args.projects)]
    for project in projects:
        project.webhooks_enabled = args.webhooks
    webhooks = FleetWebhooks(output)
    ingress = None
    if args.caddy:
        ingress = Ingress(identity, directory, owned, cpus[3])
    began = time.monotonic()
    phase = 'preparation'
    resource_at = began
    load_seconds = 0
    coverage = ResourceCoverage()
    stop = threading.Event()
    critical = threading.Event()
    violations = []
    active_generator = None
    heartbeat_thread = sampler = watchdog = None
    status = 'failed'
    manifest = {'identity': identity, 'output': str(output), 'reserve_path': str(output), 'pid': os.getpid(), 'process_identity': process_identity(os.getpid()), 'started_monotonic': began, 'maximum_seconds': 7200, 'preparation_seconds': 900, 'containers': [telemetry, generator, *[item.app for item in projects], *[item.pg for item in projects]], 'volumes': [telemetry_volume, *[item.volume for item in projects]]}
    if ingress is not None:
        manifest['containers'].append(ingress.name)
    atomic_json(output/'ownership.json', manifest)
    previous = signal.getsignal(signal.SIGTERM)

    def terminate(signum, frame):
        critical.set()
        stop_generator(manifest)
        raise RuntimeError('Fleet terminated by watchdog or owner')

    def heartbeat():
        while not stop.is_set():
            atomic_json(output/'heartbeat.json', {'phase': phase, 'monotonic': time.monotonic(), 'resource_monotonic': resource_at, 'load_seconds_completed': load_seconds})
            stop.wait(2)

    signal.signal(signal.SIGTERM, terminate)
    atomic_json(output/'heartbeat.json', {'phase': phase, 'monotonic': began, 'resource_monotonic': began})
    heartbeat_thread = threading.Thread(target=heartbeat, daemon=True)
    heartbeat_thread.start()
    try:
        watchdog = subprocess.Popen([sys.executable, str(ROOT/'bench/watchdog.py'), str(output/'ownership.json')], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
        atomic_json(output/'manifest.json', {'schema': 1, 'scope': 'local independent app/PG clusters, no proxy/controller', 'projects': args.projects, 'rows_per_project': 1000, 'shared_user_id': '00000000-0000-4000-8000-000000000001', 'binary_sha256': hashlib.file_digest(args.binary.open('rb'), 'sha256').hexdigest(), 'cpu_effective': effective_cpus(cpus), 'app_aggregate_cpus': 1, 'postgres_aggregate_cpus': 1, 'generator_cpus': 1, 'metrics_cpus': .25, 'app_memory_mib_each': 128, 'postgres_memory_mib_each': 256, 'max_arrival_rate': 10000, 'fixture_disk_mib': args.disk_mib, 'storage_filesystem': filesystem, 'seconds_per_step': args.seconds, 'certifies_five_minute_steps': args.seconds >= 300, 'base_rate_per_project': args.rate, 'SLOs': 'G05 unchanged'})
        fixtures = [project.create() for project in projects]
        if args.webhooks:
            for project in projects:
                webhooks.attach(project)
        profile=json.loads((output/'manifest.json').read_text())
        profile['ingress'] = {'kind': 'direct loopback HTTP'}
        if ingress is not None:
            profile['scope'] = 'local independent app/PG clusters with verified local Caddy, no public DNS/controller'
            profile['ingress'] = ingress.start(projects, output)
        profile['webhooks'] = {'enabled': args.webhooks, 'receiver': 'owned loopback, verified signatures and durable event identities', 'evidence_limit_mib': 128, 'drain_seconds': 30}
        profile['resource_protocol']='component cgroup v2 plus host/process owners; allocated du totals count only disappearing child entries'
        profile['container_log_profile']=LOG_PROFILE
        profile['harness_js_sha256']=hashlib.file_digest((ROOT/'bench/campaign.js').open('rb'),'sha256').hexdigest()
        profile['client_initialization']={'maximum_sessions_per_project':128,'bootstrap_concurrency':4,'setup_timeout_seconds':120,'fully_preallocated_arrival_VUs':True,'mix_schedule':'global scenario arrival ordinal','bootstrap_excluded_from_nominal_metrics':True}
        profile.update({'schema':2,'cpu_affinity':cpus[:3],'metrics_cpu_affinity':cpus[-1],'capacity_qualification':args.capacity,'provisional_rate_per_project':args.provisional_rate,'max_connections_each':32,'pool_each':4,'generator_memory_mib':512,'metrics_memory_mib':512,'images':{image:json.loads(owned('image','inspect',image))[0]['RepoDigests'] for image in ['ubuntu:24.04','postgres:17-alpine',K6,PROMETHEUS]}})
        atomic_json(output/'manifest.json',profile)
        for project in projects:
            for foreign in fixtures:
                if foreign['label'] != project.label:
                    project.assert_foreign_denied(foreign)
        atomic_json(output/'cross-project-preflight.json', {'distinct_keys': True, 'directed_rejections': args.projects*(args.projects-1), 'status': 'passed'})
        metrics_port = free_port()
        metrics_url = f'http://127.0.0.1:{metrics_port}'
        config = directory/'prometheus.yml'
        config.write_text('global:\n  scrape_interval: 15s\nscrape_configs: []\n')
        config.chmod(0o444)
        owned('volume', 'create', '--label', LABEL+'='+identity, telemetry_volume)
        owned('run', '-d', '--name', telemetry, '--label', LABEL+'='+identity, '--pids-limit', '128', '--network', 'host', '--cpus', '.25', '--cpuset-cpus', str(cpus[-1]), '--memory', '512m', '--memory-swap', '512m', '--user', '65534:65534', '--cap-drop', 'ALL', '--read-only', '--security-opt', 'no-new-privileges', '--mount', f'type=volume,source={telemetry_volume},destination=/prometheus', '-v', str(config)+':/etc/prometheus/prometheus.yml:ro', PROMETHEUS, '--config.file=/etc/prometheus/prometheus.yml', '--web.listen-address=127.0.0.1:'+str(metrics_port), '--web.enable-remote-write-receiver', '--enable-feature=native-histograms', '--storage.tsdb.retention.time=1h', '--storage.tsdb.retention.size=128MB', '--storage.tsdb.wal-segment-size=16MB', '--storage.tsdb.min-block-duration=15m', '--storage.tsdb.max-block-duration=15m')
        for _ in range(120):
            try:
                if owns_listener(owned.require(telemetry), metrics_port):
                    with urllib.request.urlopen(metrics_url+'/-/ready', timeout=2):
                        break
            except (OSError, RuntimeError):
                pass
            time.sleep(.25)
        metrics = NativeMetrics(metrics_url)
        metrics.value('vector(1)')
        observed_containers = [item.app for item in projects]+[item.pg for item in projects]+[telemetry]
        if ingress is not None:
            observed_containers.append(ingress.name)
        states = json.loads(owned('inspect', *observed_containers))
        process_ids = {state['Name']: state['State']['Pid'] for state in states}
        resource_began = time.monotonic()
        resource_at = resource_began

        def sample():
            nonlocal resource_at
            while not stop.is_set():
                cycle = time.monotonic()
                try:
                    reason = host_reserves(output)
                    if reason:
                        raise RuntimeError(reason)
                    states = json.loads(owned('inspect', *observed_containers))
                    if any(not state['State']['Running'] or state['State']['OOMKilled'] or state['RestartCount'] or state['State']['Pid'] != process_ids[state['Name']] for state in states):
                        raise RuntimeError('Fixture unexpectedly restarted or OOMed')
                    memory_events={}
                    cgroups={}
                    for state in states:
                        cgroups[state['Name']]=owned_cgroup_snapshot(owned,state['Name'])
                        events=cgroups[state['Name']]['memory.events']
                        memory_events[state['Name']]=events
                        try:
                            require_no_oom(events)
                        except (ValueError,RuntimeError):
                            atomic_json(output/'memory-events-violation.json',memory_events)
                            raise
                    stats = [json.loads(line) for line in owned('stats', '--no-stream', '--format', '{{json .}}', *observed_containers).splitlines()]
                    if len(stats) != len(observed_containers):
                        raise RuntimeError('Incomplete resource observation')
                    databases = {}
                    disk_bytes = 0
                    for project in projects:
                        values = json.loads(project.sql("SELECT json_build_object('connections',count(*),'idle_in_transaction',count(*) FILTER(WHERE state='idle in transaction'),'stuck_idle_transactions',count(*) FILTER(WHERE state='idle in transaction' AND now()-state_change>interval '5 seconds'),'waiting_locks',count(*) FILTER(WHERE wait_event_type='Lock'),'webhook_queue',(SELECT json_build_object('outstanding',outstanding,'retained',retained) FROM nelcota.webhook_queue_usage WHERE singleton),'webhook_work',(SELECT json_build_object('processing',count(*) FILTER(WHERE status='processing'),'oldest_pending_seconds',coalesce(extract(epoch FROM clock_timestamp()-min(created_at)),0)) FROM nelcota.webhook_deliveries WHERE status IN ('pending','processing'))) FROM pg_stat_activity WHERE backend_type='client backend';"))
                        if values['connections'] > 32 or values['stuck_idle_transactions']:
                            raise RuntimeError('Connection or transaction ownership violation')
                        values['disk_observation']=container_disk_usage(owned,project.pg,'/var/lib/postgresql/data',unit='k')
                        values['disk_bytes']=values['disk_observation']['value']*1024
                        disk_bytes += values['disk_bytes']
                        databases[project.label] = values
                    if disk_bytes > args.disk_mib*1024*1024:
                        raise RuntimeError('Aggregate database disk budget exceeded')
                    webhook_observation = webhooks.observe()
                    ingress_observation = None
                    if ingress is not None:
                        ingress_observation = ingress.observe()
                    generation = generator_observation(active_generator, lambda: active_generator)
                    if generation and generation.get('host_pid') and generation.get('pid_starttime'):
                        atomic_json(output/'generator-process.json',{'identity':identity,'pid':generation['host_pid'],'process_identity':generation['pid_starttime'],'container_id':generation['container_id']})
                    resource_at = time.monotonic()
                    coverage.observe(resource_at-resource_began)
                    with (output/'resources.jsonl').open('a') as target:
                        target.write(json.dumps({'monotonic': resource_at, 'phase': phase, 'containers': stats, 'databases': databases, 'generator': generation,'host':host_snapshot(cpus),'memory_events':memory_events,'component_cgroups':cgroups,'webhooks':webhook_observation,'ingress':ingress_observation})+'\n')
                    atomic_json(output/'checkpoint.json', {'phase': phase, 'monotonic': resource_at, 'load_seconds_completed': load_seconds, 'resource_observations': coverage.count})
                except Exception as error:
                    violations.append({'phase': phase, 'reason': str(error)[-600:], 'type': type(error).__name__})
                    critical.set()
                    return
                stop.wait(max(0, 5-(time.monotonic()-cycle)))

        phase = 'ready'
        sampler = threading.Thread(target=sample, daemon=True)
        sampler.start()

        def run(label, rates, hot='', overload=False, seconds=None):
            nonlocal phase, active_generator, load_seconds
            if seconds is None:
                seconds = args.seconds
            if critical.is_set():
                raise RuntimeError('Fleet resource guard stopped traffic')
            phase = label
            command = ['docker', 'run', '--rm', '--name', generator, '--label', LABEL+'='+identity, '--pids-limit', '128', '--network', 'host', '--cpus', '1', '--cpuset-cpus', str(cpus[2]), '--memory', '512m', '--memory-swap', '512m', '--user', f'{os.getuid()}:{os.getgid()}', '-v', str(ROOT/'bench/campaign.js')+':/campaign.js:ro', '-e', 'FIXTURE', '-e', 'FLEET=true', '-e', 'PROJECT_RATES='+','.join(map(str,rates)), '-e', 'WORKLOAD=mixed', '-e', 'HOT_WORKLOAD='+hot, '-e', 'DURATION='+str(seconds)+'s', '-e', 'RUN_ID='+label, '-e', 'OVERLOAD='+str(bool(hot) or overload).lower(), '-e', 'GOMEMLIMIT=448MiB', '-e', 'K6_PROMETHEUS_RW_SERVER_URL='+metrics_url+'/api/v1/write', '-e', 'K6_PROMETHEUS_RW_TREND_AS_NATIVE_HISTOGRAM=true', '-e', 'K6_PROMETHEUS_RW_PUSH_INTERVAL=2s', K6, 'run', '--quiet', '--summary-mode=disabled', '--no-thresholds', '--out=experimental-prometheus-rw', '/campaign.js']
            if ingress is not None:
                position = command.index(K6)
                command[position:position] = ['-v', str(ingress.bundle)+':/verified-ca.pem:ro', '-e', 'SSL_CERT_FILE=/verified-ca.pem']
            print('Running', label, rates, hot, flush=True)
            start = time.monotonic()
            with (output/(label+'.log')).open('w') as target:
                process = subprocess.Popen(command, stdout=target, stderr=subprocess.STDOUT, env={**os.environ, 'FIXTURE':json.dumps({'projects':fixtures})})
                active_generator = {'process':process, 'name':generator, 'identity':identity, 'phase':label, 'started':start}
                try:
                    while process.poll() is None:
                        if critical.wait(.25) or watchdog.poll() is not None or time.monotonic()-start > seconds+120:
                            stop_generator(manifest)
                            raise RuntimeError('Fleet observation, watchdog or duration gate stopped traffic')
                finally:
                    elapsed = time.monotonic()-start
                    with (output/'traffic-ledger.jsonl').open('a') as target:
                        target.write(json.dumps({'phase':label, 'requested_seconds':seconds, 'elapsed_seconds':elapsed, 'completed':process.returncode==0, 'exit_code':process.returncode, 'started_monotonic':start, 'ended_monotonic':time.monotonic()})+'\n')
                    active_generator = None
                    if process.poll() is None:
                        owned('rm', '-f', generator)
                        process.wait(timeout=20)
                    atomic_json(output/(label+'-metrics-raw.json'), metrics.snapshot(label))
            if process.returncode or elapsed < seconds:
                raise RuntimeError('Fleet generator failed or ended before its measured interval')
            reports = {project.label:metrics.summary(label,seconds,project.label) for project in projects}
            atomic_json(output/(label+'.json'), reports)
            load_seconds += seconds
            for report in reports.values():
                if report['metrics']['unexpected']['value'] or report['metrics']['checks']['fails'] or report['metrics']['iterations']['count'] <= 0:
                    raise RuntimeError('Project correctness failed')
            return reports

        nominal_rate = args.rate
        capacity = []
        if args.capacity:
            qualified = 0
            for percent in [25,50,75,100]:
                offered = max(1,args.provisional_rate*percent//100)
                reports = run('capacity-'+str(percent),[offered]*args.projects,overload=True)
                failures = []
                for label,report in reports.items():
                    failures.extend({'project':label,**item} for item in nominal_violations(report,'mixed'))
                    values = report['metrics']
                    if values['overload_rejections']['count'] or values['iterations']['count']<offered*args.seconds*.99 or values['dropped_iterations']['count']>offered*args.seconds*.01:
                        failures.append({'project':label,'arrival_or_rejection_gate':False})
                capacity.append({'percent':percent,'per_project_iterations_per_second':offered,'qualified':not failures,'failures':failures})
                atomic_json(output/'capacity-probes.json',capacity)
                if not failures:
                    qualified=max(qualified,offered)
            if not qualified:
                raise RuntimeError('No host arrival rate qualified under the declared SLOs')
            nominal_rate=max(1,int(qualified*.8))
            atomic_json(output/'capacity.json',{'qualified_lower_bound_per_project':qualified,'nominal_rate_per_project':nominal_rate,'provisional_per_project':args.provisional_rate})
            for percent in [125,150,200]:
                run('overload-'+str(percent),[max(1,qualified*percent//100)]*args.projects,overload=True,seconds=30)
        balanced_samples = []
        profile['nominal_rate_per_project']=nominal_rate
        atomic_json(output/'manifest.json',profile)
        for index in range(1,4):
            reports = run('balanced-'+str(index), [nominal_rate]*args.projects)
            failures = []
            for label, report in reports.items():
                failures.extend({'project':label, **item} for item in nominal_violations(report,'mixed'))
                values = report['metrics']
                if values['overload_rejections']['count'] or values['iterations']['count'] < nominal_rate*args.seconds*.99 or values['dropped_iterations']['count'] > nominal_rate*args.seconds*.01:
                    failures.append({'project':label,'arrival_or_rejection_gate':False})
            atomic_json(output/('balanced-'+str(index)+'-gates.json'), failures)
            if failures:
                raise RuntimeError('Balanced fleet SLO or delivered load failed')
            balanced_samples.append(reports)
        baseline = median_baseline(balanced_samples)
        atomic_json(output/'balanced-median.json',baseline)
        if args.projects > 1:
            for hot in ['mixed','login','read','storage','write']:
                reports = run('noisy-'+hot, [4*(args.projects-1)*nominal_rate]+[nominal_rate]*(args.projects-1), hot)
                failures = fairness_violations(baseline,reports,nominal_rate,args.seconds)
                atomic_json(output/('noisy-'+hot+'-gates.json'), failures)
                if failures:
                    raise RuntimeError('Healthy-project fairness failed: '+hot)
        if args.webhooks:
            phase = 'webhook-drain'
            atomic_json(output/'webhook-invariants.json', webhooks.reconcile())
        for project in projects:
            for foreign in fixtures:
                if project.label != foreign['label']:
                    project.assert_foreign_denied(foreign)
            count = project.sql("SELECT count(*) FROM public.bench_products WHERE name NOT LIKE '"+project.label+":%';")
            if count != '0':
                raise RuntimeError('Project database marker integrity failed')
        stop.set()
        sampler.join(timeout=30)
        failures = coverage.violations(time.monotonic()-resource_began)
        if sampler.is_alive() or critical.is_set() or failures:
            violations.extend(failures)
            raise RuntimeError('Fleet resource evidence incomplete')
        status = 'passed'
    finally:
        if sys.exception() is not None:
            atomic_json(output/'error.json', {'type':type(sys.exception()).__name__})
        phase = 'teardown'
        stop.set()
        if sampler is not None:
            sampler.join(timeout=30)
        atomic_json(output/'resource-violations.json', violations)
        cleanup(manifest)
        webhook_cleanup = webhooks.close()
        atomic_json(output/'webhook-cleanup.json', {'errors': webhook_cleanup})
        if webhook_cleanup:
            status = 'failed'
        remaining_containers = owned('ps','-aq','--filter','label='+LABEL+'='+identity)
        remaining_volumes = owned('volume','ls','-q','--filter','label='+LABEL+'='+identity)
        shutil.rmtree(directory)
        owned_clean = not remaining_containers and not remaining_volumes and not directory.exists()
        clean = owned_clean and not webhook_cleanup
        atomic_json(output/'cleanup.json', {'confirmed':clean,'containers':remaining_containers.splitlines(),'volumes':remaining_volumes.splitlines(),'directory_removed':not directory.exists()})
        if not clean:
            status = 'failed'
        if owned_clean:
            (output/'watchdog-disarmed').touch()
        if heartbeat_thread is not None:
            heartbeat_thread.join(timeout=5)
        if watchdog is not None:
            watchdog.wait(timeout=20)
        signal.signal(signal.SIGTERM,previous)
        atomic_json(output/'completion.json',{'status':status,'projects':args.projects,'load_seconds':load_seconds,'resource_observations':coverage.count,'cleanup_confirmed':clean,'error_type':type(sys.exception()).__name__})
        if status != 'passed' and sys.exception() is None:
            raise RuntimeError('Fleet final cleanup failed')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--projects',type=int,choices=[1,5,10],required=True)
    parser.add_argument('--seconds',type=int,default=300)
    parser.add_argument('--rate',type=int,default=8)
    parser.add_argument('--capacity',action='store_true')
    parser.add_argument('--webhooks',action='store_true',help='Enable real signed webhook workers in every independent project, including the noisy write phase')
    parser.add_argument('--caddy',action='store_true',help='Use an owned real Caddy with verified local HTTPS; requires a pre-pulled caddy:2-alpine image')
    parser.add_argument('--provisional-rate',type=int,default=32)
    parser.add_argument('--disk-mib',type=int,default=2048)
    args = parser.parse_args()
    if not 30 <= args.seconds <= 300 or not 1 <= args.rate <= 64 or not 4 <= args.provisional_rate <= 64 or not 512 <= args.disk_mib <= 4096:
        parser.error('Steps must be 30..300 seconds, rate 1..64, disk budget 512..4096 MiB')
    campaign(args)


if __name__ == '__main__':
    main()
