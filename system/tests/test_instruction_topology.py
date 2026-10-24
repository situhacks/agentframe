from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[2]


def text(rel):
    return (ROOT / rel).read_text(encoding="utf-8-sig")


OPERATOR_ONLY_SKILLS = ("deliverable-harvest", "voice-harvest", "doc-export", "job-scout",
                        "d2-diagrams", "extract-design")
MODEL_INVOCABLE_SKILLS = ("agent-reach", "deep-research")


def frontmatter(rel):
    return text(rel).split("---", 2)[1]


class InstructionTopologyCharacterizationTests(unittest.TestCase):
    """Pin the discovery and routing shape."""

    def test_root_is_a_small_stable_task_classifier(self):
        root = text("AGENTS.md")
        self.assertNotEqual(root, text("AGENTS.operator.md"))
        self.assertNotEqual(root, text("AGENTS.builder.md"))
        self.assertIn("AGENTS.operator.md", root)
        self.assertIn("AGENTS.builder.md", root)
        self.assertIn("read exactly one router", root)
        self.assertLessEqual(len(root.split()), 500)

    def test_skill_and_vendor_catalogs_carry_their_columns(self):
        self.assertIn("| Skill | Owns | Load when | Invocation |", text(".claude/skills/README.md"))
        self.assertIn("| Runtime | What it is | Routed by | Refresh |", text("system/vendor/README.md"))
        self.assertFalse((ROOT / "system" / "skills").exists(), "a skills folder under system/ is retired: vendored runtimes go in system/vendor/, owned skills in .claude/skills/")

    def test_every_skill_folder_has_a_catalog_row_and_a_matching_name(self):
        catalog = text(".claude/skills/README.md")
        for skill in sorted(p.parent for p in (ROOT / ".claude" / "skills").glob("*/SKILL.md")):
            rel = f".claude/skills/{skill.name}/SKILL.md"
            self.assertRegex(frontmatter(rel), rf"(?m)^name:\s*{skill.name}\s*$", rel)
            self.assertIn(f"[`{skill.name}/`]", catalog, f"{skill.name} has no row in .claude/skills/README.md")
        vendor = text("system/vendor/README.md")
        for runtime in sorted(p for p in (ROOT / "system" / "vendor").iterdir()
                              if p.is_dir() and not p.name.startswith(("_", "."))):
            self.assertIn(f"[`{runtime.name}/`]", vendor, f"{runtime.name} has no row in system/vendor/README.md")
            self.assertTrue((runtime / "VENDOR.md").is_file(), f"system/vendor/{runtime.name} has no VENDOR.md")

    def test_operator_only_skills_are_hidden_from_the_model(self):
        for name in OPERATOR_ONLY_SKILLS:
            self.assertRegex(frontmatter(f".claude/skills/{name}/SKILL.md"),
                             r"(?m)^disable-model-invocation:\s*true\s*$", name)
        for name in MODEL_INVOCABLE_SKILLS:
            self.assertNotIn("disable-model-invocation", frontmatter(f".claude/skills/{name}/SKILL.md"), name)

    def test_marketing_production_has_no_pseudo_links(self):
        production = text("library/domains/marketing/production.md")
        for pseudo_link in ("[voice]", "[positioning]"):
            self.assertNotIn(pseudo_link, production)
        self.assertIn("[`voice`](../../../workspace/life/voice/README.md)", production)

    def test_public_prose_templates_have_an_action_anchored_prewrite_gate(self):
        for rel in (
            "library/domains/marketing/deliverables/body-copy/template.md",
            "library/domains/marketing/deliverables/slide-copy/template.md",
        ):
            template = text(rel)
            self.assertIn("## Before Writing", template, rel)
            self.assertIn("system/af.py draft", template, rel)
            self.assertIn("system/af.py version", template, rel)
            self.assertIn("voice/README.md", template, rel)

    def test_task_routers_stay_small_and_task_local(self):
        self.assertLessEqual(len(text("AGENTS.operator.md").split()), 1000)
        self.assertLessEqual(len(text("AGENTS.builder.md").split()), 900)
        for rel in ("AGENTS.operator.md", "AGENTS.builder.md"):
            self.assertIn("task-local", text(rel), rel)
            self.assertIn("system/backlog.md", text(rel), rel)

    def test_operator_context_routes_stay_conditional(self):
        operator = text("AGENTS.operator.md")
        self.assertIn("Project formation (no folder yet)", operator)
        self.assertNotIn("OR loading an existing one", operator)
        self.assertIn("only for state creation/mutation, schema questions, or reported drift", operator)
        self.assertIn("represent the operator to another person", operator)
        self.assertIn("skip voice for private working text", operator)
        self.assertNotIn("voice/anti-patterns.md", operator)

    def test_life_layer_is_discoverable_private_and_not_a_project(self):
        operator = text("AGENTS.operator.md")
        self.assertIn("| `workspace/life/` | The operator layer: ", operator)
        self.assertIn("`workspace/life/` is the operator layer, not a project: "
                      "the migration created it once", operator)
        self.assertNotIn("new-project life", operator)

        ignore = text(".gitignore")
        self.assertRegex(ignore, r"(?m)^workspace/$")
        self.assertNotIn("!workspace/", ignore)

    def test_model_invocable_skills_have_discoverable_descriptions(self):
        for name in MODEL_INVOCABLE_SKILLS:
            rel = f".claude/skills/{name}/SKILL.md"
            self.assertRegex(frontmatter(rel), r"(?m)^description:\s*(\||\S)", rel)
        self.assertIn("Do not use for a quick lookup", text(".claude/skills/deep-research/SKILL.md"))

if __name__ == "__main__":
    unittest.main()
