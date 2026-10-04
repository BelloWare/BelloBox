import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).with_name('measure.py')
spec = importlib.util.spec_from_file_location('measure', SCRIPT)
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)

class MeasurementTests(unittest.TestCase):
    def test_percentiles(self):
        self.assertIsNone(mod.percentile([], .95))
        self.assertEqual(mod.percentile([4, 1, 2, 3], .5), 2.5)
        self.assertEqual(mod.percentile([7], .99), 7)

    def run_measure(self, code, timeout='2'):
        with tempfile.TemporaryDirectory() as d:
            output = Path(d) / 'out.json'
            p = subprocess.run([sys.executable, str(SCRIPT), '--trials', '1',
                                '--timeout', timeout, '--output', str(output),
                                '--', sys.executable, '-c', code], capture_output=True)
            return p.returncode, json.loads(output.read_text())

    def test_success(self):
        rc, report = self.run_measure('pass')
        self.assertEqual(rc, 0)
        self.assertTrue(report['all_passed'])
        self.assertEqual(report['elapsed_ms']['n'], 1)

    def test_failed_samples_not_counted(self):
        rc, report = self.run_measure('raise SystemExit(7)')
        self.assertEqual(rc, 1)
        self.assertEqual(report['runs'][0]['exit_code'], 7)
        self.assertEqual(report['elapsed_ms']['n'], 0)

    def test_timeout(self):
        rc, report = self.run_measure('import time; time.sleep(5)', '.05')
        self.assertEqual(rc, 1)
        self.assertTrue(report['runs'][0]['timed_out'])
        self.assertEqual(report['elapsed_ms']['n'], 0)

if __name__ == '__main__':
    unittest.main()
