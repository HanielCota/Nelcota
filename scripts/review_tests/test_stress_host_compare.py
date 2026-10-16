"""Independent topology comparisons cannot certify incomplete or regressed work."""
from pathlib import Path
import json
import math
import sys
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'bench'))
import host_compare
from arrival_counters import PROTOCOL as ARRIVAL_COUNTER_PROTOCOL


class HostComparison(unittest.TestCase):
    def setUp(self):
        self.directory=tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.before=Path(self.directory.name)/'before'
        self.after=Path(self.directory.name)/'after'
        for root in [self.before,self.after]:
            root.mkdir()
            profile={field:1 for field in host_compare.FIELDS}
            profile.update(schema=2,projects=1,seconds_per_step=300,nominal_rate_per_project=8,certifies_five_minute_steps=True,project_cpu_period_micros=100000,resource_probe_workers=4)
            profile['arrival_counter_protocol']=ARRIVAL_COUNTER_PROTOCOL
            (root/'manifest.json').write_text(json.dumps(profile))
            (root/'completion.json').write_text(json.dumps({'status':'passed','cleanup_confirmed':True}))
            (root/'cleanup.json').write_text(json.dumps({'confirmed':True}))
            (root/'resource-violations.json').write_text('[]')
            (root/'acceptance-failures.json').write_text('[]')
            intervals=[]
            for index in range(1,4):
                metrics={'unexpected':{'value':0},'checks':{'fails':0},'overload_rejections':{'count':0},'iterations':{'count':2400},'dropped_iterations':{'count':0}}
                for operation in host_compare.OPERATIONS:
                    metrics['latency_'+operation]={'p(95)':10,'p(99)':20}
                    metrics['throughput_'+operation]={'rate':4}
                counters={'phase':'balanced-'+str(index),'protocol':ARRIVAL_COUNTER_PROTOCOL,'began_monotonic':0,'last_monotonic':300,'maximum_poll_gap_seconds':5,'observations':62,'closed':True,'dropped_by_scenario':{'project-0':0}}
                (root/('balanced-'+str(index)+'.json')).write_text(json.dumps({'project-0':{'schema':2,'metrics':metrics,'duration_seconds':300,'arrival_counter_evidence':counters}}))
                (root/('balanced-'+str(index)+'-gates.json')).write_text('[]')
                intervals.append({'phase':'balanced-'+str(index),'completed':True,'requested_seconds':300,'elapsed_seconds':301})
            (root/'traffic-ledger.jsonl').write_text(''.join(json.dumps(item)+'\n' for item in intervals))

    def test_equivalent_three_samples_pass(self):
        self.assertTrue(host_compare.compare(self.before,self.after)['gate_passed'])

    def test_missing_arrival_coverage_cannot_hide_behind_a_zero_report(self):
        path=self.after/'balanced-1.json'
        sample=json.loads(path.read_text());sample['project-0'].pop('arrival_counter_evidence');path.write_text(json.dumps(sample))
        with self.assertRaises(ValueError):host_compare.compare(self.before,self.after)

    def test_arrival_polling_gap_cannot_certify_comparable_host_samples(self):
        path=self.after/'balanced-1.json'
        sample=json.loads(path.read_text());sample['project-0']['arrival_counter_evidence']['maximum_poll_gap_seconds']=31;path.write_text(json.dumps(sample))
        with self.assertRaises(ValueError):host_compare.compare(self.before,self.after)

    def test_failed_interference_cannot_hide_behind_a_passed_completion(self):
        (self.after/'acceptance-failures.json').write_text(json.dumps([{'phase':'noisy-login','failures':[{'p95':'more than 20% worse'}]}]))
        with self.assertRaises(ValueError):host_compare.compare(self.before,self.after)

    def test_webhook_workers_cannot_be_silently_absent_on_one_side(self):
        path=self.after/'manifest.json'
        value=json.loads(path.read_text())
        value['webhooks']={'enabled':False}
        path.write_text(json.dumps(value))
        result=host_compare.compare(self.before,self.after)
        self.assertFalse(result['gate_passed'])
        self.assertIn('webhooks',result['fixture_differences'])

    def test_different_cpu_bandwidth_periods_cannot_certify_equivalent_latency(self):
        path=self.after/'manifest.json'
        value=json.loads(path.read_text())
        value['project_cpu_period_micros']=10000
        path.write_text(json.dumps(value))
        result=host_compare.compare(self.before,self.after)
        self.assertFalse(result['gate_passed'])
        self.assertIn('project_cpu_period_micros',result['fixture_differences'])

    def test_unknown_cpu_bandwidth_policy_cannot_certify(self):
        path=self.after/'manifest.json'
        original=json.loads(path.read_text())
        for period in [None,True,10000.0,0,math.nan,1000]:
            value={**original,'project_cpu_period_micros':period}
            path.write_text(json.dumps(value))
            with self.subTest(period=period),self.assertRaises(ValueError):
                host_compare.compare(self.before,self.after)

    def test_serial_observer_cannot_certify_the_parallel_observer_profile(self):
        path=self.after/'manifest.json'
        value=json.loads(path.read_text())
        value['resource_protocol']='another observation protocol'
        path.write_text(json.dumps(value))
        result=host_compare.compare(self.before,self.after)
        self.assertFalse(result['gate_passed'])
        self.assertIn('resource_protocol',result['fixture_differences'])

    def test_one_operation_regression_cannot_hide_in_total_throughput(self):
        for index in range(1,4):
            path=self.after/('balanced-'+str(index)+'.json')
            value=json.loads(path.read_text())
            value['project-0']['metrics']['throughput_write']['rate']=3
            path.write_text(json.dumps(value))
        self.assertFalse(host_compare.compare(self.before,self.after)['gate_passed'])

    def test_direct_http_cannot_certify_a_verified_proxy_profile(self):
        path=self.after/'manifest.json'
        value=json.loads(path.read_text())
        value['ingress']={'kind':'real Caddy with verified internal CA on loopback'}
        path.write_text(json.dumps(value))
        result=host_compare.compare(self.before,self.after)
        self.assertFalse(result['gate_passed'])
        self.assertIn('ingress',result['fixture_differences'])

    def test_failed_or_missing_interval_cannot_certify(self):
        (self.after/'traffic-ledger.jsonl').write_text('')
        with self.assertRaises(ValueError):
            host_compare.compare(self.before,self.after)
        (self.after/'completion.json').write_text(json.dumps({'status':'failed','cleanup_confirmed':True}))
        with self.assertRaises(ValueError):
            host_compare.compare(self.before,self.after)

    def test_nonfinite_success_rate_cannot_certify(self):
        path=self.after/'balanced-1.json'
        value=json.loads(path.read_text())
        value['project-0']['metrics']['throughput_read']['rate']=math.nan
        path.write_text(json.dumps(value))
        with self.assertRaises(ValueError):
            host_compare.compare(self.before,self.after)

    def test_nonfinite_counts_and_durations_cannot_hide_underdelivery(self):
        path=self.after/'balanced-1.json'
        original=path.read_text()
        for count in [math.nan,math.inf,-1,True]:
            value=json.loads(original)
            value['project-0']['metrics']['dropped_iterations']['count']=count
            path.write_text(json.dumps(value))
            with self.assertRaises(ValueError):host_compare.compare(self.before,self.after)
        path.write_text(original)
        ledger=self.after/'traffic-ledger.jsonl'
        records=[json.loads(line) for line in ledger.read_text().splitlines()]
        records[0]['elapsed_seconds']=math.nan
        ledger.write_text(''.join(json.dumps(item)+'\n' for item in records))
        with self.assertRaises(ValueError):host_compare.compare(self.before,self.after)

    def test_matching_bad_p99_cannot_certify_from_an_empty_gate_file(self):
        for root in [self.before,self.after]:
            path=root/'balanced-1.json'
            value=json.loads(path.read_text())
            value['project-0']['metrics']['latency_read']['p(99)']=501
            path.write_text(json.dumps(value))
        with self.assertRaises(ValueError):host_compare.compare(self.before,self.after)
