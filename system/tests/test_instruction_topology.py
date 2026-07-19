from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[2]


def text(rel):
    return (ROOT / rel).read_text(encoding="utf-8-sig")


class InstructionTopologyCharacterizationTests(unittest.TestCase):
    """Pin the pre-rearchitecture discovery and routing gaps."""

    def test_root_is_a_small_stable_task_classifier(self):
        root = text("AGENTS.md")
        self.assertNotEqual(root, text("AGENTS.operator.md"))
        self.assertNotEqual(root, text("AGENTS.builder.md"))
        self.assertIn("AGENTS.daemon.md", root)
        self.assertIn("AGENTS.operator.md", root)
        self.assertIn("AGENTS.builder.md", root)
        self.assertIn("read exactly one router", root)
        self.assertLessEqual(len(root.split()), 500)

    def test_mode_swap_writer_retires_persona_copying(self):
        writer = text("system/audit/writer.py")
        self.assertIn("mode_swap is retired", writer)
        self.assertNotIn("MODE_SWAP_PERSONA_FILES", writer)
        self.assertNotIn("destination.write_bytes(source.read_bytes())", writer)

    def test_skill_catalog_carries_load_when_metadata(self):
        catalog = text("system/skills/README.md")
        self.assertIn("| Skill | Owns | Load when | Provenance |", catalog)
        self.assertNotIn("| Skill | Purpose | Provenance |", catalog)

    def test_marketing_production_has_no_pseudo_links(self):
        production = text("library/domains/marketing/production.md")
        for pseudo_link in ("[voice]", "[positioning]", "[preview-server]"):
            self.assertNotIn(pseudo_link, production)
        self.assertIn("[`voice`](../../context/operator/voice/README.md)", production)

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

    def test_humanizer_is_early_and_authorship_aware(self):
        process = text("library/process/humanizer-integration.md")
        self.assertIn("Initial agent-authored prose", process)
        self.assertIn("Operator hand-tuning", process)
        self.assertIn("Lock with no new agent-authored prose", process)
        self.assertNotIn("Append one `humanizer_pass` event", process)

    def test_task_routers_remain_lazy_loaded_diagnostics(self):
        sizes = {}
        for rel in ("AGENTS.operator.md", "AGENTS.builder.md"):
            content = text(rel)
            sizes[rel] = {"words": len(content.split()), "characters": len(content)}
        self.assertGreater(sizes["AGENTS.operator.md"]["words"], 1000)
        self.assertLessEqual(sizes["AGENTS.operator.md"]["words"], 1700)
        self.assertGreater(sizes["AGENTS.builder.md"]["words"], 1000)
        for rel in ("AGENTS.operator.md", "AGENTS.builder.md"):
            self.assertIn("task-local", text(rel), rel)

    def test_operator_context_routes_stay_conditional(self):
        operator = text("AGENTS.operator.md")
        self.assertIn("Project formation (no folder yet)", operator)
        self.assertNotIn("OR loading an existing one", operator)
        self.assertIn("only for state creation/mutation, schema questions, or reported drift", operator)
        self.assertIn("represent the operator to another person", operator)
        self.assertIn("skip voice for private working text", operator)
        self.assertNotIn("voice/anti-patterns.md", operator)

    def test_pilot_skills_have_discoverable_positive_and_near_miss_descriptions(self):
        for rel in (
            "system/skills/humanizer/SKILL.md",
            "system/skills/deep-research/SKILL.md",
            "system/skills/agentframe-structure/SKILL.md",
        ):
            frontmatter = text(rel).split("---", 2)[1]
            self.assertRegex(frontmatter, r"(?m)^name:\s*\S+", rel)
            self.assertRegex(frontmatter, r"(?m)^description:\s*(\||\S)", rel)
        self.assertIn("Do not use for a quick lookup", text("system/skills/deep-research/SKILL.md"))
        self.assertIn("Do not use for ordinary one-file patches", text("system/skills/agentframe-structure/SKILL.md"))


if __name__ == "__main__":
    unittest.main()
