import contextlib
import datetime
import io
import os
import shutil
import tempfile
import unittest
from types import SimpleNamespace

from system import af

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CREATOR_PACK = os.path.join(REPO_ROOT, "library", "domains", "creator")
CAREERS_PACK = os.path.join(REPO_ROOT, "library", "domains", "careers")


def write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)


def quiet(fn, *a, **kw):
    with contextlib.redirect_stdout(io.StringIO()):
        return fn(*a, **kw)


class StudioBase(unittest.TestCase):
    """Temp-root harness: af globals repointed, real creator pack copied in."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        root = self.tmp.name
        self._saved = {k: getattr(af, k) for k in ("ROOT", "PROJECTS", "DOMAINS", "PIPELINE", "STUDIO")}
        af.ROOT = root
        af.PROJECTS = os.path.join(root, "workspace", "projects")
        af.DOMAINS = os.path.join(root, "library", "domains")
        af.PIPELINE = os.path.join(root, "workspace", "pipeline")
        af.STUDIO = os.path.join(root, "workspace", "studio")
        os.makedirs(af.PROJECTS)
        shutil.copytree(CREATOR_PACK, os.path.join(af.DOMAINS, "creator"))
        self.clip = os.path.join(root, "inbox", "take1.mov")
        write(self.clip, "fake video bytes")

    def tearDown(self):
        for k, v in self._saved.items():
            setattr(af, k, v)
        self.tmp.cleanup()

    def new(self, slug="meta-read-1", date=None, source=None, series=None, platform="tiktok", name=None):
        quiet(af.cmd_studio_new, SimpleNamespace(slug=slug, date=date, source=source, series=series,
                                                 platform=platform, name=name))
        return slug

    def stage(self, slug, state):
        quiet(af.cmd_studio_stage, SimpleNamespace(slug=slug, state=state))

    def post(self, slug, url="https://www.tiktok.com/@b/video/1", posted_at=None):
        quiet(af.cmd_studio_post, SimpleNamespace(slug=slug, url=url, posted_at=posted_at))

    def cal_fm(self):
        return af.split_fm(af.read(af.calendar_path()), "calendar.md")[0]

    def post_fm(self, slug):
        return af.split_fm(af.read(os.path.join(af.post_dir(slug), "post.md")), "post.md")[0]

    def render(self, slug):
        write(af.post_render(slug), "mp4 bytes")


class TestNew(StudioBase):
    def test_planned_needs_a_date(self):
        with self.assertRaises(SystemExit):
            self.new()

    def test_planned_creates_row_folder_and_records(self):
        slug = self.new(date="2026-10-01", series="patch-notes")
        fm = self.cal_fm()
        self.assertEqual(af.slot_rows(fm), [slug])
        self.assertEqual(af.slot_get(fm, slug, "state"), "planned")
        self.assertEqual(af.slot_get(fm, slug, "date"), "2026-10-01")
        self.assertEqual(af.slot_get(fm, slug, "series"), "patch-notes")
        pfm = self.post_fm(slug)
        self.assertEqual(af.get_scalar(pfm, "domain"), "creator")
        self.assertEqual(af.get_scalar(pfm, "slug"), slug)
        self.assertEqual(af.get_scalar(pfm, "platform"), "tiktok")
        self.assertEqual(af.get_scalar(pfm, "date"), "2026-10-01")
        self.assertTrue(os.path.isfile(os.path.join(af.post_dir(slug), "edit.md")))
        self.assertTrue(os.path.isdir(os.path.join(af.post_dir(slug), "video")))
        self.assertTrue(os.path.isdir(os.path.join(af.post_dir(slug), "renders")))

    def test_captured_from_a_file(self):
        slug = self.new(source=self.clip)
        fm = self.cal_fm()
        self.assertEqual(af.slot_get(fm, slug, "state"), "captured")
        self.assertIsNone(af.slot_get(fm, slug, "date"))
        src = os.path.join(af.post_dir(slug), "video", "source.mov")
        self.assertTrue(os.path.isfile(src))
        self.assertEqual(af.read(src), "fake video bytes")
        self.assertEqual(af.get_scalar(self.post_fm(slug), "source_file"), "video/source.mov")

    def test_duplicate_and_bad_date_refused(self):
        self.new(date="2026-10-01")
        with self.assertRaises(SystemExit):
            self.new(date="2026-10-02")
        with self.assertRaises(SystemExit):
            self.new(slug="other", date="Oct 1")

    def test_new_project_refuses_studio_topology(self):
        with self.assertRaises(SystemExit):
            quiet(af.cmd_new_project, SimpleNamespace(slug="x", domain="creator", flow="open-flow", name=None))

    def test_posts_resolve_as_work_folders(self):
        slug = self.new(date="2026-10-01")
        self.assertEqual(af.project_dir(slug), af.post_dir(slug))
        self.assertEqual(af.state_doc(af.post_dir(slug)), "post.md")

    def test_draft_script_on_a_post(self):
        slug = self.new(date="2026-10-01")
        quiet(af.cmd_draft, SimpleNamespace(project=slug, deliverable="script",
                                            file="script/script-v1.md", artifact=None))
        self.assertTrue(os.path.isfile(os.path.join(af.post_dir(slug), "script", "script-v1.md")))
        self.assertEqual(af.row_get(self.post_fm(slug), "script", "status"), "drafting")


class TestStateMachine(StudioBase):
    def test_captured_requires_a_source(self):
        slug = self.new(date="2026-10-01")
        with self.assertRaises(SystemExit):
            self.stage(slug, "captured")
        write(os.path.join(af.post_dir(slug), "video", "source.mp4"), "v")
        self.stage(slug, "captured")
        self.assertEqual(af.slot_get(self.cal_fm(), slug, "state"), "captured")

    def test_cut_and_scheduled_require_a_render(self):
        slug = self.new(source=self.clip)
        with self.assertRaises(SystemExit):
            self.stage(slug, "cut")
        self.render(slug)
        self.stage(slug, "cut")
        self.stage(slug, "scheduled")
        self.assertEqual(af.slot_get(self.cal_fm(), slug, "state"), "scheduled")

    def test_illegal_transition_refused(self):
        slug = self.new(date="2026-10-01")
        with self.assertRaises(SystemExit):
            self.stage(slug, "scheduled")

    def test_stage_posted_is_refused_in_favour_of_post(self):
        slug = self.new(source=self.clip)
        self.render(slug)
        self.stage(slug, "cut")
        with self.assertRaises(SystemExit):
            self.stage(slug, "posted")

    def test_post_requires_a_render_state(self):
        slug = self.new(source=self.clip)
        with self.assertRaises(SystemExit):
            self.post(slug)

    def test_post_writes_receipt_on_board_and_record(self):
        slug = self.new(source=self.clip)
        self.render(slug)
        self.stage(slug, "cut")
        self.post(slug, url="https://www.tiktok.com/@b/video/42", posted_at="2026-09-08T10:00:00-07:00")
        fm = self.cal_fm()
        self.assertEqual(af.slot_get(fm, slug, "state"), "posted")
        self.assertEqual(af.slot_get(fm, slug, "link"), "https://www.tiktok.com/@b/video/42")
        self.assertEqual(af.slot_get(fm, slug, "posted"), "2026-09-08")
        pfm = self.post_fm(slug)
        self.assertEqual(af.get_scalar(pfm, "posted_at"), "2026-09-08T10:00:00-07:00")
        self.assertEqual(af.get_scalar(pfm, "url"), "https://www.tiktok.com/@b/video/42")

    def test_scheduled_can_return_to_cut(self):
        slug = self.new(source=self.clip)
        self.render(slug)
        self.stage(slug, "cut")
        self.stage(slug, "scheduled")
        self.stage(slug, "cut")
        self.assertEqual(af.slot_get(self.cal_fm(), slug, "state"), "cut")


class TestStudioDoctor(StudioBase):
    def test_clean_studio_is_quiet(self):
        self.new(source=self.clip)
        self.new(slug="planned-one", date="2099-01-01")
        issues, notes = af.check_studio()
        self.assertEqual(issues, [])
        self.assertEqual(notes, [])

    def test_missing_folder_and_orphan_folder_are_issues(self):
        slug = self.new(date="2099-01-01")
        shutil.rmtree(af.post_dir(slug))
        write(os.path.join(af.posts_root(), "orphan", "post.md"), "---\nslug: orphan\n---\n")
        issues, _ = af.check_studio()
        self.assertTrue(any("has no post folder" in i for i in issues))
        self.assertTrue(any("orphan" in i and "no board row" in i for i in issues))

    def test_missing_render_behind_cut_is_an_issue(self):
        slug = self.new(source=self.clip)
        self.render(slug)
        self.stage(slug, "cut")
        os.remove(af.post_render(slug))
        issues, _ = af.check_studio()
        self.assertTrue(any("renders/final.mp4 is missing" in i for i in issues))

    def test_overdue_scheduled_and_capture_due_notes(self):
        slug = self.new(source=self.clip)
        self.render(slug)
        self.stage(slug, "cut")
        self.stage(slug, "scheduled")
        fm, body = af.load_calendar()
        af.write_calendar(af.slot_set(fm, slug, "date", "2026-01-01"), body)
        _, notes = af.check_studio()
        self.assertTrue(any("has no receipt" in n for n in notes))
        old = (datetime.date.today() - datetime.timedelta(days=30)).isoformat() + "T09:00:00-07:00"
        self.post(slug, posted_at=old)
        _, notes = af.check_studio()
        self.assertTrue(any("metrics not captured" in n for n in notes))

    def test_doctor_studio_scope_runs_clean(self):
        self.new(source=self.clip)
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            af.cmd_doctor(SimpleNamespace(project="studio"))
        self.assertIn("+ studio", out.getvalue())


class TestTopologyPack(StudioBase):
    def test_singleton_is_enforced(self):
        shutil.copytree(CREATOR_PACK, os.path.join(af.DOMAINS, "creator-two"))
        with self.assertRaises(SystemExit):
            af.topology_pack("studio")

    def test_missing_topology_dies(self):
        with self.assertRaises(SystemExit):
            af.topology_pack("pipeline")

    def test_pipe_pack_alias_resolves_careers(self):
        shutil.copytree(CAREERS_PACK, os.path.join(af.DOMAINS, "careers"))
        self.assertEqual(af.pipe_pack()[0], "careers")


if __name__ == "__main__":
    unittest.main()
