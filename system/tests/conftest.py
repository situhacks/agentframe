"""Keep the suite off the live run log.

af.main() appends every run to system/logs/af-runs.jsonl. Several tests drive main() with a
real argv, so without this every test run would write its own lines into the live log and the
doctor buttons note would count fixtures as button history.
"""
import os
import tempfile

import pytest

from system import af


@pytest.fixture(autouse=True, scope="session")
def _af_runs_in_a_temp_log():
    with tempfile.TemporaryDirectory() as tmp:
        saved = af.RUN_LOG
        af.RUN_LOG = os.path.join(tmp, "logs", "af-runs.jsonl")
        try:
            yield
        finally:
            af.RUN_LOG = saved
