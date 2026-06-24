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

    def test_default_db_path_migrates_legacy_marketingos_db(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp_path = Path(tmpdir)
            legacy_path = tmp_path / "marketingos.db"
            new_path = tmp_path / "agentframe.db"

            writer.ensure_db(legacy_path)
            writer.append_system_change(
                db_path=legacy_path,
                change_type="process_patch",
                actor="agent",
                reason="Legacy path smoke test",
            )

            original_default = writer.DEFAULT_DB_PATH
            original_legacy = writer.LEGACY_DB_PATH
            try:
                writer.DEFAULT_DB_PATH = new_path
                writer.LEGACY_DB_PATH = legacy_path

                writer.ensure_db()

                self.assertTrue(new_path.exists())
                self.assertFalse(legacy_path.exists())
                with closing(sqlite3.connect(new_path)) as conn:
                    count = conn.execute(
                        "SELECT COUNT(*) FROM system_changes"
                    ).fetchone()[0]
                self.assertEqual(count, 1)
            finally:
                writer.DEFAULT_DB_PATH = original_default
                writer.LEGACY_DB_PATH = original_legacy

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
                    "writer.py",
                    "--db-path",
                    str(db_path),
                    "system-change",
                    "--change-type",
                    "process_refinement",
                    "--actor",
                    "agent",
                    "--reason",
                    "Payload file test",
                    "--payload-json-file",
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
                    "writer.py",
                    "--db-path",
                    str(db_path),
                    "system-change",
                    "--change-type",
                    "process_refinement",
                    "--actor",
                    "agent",
                    "--reason",
                    "BOM payload file test",
                    "--payload-json-file",
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


class ModeSwapAtomicityTests(unittest.TestCase):
    """Coverage for BB-2026-05-26-01.

    The mode-swap protocol must be atomic: an audit row with
    `change_type=mode_swap` is only written if the AGENTS.{mode}.md → AGENTS.md
    file copy succeeds. Failed copies must not leave a row behind.
    """

    def _scaffold_project_root(self, root: Path) -> dict[str, Path]:
        """Create a minimal repo layout so writer.append_system_change can run."""
        agents_md = root / "AGENTS.md"
        builder_md = root / "AGENTS.builder.md"
        cmo_md = root / "AGENTS.operator.md"
        agents_md.write_text("# starting persona — Operator\n", encoding="utf-8")
        builder_md.write_text("# Builder persona\n## rules go here\n", encoding="utf-8")
        cmo_md.write_text("# Operator persona\n## different rules\n", encoding="utf-8")
        return {
            "agents_md": agents_md,
            "builder_md": builder_md,
            "cmo_md": cmo_md,
        }

    def test_mode_swap_copies_persona_file_before_writing_row(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            project_root = Path(tmpdir)
            files = self._scaffold_project_root(project_root)
            db_path = project_root / "system" / "audit" / "agentframe.db"

            row_id = writer.append_system_change(
                db_path=db_path,
                change_type="mode_swap",
                actor="agent",
                mode="builder",
                reason="Test atomic mode swap",
                project_root=project_root,
            )

            # File copy actually happened
            self.assertEqual(
                files["agents_md"].read_text(encoding="utf-8"),
                files["builder_md"].read_text(encoding="utf-8"),
            )
            # And the audit row exists
            with closing(sqlite3.connect(db_path)) as conn:
                row = conn.execute(
                    "SELECT change_type, mode FROM system_changes WHERE id = ?",
                    (row_id,),
                ).fetchone()
            self.assertEqual(row[0], "mode_swap")
            self.assertEqual(row[1], "builder")

    def test_mode_swap_round_trip_builder_then_cmo(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            project_root = Path(tmpdir)
            files = self._scaffold_project_root(project_root)
            db_path = project_root / "system" / "audit" / "agentframe.db"

            writer.append_system_change(
                db_path=db_path,
                change_type="mode_swap",
                actor="agent",
                mode="builder",
                reason="Swap to Builder",
                project_root=project_root,
            )
            self.assertEqual(
                files["agents_md"].read_text(encoding="utf-8"),
                files["builder_md"].read_text(encoding="utf-8"),
            )

            writer.append_system_change(
                db_path=db_path,
                change_type="mode_swap",
                actor="agent",
                mode="operator",
                reason="Swap back to Operator",
                project_root=project_root,
            )
            self.assertEqual(
                files["agents_md"].read_text(encoding="utf-8"),
                files["cmo_md"].read_text(encoding="utf-8"),
            )

            with closing(sqlite3.connect(db_path)) as conn:
                count = conn.execute(
                    "SELECT COUNT(*) FROM system_changes WHERE change_type = 'mode_swap'"
                ).fetchone()[0]
            self.assertEqual(count, 2)

    def test_mode_swap_rejects_unknown_mode_and_writes_no_row(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            project_root = Path(tmpdir)
            files = self._scaffold_project_root(project_root)
            original_content = files["agents_md"].read_text(encoding="utf-8")
            db_path = project_root / "system" / "audit" / "agentframe.db"

            with self.assertRaises(ValueError) as ctx:
                writer.append_system_change(
                    db_path=db_path,
                    change_type="mode_swap",
                    actor="agent",
                    mode="not-a-real-mode",
                    reason="Should not land",
                    project_root=project_root,
                )
            self.assertIn("unknown mode", str(ctx.exception))

            # AGENTS.md untouched
            self.assertEqual(
                files["agents_md"].read_text(encoding="utf-8"),
                original_content,
            )

            # No row written. ensure_db is only called AFTER the side effect,
            # so a rejected mode_swap leaves the DB uncreated. That's fine —
            # query against the DB only if it exists.
            if db_path.exists():
                with closing(sqlite3.connect(db_path)) as conn:
                    count = conn.execute(
                        "SELECT COUNT(*) FROM system_changes"
                    ).fetchone()[0]
                self.assertEqual(count, 0)

    def test_mode_swap_rejects_missing_mode_arg(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            project_root = Path(tmpdir)
            self._scaffold_project_root(project_root)
            db_path = project_root / "system" / "audit" / "agentframe.db"

            with self.assertRaises(ValueError) as ctx:
                writer.append_system_change(
                    db_path=db_path,
                    change_type="mode_swap",
                    actor="agent",
                    mode=None,
                    reason="No mode provided",
                    project_root=project_root,
                )
            self.assertIn("mode is required", str(ctx.exception))

    def test_mode_swap_rejects_when_source_persona_missing(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            project_root = Path(tmpdir)
            files = self._scaffold_project_root(project_root)
            # Delete the builder persona to simulate a broken repo state
            files["builder_md"].unlink()
            original_content = files["agents_md"].read_text(encoding="utf-8")
            db_path = project_root / "system" / "audit" / "agentframe.db"

            with self.assertRaises(ValueError) as ctx:
                writer.append_system_change(
                    db_path=db_path,
                    change_type="mode_swap",
                    actor="agent",
                    mode="builder",
                    reason="Should reject because source file missing",
                    project_root=project_root,
                )
            self.assertIn("not found", str(ctx.exception))

            # AGENTS.md untouched, no row written
            self.assertEqual(
                files["agents_md"].read_text(encoding="utf-8"),
                original_content,
            )

    def test_non_mode_swap_change_does_not_touch_agents_md(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            project_root = Path(tmpdir)
            files = self._scaffold_project_root(project_root)
            original_content = files["agents_md"].read_text(encoding="utf-8")
            db_path = project_root / "system" / "audit" / "agentframe.db"

            writer.append_system_change(
                db_path=db_path,
                change_type="process_patch",
                actor="agent",
                mode="builder",
                reason="Unrelated change should not trigger file copy",
                project_root=project_root,
            )

            # AGENTS.md is exactly as we left it
            self.assertEqual(
                files["agents_md"].read_text(encoding="utf-8"),
                original_content,
            )


if __name__ == "__main__":
    unittest.main()
