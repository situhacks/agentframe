import contextlib
import io
import json
import tempfile
import types
import unittest
from pathlib import Path
from unittest.mock import patch

from system import af
from system.hooks import ppt_master_guard
from system.tools import ppt_master_contract as contract


PROJECT_MD = """---
name: Deck Test
slug: deck-test
schema_version: 2026-07-19-v2
created_at: 2026-08-03
domain: marketing
status: active
current_phase: active
flow: open-flow
last_activity: 2026-08-03T10:00:00+00:00
deliverables:
---

# Deck Test
"""


def current_result():
    return {
        "canvas": "ppt169",
        "page_count": "10-12",
        "audience": "Executive team",
        "content_divergence": "",
        "mode": "pyramid",
        "visual_style": "swiss-minimal",
        "color": {
            "name": "Cobalt",
            "palette": {
                "background": "#FFFFFF",
                "secondary_bg": "#F2F5F9",
                "primary": "#164B8B",
                "accent": "#FFB000",
                "secondary_accent": "#57A0D3",
                "body_text": "#17202A",
            },
        },
        "icons": "tabler-outline",
        "typography": {
            "name": "Inter",
            "heading": {"cjk": "Noto Sans CJK", "latin": "Inter", "css": "Inter, sans-serif"},
            "body": {"cjk": "Noto Sans CJK", "latin": "Inter", "css": "Inter, sans-serif"},
            "body_size": 24,
            "body_size_unit": "px",
            "sizes": {"title": 42, "subtitle": 32, "annotation": 18},
        },
        "delivery_purpose": "balanced",
        "formula_policy": "mixed",
        "image_usage": ["provided"],
        "image_notes": "",
        "generation_mode": "continuous",
        "refine_spec": False,
        "stage": "final",
        "status": "confirmed",
        "confirmed_at": "2026-08-03T10:30:00",
    }


class PptMasterContractTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.projects = self.root / "workspace" / "projects"
        self.project = self.projects / "deck-test"
        self.ppt = self.project / "decks" / "board-deck"
        (self.project / "knowledge").mkdir(parents=True)
        (self.ppt / "sources").mkdir(parents=True)
        (self.project / "project.md").write_text(PROJECT_MD, encoding="utf-8")
        self.source = self.ppt / "sources" / "brief.md"
        self.source.write_text("Approved deck brief\n", encoding="utf-8")
        self.draft = self.ppt / "agentframe-confirmation.draft.json"
        self.patches = [
            patch.object(af, "ROOT", str(self.root)),
            patch.object(af, "PROJECTS", str(self.projects)),
            patch.object(contract, "ROOT", self.root),
            patch.object(ppt_master_guard, "ROOT", self.root),
        ]
        for item in self.patches:
            item.start()
            self.addCleanup(item.stop)
        self.addCleanup(self.tmp.cleanup)

    def quiet(self, fn, args):
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            return fn(args)

    def rel(self, path):
        return path.relative_to(self.root).as_posix()

    def prepare_run(self, allowed=None):
        self.quiet(
            af.cmd_autonomy_init,
            types.SimpleNamespace(project="deck-test", run_id="deck-run", level="assisted"),
        )
        run = self.project / "knowledge" / "autonomy" / "deck-run.md"
        wrapper = self.ppt.parent / f"{self.ppt.name}.deck-run{contract.SUFFIX}"
        fm, body = af.split_fm(run.read_text(encoding="utf-8"), str(run))
        values = {
            "goal": af.yaml_quote("Build the board deck"),
            "done_when": af.yaml_quote("Rendered deck passes review"),
            "context_sources": f"[{self.rel(self.source)}, {self.rel(wrapper)}]",
            "frozen_context": f"[{self.rel(self.source)}, {self.rel(wrapper)}]",
            "allowed_paths": f"[{allowed or self.rel(self.ppt)}]",
            "verification": "[render, visual-review]",
        }
        for key, value in values.items():
            fm = af.set_scalar(fm, key, value, str(run))
        run.write_text(af.join_fm(fm, body), encoding="utf-8")
        return run, wrapper

    def write_draft(self, mode="fixed-values", delegation=None):
        data = {
            "mode": mode,
            "by": "operator",
            "inputs": [self.rel(self.source)],
            "result": current_result(),
        }
        if mode == "delegate-strategist":
            data["delegation"] = delegation or {"fields": "all", "constraints": {}}
        self.draft.write_text(json.dumps(data, indent=2), encoding="utf-8")

    def seal_and_start(self):
        run, wrapper = self.prepare_run()
        self.write_draft()
        self.assertEqual(
            contract.seal(str(self.ppt), str(run), str(self.draft)),
            wrapper,
        )
        self.quiet(
            af.cmd_autonomy_start,
            types.SimpleNamespace(
                project="deck-test",
                run_id="deck-run",
                resume_reason=None,
                session_binding="codex:ppt-session",
            ),
        )
        return run, wrapper

    def test_fixed_values_seal_start_materialize_and_check(self):
        _, wrapper = self.seal_and_start()
        sealed = contract.validate_contract(
            wrapper, expected_session="codex:ppt-session"
        )
        self.assertEqual(sealed["vendor_commit"], contract.VENDOR_COMMIT)
        result_path = contract.materialize(
            str(wrapper), expected_session="codex:ppt-session"
        )
        self.assertEqual(
            json.loads(result_path.read_text(encoding="utf-8")),
            current_result(),
        )
        contract.validate_contract(
            wrapper,
            expected_session="codex:ppt-session",
            require_materialized=True,
        )

    def test_seal_refuses_allowed_parent_that_contains_sibling(self):
        run, _ = self.prepare_run(allowed=self.rel(self.ppt.parent))
        self.write_draft()
        with self.assertRaisesRegex(contract.ContractError, "sealed sibling"):
            contract.seal(str(self.ppt), str(run), str(self.draft))

    def test_delegation_is_all_or_nothing(self):
        run, _ = self.prepare_run()
        self.write_draft(
            mode="delegate-strategist",
            delegation={"fields": "style", "constraints": {}},
        )
        with self.assertRaisesRegex(contract.ContractError, "fields=all"):
            contract.seal(str(self.ppt), str(run), str(self.draft))

    def test_standard_template_files_are_part_of_discovered_closure(self):
        template_dir = self.ppt / "templates"
        template_dir.mkdir()
        (template_dir / "brand.pptx").write_bytes(b"template")
        run, _ = self.prepare_run()
        self.write_draft()
        with self.assertRaisesRegex(contract.ContractError, "omits PPT project facts"):
            contract.seal(str(self.ppt), str(run), str(self.draft))

    def test_input_drift_invalidates_contract(self):
        _, wrapper = self.seal_and_start()
        self.source.write_text("Mutated brief\n", encoding="utf-8")
        with self.assertRaisesRegex(contract.ContractError, "hash/path drift"):
            contract.validate_contract(
                wrapper, expected_session="codex:ppt-session"
            )

    def test_guard_blocks_ui_and_revalidates_export(self):
        run, wrapper = self.seal_and_start()
        confirm_command = (
            f'python system/skills/ppt-master/scripts/confirm_ui/server.py '
            f'"{self.ppt.as_posix()}" --daemon --wait'
        )
        payload = {
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {"command": confirm_command},
            "cwd": str(self.root),
            "session_id": "ppt-session",
        }
        denied = ppt_master_guard.decide(payload, harness="codex")
        reason = denied["hookSpecificOutput"]["permissionDecisionReason"]
        self.assertIn("materialize", reason)
        wrong_session = {**payload, "session_id": "other-session"}
        denied = ppt_master_guard.decide(wrong_session, harness="codex")
        self.assertIn(
            "different harness session",
            denied["hookSpecificOutput"]["permissionDecisionReason"],
        )
        with patch.object(
            ppt_master_guard,
            "decide",
            side_effect=RuntimeError("synthetic guard failure"),
        ):
            fail_closed = json.loads(
                ppt_master_guard.run(json.dumps(payload), harness="codex")
            )
        self.assertIn(
            "guard failed",
            fail_closed["hookSpecificOutput"]["permissionDecisionReason"],
        )
        contract.materialize(str(wrapper), expected_session="codex:ppt-session")

        export_payload = {
            **payload,
            "tool_input": {
                "command": (
                    "python system/skills/ppt-master/scripts/svg_to_pptx.py "
                    f'"{self.ppt.as_posix()}" --no-notes'
                )
            },
        }
        self.assertIsNone(
            ppt_master_guard.decide(export_payload, harness="codex")
        )
        self.source.write_text("Drift\n", encoding="utf-8")
        denied = ppt_master_guard.decide(export_payload, harness="codex")
        self.assertIn(
            "invalid",
            denied["hookSpecificOutput"]["permissionDecisionReason"].lower(),
        )

        fm, body = af.split_fm(run.read_text(encoding="utf-8"), str(run))
        fm = af.set_scalar(fm, "status", "complete", str(run))
        run.write_text(af.join_fm(fm, body), encoding="utf-8")
        self.assertIsNone(
            ppt_master_guard.decide(payload, harness="codex")
        )


if __name__ == "__main__":
    unittest.main()
