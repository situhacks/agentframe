import tempfile
import unittest
from pathlib import Path

from system.server.lib import hub


class HubTests(unittest.TestCase):
    def _flatten_entries(self, node: dict) -> list[dict]:
        entries = list(node["entries"])
        for child in node["dirs"]:
            entries.extend(self._flatten_entries(child))
        return entries

    def _find_dir(self, node: dict, parts: list[str]) -> dict | None:
        current = node
        for part in parts:
            current = next((d for d in current["dirs"] if d["name"] == part), None)
            if current is None:
                return None
        return current

    def test_scan_workspace_surfaces_video_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            video_dir = (
                root
                / "workspace"
                / "projects"
                / "demo-project"
                / "phase-4-production"
                / "posts"
                / "post-1"
                / "video"
            )
            renders_dir = video_dir / "renders"
            renders_dir.mkdir(parents=True)
            (video_dir / "index.html").write_text("<html></html>", encoding="utf-8")
            (renders_dir / "final.mp4").write_bytes(b"fake mp4")
            (renders_dir / "transition.webm").write_bytes(b"fake webm")
            (renders_dir / "source.mov").write_bytes(b"fake mov")

            model = hub.scan_workspace(root)
            entries = self._flatten_entries(model["active"][0]["tree"])
            by_id = {entry["id"]: entry for entry in entries}
            base = "/workspace/projects/demo-project/phase-4-production/posts/post-1/video"

            self.assertEqual(by_id[f"{base}/renders/final.mp4"]["kind"], "video")
            self.assertEqual(by_id[f"{base}/renders/source.mov"]["kind"], "video")
            self.assertEqual(by_id[f"{base}/renders/transition.webm"]["kind"], "video")
            self.assertEqual(by_id[f"{base}/index.html"]["kind"], "single")

    def test_scan_workspace_produces_nested_tree(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            campaign_root = root / "workspace" / "projects" / "demo-project"
            (campaign_root / "phase-3-planning" / "design-language" / "preview").mkdir(parents=True)
            (campaign_root / "phase-4-production" / "posts" / "post-1" / "video").mkdir(parents=True)
            (campaign_root / "phase-3-planning" / "design-language" / "preview" / "file.html").write_text(
                "<html></html>",
                encoding="utf-8",
            )
            (campaign_root / "phase-4-production" / "posts" / "post-1" / "video" / "index.html").write_text(
                "<html></html>",
                encoding="utf-8",
            )

            model = hub.scan_workspace(root)
            tree = model["active"][0]["tree"]

            phase3_preview = self._find_dir(
                tree,
                ["phase-3-planning", "design-language", "preview"],
            )
            self.assertIsNotNone(phase3_preview)
            self.assertTrue(
                any(
                    entry["id"].endswith("/phase-3-planning/design-language/preview/file.html")
                    for entry in phase3_preview["entries"]
                )
            )

            phase4_video = self._find_dir(
                tree,
                ["phase-4-production", "posts", "post-1", "video"],
            )
            self.assertIsNotNone(phase4_video)
            self.assertTrue(
                any(
                    entry["id"].endswith("/phase-4-production/posts/post-1/video/index.html")
                    for entry in phase4_video["entries"]
                )
            )

    def test_scan_keeps_carousel_group_as_leaf(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            post_dir = (
                root
                / "workspace"
                / "projects"
                / "demo-project"
                / "phase-4-production"
                / "posts"
                / "post-1"
            )
            visuals = post_dir / "visuals"
            visuals.mkdir(parents=True)
            (visuals / "carousel-slide-1.html").write_text("<html></html>", encoding="utf-8")
            (post_dir / "copy.md").write_text("copy", encoding="utf-8")

            model = hub.scan_workspace(root)
            tree = model["active"][0]["tree"]
            posts_dir = self._find_dir(tree, ["phase-4-production", "posts"])
            self.assertIsNotNone(posts_dir)
            self.assertTrue(
                any(
                    entry["kind"] == "group" and entry["label"] == "post-1"
                    for entry in posts_dir["entries"]
                )
            )

            all_entries = self._flatten_entries(tree)
            self.assertFalse(
                any(entry["id"].endswith("/visuals/carousel-slide-1.html") for entry in all_entries)
            )

    def test_scan_excludes_glob_matches(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            phase_dir = (
                root
                / "workspace"
                / "projects"
                / "demo-project"
                / "phase-4-production"
                / "posts"
                / "post-1"
            )
            (phase_dir / "history").mkdir(parents=True)
            (phase_dir / "history" / "scratch.png").write_bytes(b"fake png")

            filtered = hub.scan_workspace(root, exclude_globs=["**/history/**"])
            unfiltered = hub.scan_workspace(root, exclude_globs=[])

            filtered_ids = {entry["id"] for entry in self._flatten_entries(filtered["active"][0]["tree"])}
            unfiltered_ids = {entry["id"] for entry in self._flatten_entries(unfiltered["active"][0]["tree"])}

            target = "/workspace/projects/demo-project/phase-4-production/posts/post-1/history/scratch.png"
            self.assertNotIn(target, filtered_ids)
            self.assertIn(target, unfiltered_ids)

    def test_scan_honors_preview_hide_marker(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            hidden_dir = (
                root
                / "workspace"
                / "projects"
                / "demo-project"
                / "phase-4-production"
                / "posts"
                / "post-1"
                / "video"
                / "assets"
            )
            hidden_dir.mkdir(parents=True)
            (hidden_dir / ".preview-hide").write_text("", encoding="utf-8")
            (hidden_dir / "hidden.png").write_bytes(b"fake png")

            model = hub.scan_workspace(root)
            ids = {entry["id"] for entry in self._flatten_entries(model["active"][0]["tree"])}

            self.assertNotIn(
                "/workspace/projects/demo-project/phase-4-production/posts/post-1/video/assets/hidden.png",
                ids,
            )

    def test_scan_intermediates_override_returns_everything(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            phase_dir = (
                root
                / "workspace"
                / "projects"
                / "demo-project"
                / "phase-4-production"
                / "posts"
                / "post-1"
            )
            (phase_dir / "history").mkdir(parents=True)
            (phase_dir / "history" / "scratch.png").write_bytes(b"fake png")
            (phase_dir / "final.png").write_bytes(b"fake png")

            filtered = hub.scan_workspace(root, exclude_globs=["**/history/**"])
            with_intermediates = hub.scan_workspace(
                root,
                exclude_globs=["**/history/**"],
                include_intermediates=True,
            )

            filtered_ids = {entry["id"] for entry in self._flatten_entries(filtered["active"][0]["tree"])}
            intermediates_ids = {
                entry["id"] for entry in self._flatten_entries(with_intermediates["active"][0]["tree"])
            }

            self.assertIn(
                "/workspace/projects/demo-project/phase-4-production/posts/post-1/final.png",
                filtered_ids,
            )
            self.assertNotIn(
                "/workspace/projects/demo-project/phase-4-production/posts/post-1/history/scratch.png",
                filtered_ids,
            )
            self.assertIn(
                "/workspace/projects/demo-project/phase-4-production/posts/post-1/history/scratch.png",
                intermediates_ids,
            )
            self.assertGreaterEqual(len(intermediates_ids), len(filtered_ids))

    def test_render_hub_html_renders_nested_details(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            campaign_root = root / "workspace" / "projects" / "demo-project"
            (campaign_root / "phase-3-planning" / "design-language" / "preview").mkdir(parents=True)
            (campaign_root / "phase-4-production" / "posts" / "post-1" / "video").mkdir(parents=True)
            visuals = campaign_root / "phase-4-production" / "posts" / "post-1" / "visuals"
            visuals.mkdir(parents=True)
            (campaign_root / "phase-3-planning" / "design-language" / "preview" / "file.html").write_text(
                "<html></html>",
                encoding="utf-8",
            )
            (campaign_root / "phase-4-production" / "posts" / "post-1" / "video" / "index.html").write_text(
                "<html></html>",
                encoding="utf-8",
            )
            (visuals / "carousel-slide-1.html").write_text("<html></html>", encoding="utf-8")

            model = hub.scan_workspace(root)
            html = hub.render_hub_html(model)

            self.assertIn('<details class="folder"><summary>phase-4-production</summary>', html)
            self.assertIn('<details class="folder"><summary>video</summary>', html)
            self.assertIn('class="entry kind-group"', html)

    def test_render_hub_html_mounts_video_artifacts(self) -> None:
        model = {
            "active": [{
                "slug": "demo-project",
                "tree": {
                    "name": "demo-project",
                    "path": "",
                    "dirs": [],
                    "entries": [{
                        "kind": "video",
                        "id": "/workspace/projects/demo/phase-4-production/posts/post-1/video/renders/final.mp4",
                        "label": "final.mp4",
                        "url": "/workspace/projects/demo/phase-4-production/posts/post-1/video/renders/final.mp4",
                        "mtime": 1.0,
                    }],
                },
            }],
            "completed": [],
            "demo": {"name": "demo", "path": "", "dirs": [], "entries": []},
            "default_id": "/workspace/projects/demo/phase-4-production/posts/post-1/video/renders/final.mp4",
        }

        html = hub.render_hub_html(model)

        self.assertIn("[video] final.mp4", html)
        self.assertIn("function mountVideo(artifact)", html)
        self.assertIn("artifact.kind === 'video'", html)


if __name__ == "__main__":
    unittest.main()
