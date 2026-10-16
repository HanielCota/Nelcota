"""Bounded native k6 histograms through a disposable Prometheus receiver.

k6's end-of-test TrendSink retains every observation. Long campaigns disable
that engine and use native histograms; correctness aborts remain in the script.
"""
import json
import math
import re
import urllib.parse
import urllib.request

PROMETHEUS = 'prom/prometheus:v3.5.0@sha256:63805ebb8d2b3920190daf1cb14a60871b16fd38bed42b857a3182bc621f4996'


class NativeMetrics:
    def __init__(self, endpoint):
        self.endpoint = endpoint

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
        if not phase or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in phase):
            raise ValueError('Invalid metrics phase')
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
        metrics = {
            'http_reqs': {'count': requests, 'rate': requests / duration},
            'iterations': {'count': iterations, 'rate': iterations / duration},
            'successful_operations': {'count': successful, 'rate': successful / duration},
            'data_received': {'bytes':received,'bytes_per_second':received/duration},
            'data_sent': {'bytes':sent,'bytes_per_second':sent/duration},
            'dropped_iterations': {'count': self.value('sum(k6_dropped_iterations_total' + dropped_selector + ')', True)},
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
        return {'metrics': metrics, 'schema': 2, 'latency_estimator': 'Prometheus native histogram', 'duration_seconds': duration}
