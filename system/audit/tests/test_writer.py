import sqlite3
import tempfile
import unittest
from contextlib import closing
from pathlib import Path
from unittest.mock import patch

from system.audit import writer


class AuditWriterTests(unittest.TestCase):
    def test_ensure_db_creates_only_system_changes_table(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            db_path = Path(tmpdir) / "agentframe.db"

            writer.ensure_db(db_path)

            with closing(sqlite3.connect(db_path)) as conn:
                rows = conn.execute(
                    "SELECT name FROM sqlite_master WHERE type = 'table'"
                ).fetchall()

            table_names = {row[0] for row in rows}
            self.assertEqual(table_names, {"system_changes", "sqlite_sequence"})

    def test_append_system_change_round_trips_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            db_path = Path(tmpdir) / "agentframe.db"
            writer.ensure_db(db_path)

            row_id = writer.append_system_change(
                db_path=db_path,
                change_type="process_patch",
                actor="agent",
                mode="builder",
                target_kind="process_file",
                target_path="library/process/typical-flow.md",
                reason="Operator asked for a smaller process.",
                summary="Removed stale telemetry instructions.",
                payload={"validation_pending": False},
            )

            with closing(sqlite3.connect(db_path)) as conn:
                row = conn.execute(
                    """
                    SELECT id, change_type, actor, mode, target_kind, target_path,
                           reason, summary, payload_json
                    FROM system_changes
                    WHERE id = ?
                    """,
                    (row_id,),
                ).fetchone()

            self.assertEqual(row[0], row_id)
            self.assertEqual(row[1], "process_patch")
            self.assertEqual(row[2], "agent")
            self.assertEqual(row[3], "builder")
            self.assertEqual(row[4], "process_file")
            self.assertEqual(row[5], "library/process/typical-flow.md")
            self.assertEqual(row[6], "Operator asked for a smaller process.")
            self.assertEqual(row[7], "Removed stale telemetry instructions.")
            self.assertIn('"validation_pending": false', row[8])

    def test_cli_accepts_payload_json_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp_path = Path(tmpdir)
            db_path = tmp_path / "agentframe.db"
            payload_path = tmp_path / "payload.json"
            payload_path.write_text(
                '{"validation_pending": true, "source": "file"}',
                encoding="utf-8",
            )

            with patch(
                "sys.argv",
                [
                    "writer.py", "--db-path", str(db_path), "system-change",
                    "--change-type", "process_refinement", "--actor", "agent",
                    "--reason", "Payload file test", "--payload-json-file",
                    str(payload_path),
                ],
            ):
                exit_code = writer.main()

            self.assertEqual(exit_code, 0)
            with closing(sqlite3.connect(db_path)) as conn:
                payload_json = conn.execute(
                    "SELECT payload_json FROM system_changes"
                ).fetchone()[0]

            self.assertIn('"source": "file"', payload_json)
            self.assertIn('"validation_pending": true', payload_json)

    def test_cli_accepts_payload_json_file_with_utf8_bom(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp_path = Path(tmpdir)
            db_path = tmp_path / "agentframe.db"
            payload_path = tmp_path / "payload.json"
            payload_path.write_bytes(
                b'\xef\xbb\xbf{"validation_pending": true, "source": "bom-file"}'
            )

            with patch(
                "sys.argv",
                [
                    "writer.py", "--db-path", str(db_path), "system-change",
                    "--change-type", "process_refinement", "--actor", "agent",
                    "--reason", "BOM payload file test", "--payload-json-file",
                    str(payload_path),
                ],
            ):
                exit_code = writer.main()

            self.assertEqual(exit_code, 0)
            with closing(sqlite3.connect(db_path)) as conn:
                payload_json = conn.execute(
                    "SELECT payload_json FROM system_changes"
                ).fetchone()[0]

            self.assertIn('"source": "bom-file"', payload_json)
            self.assertIn('"validation_pending": true', payload_json)


class StableRootTests(unittest.TestCase):
    def test_mode_swap_is_retired_without_touching_root_or_db(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            project_root = Path(tmpdir)
            root_agents = project_root / "AGENTS.md"
            root_agents.write_text("# Stable classifier\n", encoding="utf-8")
            db_path = project_root / "system" / "audit" / "agentframe.db"

            with self.assertRaisesRegex(ValueError, "mode_swap is retired"):
                writer.append_system_change(
                    db_path=db_path,
                    change_type="mode_swap",
                    actor="agent",
                    mode="builder",
                    reason="Legacy caller",
                    project_root=project_root,
                )

            self.assertEqual(root_agents.read_text(encoding="utf-8"), "# Stable classifier\n")
            self.assertFalse(db_path.exists())

    def test_normal_change_does_not_touch_stable_root(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            project_root = Path(tmpdir)
            root_agents = project_root / "AGENTS.md"
            root_agents.write_text("# Stable classifier\n", encoding="utf-8")
            db_path = project_root / "system" / "audit" / "agentframe.db"

            writer.append_system_change(
                db_path=db_path,
                change_type="process_patch",
                actor="agent",
                mode="builder",
                reason="Normal audited change",
                project_root=project_root,
            )

            self.assertEqual(root_agents.read_text(encoding="utf-8"), "# Stable classifier\n")


if __name__ == "__main__":
    unittest.main()
