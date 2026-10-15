"""Stress evidence must survive endurance, invalid metrics and process failure."""
from pathlib import Path
import importlib.util
import math
import subprocess
import sys
import unittest
import json
import tempfile
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]


def campaign_module():
    spec = importlib.util.spec_from_file_location('stress_campaign_tests', ROOT / 'bench/campaign.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class ExistingDefects(unittest.TestCase):
    def test_endurance_retains_full_resource_coverage(self):
        module = campaign_module()
        coverage = module.ResourceCoverage()
        for elapsed in range(1, 14401, 5):
            coverage.observe(elapsed)
        self.assertEqual(coverage.violations(14405), [])
        self.assertEqual(coverage.count, 2880)

    def test_nonfinite_observation_cannot_certify(self):
        module = campaign_module()
        self.assertTrue(module.resource_evidence_violations([{'elapsed': 1}, {'elapsed': math.nan}], 15))

    def test_secrets_travel_in_environment_not_argv(self):
        module = campaign_module()
        result = subprocess.CompletedProcess([], 0, 'owned-id\n', '')
        with mock.patch.object(module.subprocess, 'run', return_value=result) as run:
            module.docker('run', '-e', 'POSTGRES_PASSWORD', env={'POSTGRES_PASSWORD': 'fixture-only-secret'})
        arguments, keywords = run.call_args
        self.assertNotIn('fixture-only-secret', ' '.join(arguments[0]))
        self.assertEqual(keywords['env']['POSTGRES_PASSWORD'], 'fixture-only-secret')

    def test_duration_is_bounded_and_supports_four_hours(self):
        module = campaign_module()
        self.assertEqual(module.generator_deadline('14400s'), 14520)
        with self.assertRaises(ValueError):
            module.generator_deadline('86401s')

    def test_regressing_resource_clock_fails(self):
        module = campaign_module()
        self.assertTrue(module.resource_evidence_violations([{'elapsed': 3}, {'elapsed': 2}], 5))

    def test_tail_without_observations_fails(self):
        module = campaign_module()
        self.assertTrue(module.resource_evidence_violations([{'elapsed': 1}, {'elapsed': 6}], 45))

    def test_missing_and_nonfinite_nominal_slo_cannot_pass(self):
        module = campaign_module()
        self.assertTrue(module.nominal_violations({'metrics': {}}, 'rls'))
        self.assertTrue(module.nominal_violations({'metrics': {'latency_rls': {'p(95)': math.nan, 'p(99)': 1}}}, 'rls'))
        self.assertTrue(module.nominal_violations({'metrics': {'latency_rls': {'p(95)': 201, 'p(99)': 500}}}, 'rls'))

    def test_disk_campaign_rejects_memory_filesystems(self):
        module = campaign_module()
        for filesystem in ['tmpfs', 'ramfs']:
            with self.assertRaises(RuntimeError):
                module.validate_storage_filesystem('disk', filesystem)
        module.validate_storage_filesystem('disk', 'overlayfs')

    def test_listener_from_another_process_does_not_authorize_target(self):
        module = campaign_module()
        owned = 'lrwx------ 1 user user 64 socket -> socket:[111]\n'
        foreign = '0: 0100007F:1F90 00000000:0000 0A 0:0 0:0 0 1000 0 222\n'
        with mock.patch.object(module, 'docker', return_value=owned+foreign):
            self.assertFalse(module.owns_listener('fixture', 8080))
        local = foreign.replace('222', '111')
        with mock.patch.object(module, 'docker', return_value=owned+local):
            self.assertTrue(module.owns_listener('fixture', 8080))

    def test_resource_diagnostic_preserves_failure_without_secrets(self):
        module = campaign_module()
        text = module.redacted_diagnostic(RuntimeError('Docker refused fixture-secret-123 during stats'), ['fixture-secret-123'])
        self.assertIn('during stats', text)
        self.assertNotIn('fixture-secret-123', text)


class EvidenceOracles(unittest.TestCase):
    def setUp(self):
        campaign_module()
        from receiver import EventLedger
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.ledger = EventLedger(Path(self.directory.name) / 'events.sqlite')
        self.addCleanup(self.ledger.close)

    def test_duplicate_delivery_preserves_event_identity(self):
        event = {'event_id': 'event-1', 'raw_hash': 'digest'}
        self.ledger.append(event)
        self.ledger.append(event)
        report = self.ledger.validate(['event-1'])
        self.assertEqual(report, {'unique_events': 1, 'deliveries': 2, 'duplicates': 1, 'missing': 0, 'extra': 0})

    def test_event_identity_cannot_change_bytes(self):
        self.ledger.append({'event_id': 'event-1', 'raw_hash': 'digest'})
        with self.assertRaises(ValueError):
            self.ledger.append({'event_id': 'event-1', 'raw_hash': 'changed'})

    def test_different_sets_with_same_counts_do_not_pass(self):
        self.ledger.append({'event_id': 'wrong-event', 'raw_hash': 'digest'})
        report = self.ledger.validate(['expected-event'])
        self.assertEqual(report['missing'], 1)
        self.assertEqual(report['extra'], 1)

    def test_memory_window_does_not_lose_persisted_event_evidence(self):
        for i in range(3000):
            self.ledger.append({'event_id': str(i), 'raw_hash': 'digest'})
        self.assertEqual(len(self.ledger), 2048)
        report = self.ledger.validate(str(i) for i in range(3000))
        self.assertEqual(report['unique_events'], 3000)
        self.assertEqual((report['missing'], report['extra']), (0, 0))

    def test_approved_retention_preserves_expected_committed_identities(self):
        self.ledger.append({'event_id': 'pruned-event', 'raw_hash': 'digest'})
        self.ledger.preserve_expected(['pruned-event'])
        report = self.ledger.validate([])
        self.assertEqual((report['missing'], report['extra']), (0, 0))


class MetricsAndOwnership(unittest.TestCase):
    def generator_context(self):
        process = mock.Mock()
        process.poll.return_value = None
        return {'process': process, 'name': 'generator', 'container_id': 'old-container-id', 'identity': 'this-run', 'phase': 'fixture', 'started': 0}

    def test_completed_generator_eof_is_a_lifecycle_boundary(self):
        campaign_module()
        from observations import generator_observation
        context = self.generator_context()
        results = [subprocess.CompletedProcess([], 1, '', 'EOF'), subprocess.CompletedProcess([], 0, json.dumps([{'State':{'Running':False}}]), '')]
        with mock.patch('observations.subprocess.run', side_effect=results) as run:
            self.assertIsNone(generator_observation(context, lambda: context))
        self.assertEqual(run.call_args_list[0].args[0][-1], 'old-container-id')

    def test_live_generator_eof_still_invalidates_evidence(self):
        campaign_module()
        from observations import generator_observation
        context = self.generator_context()
        results = [subprocess.CompletedProcess([], 1, '', 'EOF'), subprocess.CompletedProcess([], 0, json.dumps([{'State':{'Running':True}}]), '')]
        with mock.patch('observations.subprocess.run', side_effect=results):
            with self.assertRaisesRegex(RuntimeError, 'Live generator'):
                generator_observation(context, lambda: context)

    def test_reused_name_does_not_reassign_an_old_stats_stream(self):
        campaign_module()
        from observations import generator_observation
        context = self.generator_context()
        active = [context]
        def end_generation(*args, **kwargs):
            active[0] = self.generator_context()
            return subprocess.CompletedProcess([], 1, '', 'EOF')
        with mock.patch('observations.subprocess.run', side_effect=end_generation):
            self.assertIsNone(generator_observation(context, lambda: active[0]))

    def test_nonfinite_remote_metrics_fail(self):
        campaign_module()
        from metrics import NativeMetrics
        response = mock.MagicMock()
        response.__enter__.return_value = response
        for value in ['NaN', '+Inf', '-1']:
            response.read.return_value = json.dumps({'status': 'success', 'data': {'result': [{'value': [1, value]}]}}).encode()
            with mock.patch('metrics.urllib.request.urlopen', return_value=response):
                with self.assertRaises(ValueError):
                    NativeMetrics('http://fixture.invalid').value('sum(metric)')

    def test_name_similarity_does_not_grant_cleanup_ownership(self):
        campaign_module()
        from watchdog import owned_container
        result = subprocess.CompletedProcess([], 0, json.dumps({'io.nelcota.stress.run': 'other-run'}), '')
        with mock.patch('watchdog.subprocess.run', return_value=result):
            self.assertFalse(owned_container('same-looking-prefix', 'this-run'))


if __name__ == '__main__':
    unittest.main()
