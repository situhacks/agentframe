"""Keep the suite off the live audit database.

af.main() logs every run to af_runs. Several tests drive main() with a real argv, so without
this every test run would write its own rows into system/audit/agentframe.db and the doctor
buttons note would count fixtures as button history.
"""
import os
import tempfile

import pytest

from system import af


@pytest.fixture(autouse=True, scope="session")
def _af_runs_in_a_temp_db():
    with tempfile.TemporaryDirectory() as tmp:
        saved = af.AUDIT_DB
        af.AUDIT_DB = os.path.join(tmp, "af-runs-test.db")
        try:
            yield
        finally:
            af.AUDIT_DB = saved
