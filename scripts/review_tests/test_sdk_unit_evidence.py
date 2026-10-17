"""Private SDK test payloads cannot appear in the public fixed projection."""
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'bench'))
from sdk_unit_evidence import TITLE, project


class SdkUnitProjection(unittest.TestCase):
    def value(self):
        return {'numTotalTests': 2, 'numPassedTests': 0, 'numFailedTests': 2, 'numPendingTests': 0,
                'private': 'private-token', 'testResults': [{'assertionResults': [
                    {'fullName': TITLE + state + ')', 'status': 'failed', 'failureMessages': ['private-token']}
                    for state in ['false', 'true']]}]}

    def test_failed_private_results_preserve_failure_without_payloads(self):
        report = project(self.value(), 'a' * 40)
        self.assertFalse(report['successful'])
        self.assertEqual(report['failed'], 2)
        self.assertNotIn('private-token', str(report))
        self.assertEqual(len(report['independent_server_clients']), 2)

    def test_missing_cases_and_nonfinite_boolean_or_inconsistent_counts_are_rejected(self):
        for field, number in [('numFailedTests', True), ('numTotalTests', float('nan')), ('numPassedTests', 2)]:
            value = self.value();value[field] = number
            with self.subTest(field=field), self.assertRaises(ValueError):
                project(value, 'a' * 40)
        value = self.value();value['testResults'][0]['assertionResults'].pop()
        with self.assertRaises(ValueError):
            project(value, 'a' * 40)


if __name__ == '__main__':
    unittest.main()
