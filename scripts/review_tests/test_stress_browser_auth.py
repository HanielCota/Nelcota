"""Incomplete/raw auth telemetry cannot certify a browser stress campaign."""
import copy
from pathlib import Path
import sys
import unittest

sys.path.insert(0,str(Path(__file__).resolve().parents[2]/'bench'))
from browser_auth import COUNTS, SUCCESS, complete, cycle


def proof(index=0):
    return {'cycle':index,'status':'passed','requests_attempted':112,'requests_completed':112,
            'independent_browser_request_count':112,'maximum_inflight':8,'maximum_arrivals_per_second':16,
            'accounts_created':8,'mutation_retries':0,'response_bytes_consumed':12000,'elapsed_seconds':7.1,
            'counts':dict(COUNTS),'successful_latency':{name:{'samples':count,'p50_ms':10,'p95_ms':20,'p99_ms':30}
                                                     for name,count in SUCCESS.items()}}


class RawBrowserAuthEvidence(unittest.TestCase):
    def test_complete_five_cycles_preserve_positive_negative_denominators(self):
        values=complete([proof(index) for index in range(5)])
        self.assertEqual(sum(row['requests_completed'] for row in values),560)
        self.assertEqual(sum(row['counts']['signup'] for row in values),40)
        self.assertEqual(sum(row['successful_latency']['refresh']['samples'] for row in values),160)

    def test_missing_duplicate_or_out_of_order_cycle_is_rejected(self):
        for values in [[proof()], [proof(0)]*5, [proof(index) for index in reversed(range(5))]]:
            with self.subTest(cycles=len(values)),self.assertRaises(ValueError):
                complete(values)

    def test_live_wire_count_or_expected_rejection_disagreement_is_rejected(self):
        for key in ['requests_attempted','requests_completed','independent_browser_request_count']:
            value=proof();value[key]=111
            with self.subTest(key=key),self.assertRaises(ValueError):cycle(value,0)
        value=proof();value['counts']['family_revoked']=7
        with self.assertRaises(ValueError):cycle(value,0)

    def test_nonfinite_stale_overbudget_or_boolean_metrics_are_rejected(self):
        for key,bad in [('maximum_inflight',9),('maximum_arrivals_per_second',17),('mutation_retries',1),
                        ('elapsed_seconds',float('nan')),('elapsed_seconds',40),('response_bytes_consumed',True),
                        ('response_bytes_consumed',112*65536+1)]:
            value=proof();value[key]=bad
            with self.subTest(key=key,bad=bad),self.assertRaises(ValueError):cycle(value,0)

    def test_missing_zero_nonfinite_or_rejected_latency_samples_are_rejected(self):
        for transform in [lambda row:row.update(samples=9),lambda row:row.update(p50_ms=0),
                          lambda row:row.update(p99_ms=float('inf')),lambda row:row.update(p95_ms=201,p99_ms=501)]:
            value=proof();transform(value['successful_latency']['refresh'])
            with self.assertRaises(ValueError):cycle(value,0)
        value=proof();value['successful_latency']['invalid_password']={'samples':8,'p50_ms':1,'p95_ms':1,'p99_ms':1}
        with self.assertRaises(ValueError):cycle(value,0)

    def test_secret_bearing_extra_fields_are_not_published(self):
        value=proof();value['access_token']='private-fixture';value['scope']='private fixture content'
        projected=cycle(value,0)
        self.assertNotIn('access_token',projected)
        self.assertNotIn('scope',projected)
        self.assertEqual(projected['counts'],copy.deepcopy(COUNTS))


if __name__=='__main__':unittest.main()
