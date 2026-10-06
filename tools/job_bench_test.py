from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))

import job_bench


class MeasureTest(unittest.TestCase):
    def test_a_process_reports_its_time_memory_and_status(self):
        # A child that holds about 50 MB and spends a little CPU before it fails.
        code = "b = bytearray(50 * 2**20); sum(range(10**6)); raise SystemExit(3)"
        sample = job_bench.measure([sys.executable, "-c", code])
        self.assertEqual(sample["status"], 3)
        self.assertGreater(sample["peak_rss_mb"], 45)
        self.assertGreater(sample["cpu_s"], 0)
        self.assertGreaterEqual(sample["wall_s"], sample["cpu_s"] * 0.5)

    def test_memory_includes_the_processes_a_job_waits_for(self):
        child = "b = bytearray(80 * 2**20)"
        parent = f"import subprocess, sys; subprocess.run([sys.executable, '-c', {child!r}])"
        self.assertGreater(job_bench.measure([sys.executable, "-c", parent])["peak_rss_mb"], 75)

    def test_summaries_are_medians(self):
        samples = [{"wall_s": 1.0, "peak_rss_mb": 10.0}, {"wall_s": 3.0, "peak_rss_mb": 30.0},
                   {"wall_s": 2.0, "peak_rss_mb": 20.0}]
        self.assertEqual(job_bench.summarize(samples), {"wall_s": 2.0, "peak_rss_mb": 20.0})


if __name__ == "__main__":
    unittest.main()
