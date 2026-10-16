"""Bounded native k6 histograms through a disposable Prometheus receiver.

k6's end-of-test TrendSink retains every observation. Long campaigns disable
that engine and use native histograms; correctness aborts remain in the script.
"""
import json
import math
import re
import threading
import time
import urllib.parse
import urllib.request
from arrival_counters import ArrivalCounter, phase_label, finite, closed_count

PROMETHEUS = 'prom/prometheus:v3.5.0@sha256:63805ebb8d2b3920190daf1cb14a60871b16fd38bed42b857a3182bc621f4996'


class NativeMetrics:
    def __init__(self, endpoint):
        self.endpoint = endpoint
        self.arrivals = {}
        self.arrival_lock = threading.RLock()

    def begin_phase(self, phase, scenarios):
        with self.arrival_lock:
            phase_label(phase)
            if phase in self.arrivals or len(self.arrivals) >= 128:
                raise ValueError('Duplicate phase or arrival-counter state budget exceeded')
            self.arrivals[phase] = ArrivalCounter(phase, scenarios, time.monotonic())
            return self.observe_arrivals(phase)

    def observe_arrivals(self, phase):
        with self.arrival_lock:
            tracker = self.arrivals.get(phase)
            if tracker is None:
                return None
            if tracker.closed:
                return tracker.snapshot()
            selector = 'k6_dropped_iterations_total{run_id="' + phase_label(phase) + '"}[120s]'
            expression = 'sum by(scenario) (last_over_time(' + selector + '))'
            url = self.endpoint + '/api/v1/query?' + urllib.parse.urlencode({'query': expression})
            with urllib.request.urlopen(url, timeout=10) as response:
                body = response.read(65537)
            if len(body) > 65536:
                raise ValueError('Arrival-counter response exceeded its byte budget')
            data = json.loads(body)
            if data.get('status') != 'success' or data.get('data', {}).get('resultType') != 'vector':
                raise ValueError('Invalid scheduler counter response')
            rows = data['data'].get('result')
            if not isinstance(rows, list) or len(rows) > 11:
                raise ValueError('Invalid scheduler counter cardinality')
            values = {}
            for row in rows:
                if not isinstance(row, dict) or not isinstance(row.get('metric'), dict) or not isinstance(row.get('value'), list) or len(row['value']) != 2 or not isinstance(row['value'][1], str):
                    raise ValueError('Malformed scheduler counter sample')
                scenario = row['metric']['scenario']
                if not isinstance(scenario, str) or scenario in values:
                    raise ValueError('Ambiguous scheduler counter scenario')
                values[scenario] = float(row['value'][1])
            resets = self.value('sum(resets(' + selector + '))', optional=True)
            return tracker.observe(values, resets, time.monotonic())

    def finish_phase(self, phase):
        with self.arrival_lock:
            if phase not in self.arrivals:
                raise ValueError('Arrival-counter phase was not observed from its beginning')
            self.observe_arrivals(phase)
            return self.arrivals[phase].close()

    def value(self, expression, optional=False):
        url = self.endpoint + '/api/v1/query?' + urllib.parse.urlencode({'query': expression})
        with urllib.request.urlopen(url, timeout=10) as response:
            data = json.load(response)
        if data.get('status') != 'success':
            raise ValueError('Metrics query failed')
        rows = data['data']['result']
        if not rows and optional:
            return 0.0
        if len(rows) != 1:
            raise ValueError('Missing or ambiguous metric evidence')
        value = float(rows[0]['value'][1])
        if not math.isfinite(value) or value < 0:
            raise ValueError('Non-finite or negative metric evidence')
        return value

    def sparse_counter(self, name, phase, scenario, lookback_seconds=3600):
        """Read the last actual counter sample, including between sparse events.

        k6 emits a counter only when it receives samples. An instant query can
        lose an otherwise continuous client's refresh counter after Prometheus's
        five-minute lookback. A missing range, non-finite value or observed reset
        still fails; a zero is never invented for absent evidence.
        """
        if not all(isinstance(value, str) and re.fullmatch(r'[a-z][a-z0-9_-]{0,79}', value) for value in [name, phase, scenario]):
            raise ValueError('Invalid sparse counter selector')
        if isinstance(lookback_seconds, bool) or not isinstance(lookback_seconds, int) or not 60 <= lookback_seconds <= 3600:
            raise ValueError('Sparse counter lookback must fit retained telemetry')
        metric = 'k6_' + name + '_total{run_id="' + phase + '",scenario="' + scenario + '"}'
        interval = '[' + str(lookback_seconds) + 's]'
        if self.value('sum(resets(' + metric + interval + '))') != 0:
            raise ValueError('Sparse counter reset invalidates its continuous evidence')
        return self.value('sum(last_over_time(' + metric + interval + '))')

    def snapshot(self, phase):
        if not phase or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in phase):
            raise ValueError('Invalid metrics phase')
        url = self.endpoint + '/api/v1/query?' + urllib.parse.urlencode({'query': '{run_id="' + phase + '"}'})
        with urllib.request.urlopen(url, timeout=10) as response:
            body = response.read(4 * 1024 * 1024 + 1)
        if len(body) > 4 * 1024 * 1024:
            raise ValueError('Metrics snapshot exceeded its evidence budget')
        return json.loads(body)

    def summary(self, phase, duration, project=None):
        # Phases come from fixed runner labels, never from credentials or URLs.
        phase_label(phase)
        if not finite(duration) or not 0 < duration <= 21600 or int(duration) != duration:
            raise ValueError('Invalid bounded metrics duration')
        selector = '{run_id="' + phase + '",scenario="traffic"}'
        if project is not None:
            if not isinstance(project, str) or not re.fullmatch(r'project-[0-9]', project):
                raise ValueError('Invalid project metrics selector')
            selector = '{run_id="' + phase + '",project="' + project + '"}'
        # Dropped arrivals have no executing VU, so k6 cannot apply the
        # project's dynamic VU tag. Its scheduler still supplies the scenario.
        dropped_selector = selector
        if project is not None:
            dropped_selector = '{run_id="' + phase + '",scenario="' + project + '"}'

        def count(name, optional=False):
            return self.value('sum(k6_' + name + '_total' + selector + ')', optional)

        def latency(name):
            metric = 'k6_' + name + '_seconds' + selector
            return {key: self.value('histogram_quantile(' + str(p) + ',sum(' + metric + '))') * 1000 for key, p in [('p(50)', .50), ('p(95)', .95), ('p(99)', .99), ('min', 0), ('max', 1)]}

        requests = count('http_reqs')
        iterations = count('iterations')
        successful = count('successful_operations')
        received, sent = count('data_received'), count('data_sent')
        dropped = None
        with self.arrival_lock:
            tracker = self.arrivals.get(phase)
            if tracker is not None:
                scenario = 'traffic'
                if project is not None:
                    scenario = project
                if not tracker.closed or scenario not in tracker.counts:
                    raise ValueError('Incomplete continuous scheduler-counter evidence')
                dropped = closed_count(tracker.snapshot(), phase, scenario, duration)
        if dropped is None:
            if duration > 300:
                raise ValueError('Long phases require continuous scheduler-counter evidence')
            dropped = self.value('sum(k6_dropped_iterations_total' + dropped_selector + ')', True)
        metrics = {
            'http_reqs': {'count': requests, 'rate': requests / duration},
            'iterations': {'count': iterations, 'rate': iterations / duration},
            'successful_operations': {'count': successful, 'rate': successful / duration},
            'data_received': {'bytes':received,'bytes_per_second':received/duration},
            'data_sent': {'bytes':sent,'bytes_per_second':sent/duration},
            'dropped_iterations': {'count': dropped},
            'unexpected': {'value': self.value('max(k6_unexpected_rate' + selector + ')')},
            'checks': {'passes': count('assertions_passed'), 'fails': count('assertions_failed')},
            'expected_rejections': {'count': count('expected_rejections', True)},
            'overload_rejections': {'count': count('overload_rejections')},
            'http_req_duration': latency('successful_duration'),
        }
        for name in ['read', 'rls', 'panel', 'storage', 'login', 'refresh', 'write']:
            expression = 'sum(histogram_count(k6_latency_' + name + '_seconds' + selector + '))'
            if self.value(expression, True) > 0:
                metrics['latency_' + name] = latency('latency_' + name)
                operation_selector=selector[:-1]+',workload="'+name+'"}'
                operations=self.value('sum(k6_successful_operations_total'+operation_selector+')')
                metrics['throughput_'+name]={'count':operations,'rate':operations/duration}
        report = {'metrics': metrics, 'schema': 2, 'latency_estimator': 'Prometheus native histogram', 'duration_seconds': duration}
        with self.arrival_lock:
            if phase in self.arrivals:
                report['arrival_counter_evidence'] = self.arrivals[phase].snapshot()
        return report
