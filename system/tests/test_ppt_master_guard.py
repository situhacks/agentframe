import json
import os
import tempfile
import unittest

from system import af
from system.hooks import ppt_master_guard as guard
from system.hooks import svg_paragraph_lint as lint


SVG_HEAD = '<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="720" viewBox="0 0 1280 720">'
SVG_TAIL = "</svg>"


def svg(body):
    return f"{SVG_HEAD}{body}{SVG_TAIL}"


LONG_A = "The quarterly revenue exceeded every internal forecast this year"
LONG_B = "driven primarily by enterprise renewals and the new usage tier"
LONG_C = "while operating costs stayed flat against the previous baseline"


class TestParagraphLint(unittest.TestCase):
    def test_per_line_sibling_texts_flagged(self):
        content = svg(
            f'<text x="80" y="200" font-size="20" fill="#333333">{LONG_A}</text>'
            f'<text x="80" y="232" font-size="20" fill="#333333">{LONG_B}</text>'
            f'<text x="80" y="264" font-size="20" fill="#333333">{LONG_C}</text>'
        )
        findings = lint.check_svg_text(content)
        self.assertEqual(len(findings), 1)
        self.assertIn("3 sibling", findings[0])

    def test_two_long_lines_flagged(self):
        content = svg(
            f'<text x="80" y="200" font-size="20" fill="#333333">{LONG_A}</text>'
            f'<text x="80" y="230" font-size="20" fill="#333333">{LONG_B}</text>'
        )
        self.assertEqual(len(lint.check_svg_text(content)), 1)

    def test_tspan_block_paragraph_clean(self):
        content = svg(
            '<text x="80" y="200" font-size="20" fill="#333333">'
            f'<tspan x="80" dy="0">{LONG_A}</tspan>'
            f'<tspan x="80" dy="32">{LONG_B}</tspan>'
            f'<tspan x="80" dy="32">{LONG_C}</tspan>'
            "</text>"
        )
        self.assertEqual(lint.check_svg_text(content), [])

    def test_kpi_label_stack_clean(self):
        content = svg(
            '<text x="80" y="200" font-size="16" fill="#666666">Revenue</text>'
            '<text x="80" y="240" font-size="32" fill="#1A73E8">$1.2M</text>'
        )
        self.assertEqual(lint.check_svg_text(content), [])

    def test_short_same_style_labels_clean(self):
        content = svg(
            '<text x="80" y="200" font-size="16" fill="#666666">Revenue</text>'
            '<text x="80" y="224" font-size="16" fill="#666666">Costs</text>'
            '<text x="80" y="248" font-size="16" fill="#666666">Margin</text>'
        )
        self.assertEqual(lint.check_svg_text(content), [])

    def test_different_x_columns_clean(self):
        content = svg(
            f'<text x="80" y="200" font-size="20" fill="#333333">{LONG_A}</text>'
            f'<text x="680" y="232" font-size="20" fill="#333333">{LONG_B}</text>'
        )
        self.assertEqual(lint.check_svg_text(content), [])

    def test_section_break_spacing_clean(self):
        content = svg(
            f'<text x="80" y="200" font-size="20" fill="#333333">{LONG_A}</text>'
            f'<text x="80" y="320" font-size="20" fill="#333333">{LONG_B}</text>'
        )
        self.assertEqual(lint.check_svg_text(content), [])

    def test_style_mismatch_clean(self):
        content = svg(
            f'<text x="80" y="200" font-size="20" fill="#333333" font-weight="bold">{LONG_A}</text>'
            f'<text x="80" y="232" font-size="20" fill="#333333">{LONG_B}</text>'
        )
        self.assertEqual(lint.check_svg_text(content), [])

    def test_inline_tspans_still_grouped(self):
        # Inline (non-positional) tspans are runs, not lines — the parent
        # <text> still counts as one line of a split paragraph.
        content = svg(
            f'<text x="80" y="200" font-size="20" fill="#333333">{LONG_A}</text>'
            '<text x="80" y="232" font-size="20" fill="#333333">'
            f'growth of <tspan font-weight="bold">35%</tspan> beat the plan across all regions'
            "</text>"
        )
        self.assertEqual(len(lint.check_svg_text(content)), 1)


def payload(command, cwd, event="PreToolUse"):
    return {
        "hook_event_name": event,
        "tool_name": "Bash",
        "tool_input": {"command": command},
        "cwd": cwd,
    }


def cursor_shell_payload(command, cwd, event="beforeShellExecution"):
    return {
        "hook_event_name": event,
        "cursor_version": "test",
        "command": command,
        "cwd": cwd,
    }


class TestStagingGuard(unittest.TestCase):
    def setUp(self):
        self.skill_dir = os.path.join(guard.ROOT, "system", "skills", "ppt-master")

    def test_init_without_dir_in_skill_cwd_denied(self):
        p = payload("python3 scripts/project_manager.py init demo_deck --format 16:9", self.skill_dir)
        out = guard.decide(p)
        self.assertEqual(out["hookSpecificOutput"]["permissionDecision"], "deny")
        self.assertIn("--dir", out["hookSpecificOutput"]["permissionDecisionReason"])

    def test_init_with_dir_inside_skill_denied(self):
        p = payload(
            f'python scripts/project_manager.py init demo --dir "{self.skill_dir}\\projects"',
            guard.ROOT,
        )
        out = guard.decide(p)
        self.assertEqual(out["hookSpecificOutput"]["permissionDecision"], "deny")

    def test_init_with_workspace_dir_allowed(self):
        target = os.path.join(guard.ROOT, "workspace", "projects", "demo", "phase-4-production", "decks")
        p = payload(f'python scripts/project_manager.py init demo --dir "{target}"', self.skill_dir)
        self.assertIsNone(guard.decide(p))

    def test_init_with_tmp_dir_allowed(self):
        p = payload('python scripts/project_manager.py init demo --dir "C:\\tmp\\decks"', self.skill_dir)
        self.assertIsNone(guard.decide(p))

    def test_non_init_subcommand_ignored(self):
        p = payload("python scripts/project_manager.py info projects/demo", self.skill_dir)
        self.assertIsNone(guard.decide(p))

    def test_unrelated_command_ignored(self):
        self.assertIsNone(guard.decide(payload("git status", guard.ROOT)))

    def test_cursor_before_shell_uses_native_deny_envelope(self):
        p = cursor_shell_payload(
            "python scripts/project_manager.py init demo",
            self.skill_dir,
        )
        out = json.loads(guard.run(json.dumps(p)))
        self.assertEqual(out["permission"], "deny")
        self.assertIn("--dir", out["agent_message"])

    def test_cursor_imported_claude_twin_is_suppressed(self):
        p = cursor_shell_payload(
            "python scripts/project_manager.py init demo",
            self.skill_dir,
        )
        raw = json.dumps(p)
        self.assertEqual(json.loads(guard.dispatch(raw, [])), {})
        native = json.loads(guard.dispatch(raw, ["--cursor-native"]))
        self.assertEqual(native["permission"], "deny")


class TestExportGuard(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.proj = self.tmp.name
        os.makedirs(os.path.join(self.proj, "svg_output"))

    def tearDown(self):
        self.tmp.cleanup()

    def write_svg(self, name, content):
        path = os.path.join(self.proj, "svg_output", name)
        with open(path, "w", encoding="utf-8") as f:
            f.write(content)

    def export_payload(self, extra="", event="PreToolUse"):
        cmd = f'python3 scripts/svg_to_pptx.py "{self.proj}" --no-notes{extra}'
        return payload(cmd, os.path.dirname(self.proj), event=event)

    def test_export_with_split_paragraph_denied(self):
        self.write_svg("01_cover.svg", svg(
            f'<text x="80" y="200" font-size="20" fill="#333333">{LONG_A}</text>'
            f'<text x="80" y="232" font-size="20" fill="#333333">{LONG_B}</text>'
        ))
        out = guard.decide(self.export_payload())
        self.assertEqual(out["hookSpecificOutput"]["permissionDecision"], "deny")
        reason = out["hookSpecificOutput"]["permissionDecisionReason"]
        self.assertIn("01_cover.svg", reason)
        self.assertIn("tspan", reason)

    def test_clean_export_allowed(self):
        self.write_svg("01_cover.svg", svg(
            '<text x="80" y="200" font-size="20" fill="#333333">'
            f'<tspan x="80" dy="0">{LONG_A}</tspan>'
            f'<tspan x="80" dy="32">{LONG_B}</tspan>'
            "</text>"
        ))
        self.assertIsNone(guard.decide(self.export_payload()))

    def test_lint_escape_hatch(self):
        self.write_svg("01_cover.svg", svg(
            f'<text x="80" y="200" font-size="20" fill="#333333">{LONG_A}</text>'
            f'<text x="80" y="232" font-size="20" fill="#333333">{LONG_B}</text>'
        ))
        p = payload(
            f'AF_PPT_LINT=off python3 scripts/svg_to_pptx.py "{self.proj}" --no-notes',
            os.path.dirname(self.proj),
        )
        self.assertIsNone(guard.decide(p))

    def test_post_export_promotion_reminder(self):
        out = guard.decide(self.export_payload(event="PostToolUse"))
        ctx = out["hookSpecificOutput"]["additionalContext"]
        self.assertIn("promote", ctx.lower())
        self.assertIn("exports/", ctx)

    def test_post_unrelated_command_ignored(self):
        self.assertIsNone(guard.decide(payload("git status", guard.ROOT, event="PostToolUse")))

    def test_cursor_post_tool_uses_native_context_envelope(self):
        p = self.export_payload(event="postToolUse")
        p["cursor_version"] = "test"
        out = json.loads(guard.run(json.dumps(p)))
        self.assertIn("promote", out["additional_context"].lower())


class TestGuardMain(unittest.TestCase):
    def test_main_emits_json_on_deny(self):
        skill_dir = os.path.join(guard.ROOT, "system", "skills", "ppt-master")
        p = payload("python scripts/project_manager.py init demo", skill_dir)
        result = guard.run(json.dumps(p))
        self.assertIsNotNone(result)
        parsed = json.loads(result)
        self.assertEqual(parsed["hookSpecificOutput"]["permissionDecision"], "deny")

    def test_main_silent_on_pass(self):
        self.assertIsNone(guard.run(json.dumps(payload("git status", str(guard.ROOT)))))

    def test_main_silent_on_garbage(self):
        self.assertIsNone(guard.run("not json"))


class TestDoctorStrayDecks(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = self.tmp.name
        self.skill = os.path.join(self.root, "system", "skills", "ppt-master")
        os.makedirs(self.skill)

    def tearDown(self):
        self.tmp.cleanup()

    def test_stray_project_dir_is_issue(self):
        os.makedirs(os.path.join(self.skill, "projects", "demo", "exports"))
        issues = af.ppt_master_stray_issues(self.root)
        self.assertTrue(any("projects" in i for i in issues))

    def test_stray_pptx_is_issue(self):
        target = os.path.join(self.skill, "templates", "decks")
        os.makedirs(target)
        with open(os.path.join(target, "deck_20260709.pptx"), "wb") as f:
            f.write(b"")
        issues = af.ppt_master_stray_issues(self.root)
        self.assertTrue(any("deck_20260709.pptx" in i for i in issues))

    def test_clean_tree_no_issues(self):
        self.assertEqual(af.ppt_master_stray_issues(self.root), [])

    def test_missing_skill_dir_no_issues(self):
        self.assertEqual(af.ppt_master_stray_issues(os.path.join(self.root, "nope")), [])


if __name__ == "__main__":
    unittest.main()
