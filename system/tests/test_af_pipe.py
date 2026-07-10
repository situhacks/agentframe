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
CAREERS_PACK = os.path.join(REPO_ROOT, "library", "domains", "careers")


def write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)


def quiet(fn, *a, **kw):
    with contextlib.redirect_stdout(io.StringIO()):
        return fn(*a, **kw)


class PipeBase(unittest.TestCase):
    """Temp-root harness: af globals repointed, real careers pack copied in."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        root = self.tmp.name
        self._saved = {k: getattr(af, k) for k in ("ROOT", "PROJECTS", "DOMAINS", "PIPELINE")}
        af.ROOT = root
        af.PROJECTS = os.path.join(root, "workspace", "projects")
        af.DOMAINS = os.path.join(root, "library", "domains")
        af.PIPELINE = os.path.join(root, "workspace", "pipeline")
        os.makedirs(af.PROJECTS)
        shutil.copytree(CAREERS_PACK, os.path.join(af.DOMAINS, "careers"))

    def tearDown(self):
        for k, v in self._saved.items():
            setattr(af, k, v)
        self.tmp.cleanup()

    # -- helpers -------------------------------------------------------------
    def save(self, slug="acme-ai-pm", **kw):
        args = SimpleNamespace(company=kw.get("company", "Acme"), role=kw.get("role", "AI PM"),
                               url=kw.get("url", "https://boards.greenhouse.io/acme/jobs/1"),
                               ats=kw.get("ats", "greenhouse"), source=kw.get("source", "manual"),
                               posted=kw.get("posted"), deadline=kw.get("deadline"),
                               salary=kw.get("salary"), slug=slug)
        quiet(af.cmd_pipe_save, args)
        return slug

    def start(self, slug):
        quiet(af.cmd_pipe_start, SimpleNamespace(slug=slug))

    def stage(self, slug, stage):
        quiet(af.cmd_pipe_stage, SimpleNamespace(slug=slug, stage=stage))

    def board_fm(self):
        return af.split_fm(af.read(af.board_path()), "pipeline.md")[0]


class TestSaveAndStart(PipeBase):
    def test_save_creates_board_row(self):
        slug = self.save()
        fm = self.board_fm()
        self.assertEqual(af.pipe_rows(fm), [slug])
        self.assertEqual(af.row_get(fm, slug, "stage"), "saved")
        self.assertEqual(af.row_get(fm, slug, "saved"), af.today())
        self.assertEqual(af.row_get(fm, slug, "company"), "Acme")

    def test_duplicate_save_refused(self):
        self.save()
        with self.assertRaises(SystemExit):
            self.save()

    def test_start_scaffolds_and_moves_jd_cache(self):
        slug = self.save()
        write(af.jd_cache_path(slug), "verbatim JD text")
        self.start(slug)
        adir = af.app_dir(slug)
        self.assertTrue(os.path.isfile(os.path.join(adir, "application.md")))
        self.assertEqual(af.read(os.path.join(adir, "jd.md")), "verbatim JD text")
        self.assertFalse(os.path.exists(af.jd_cache_path(slug)))
        self.assertEqual(af.row_get(self.board_fm(), slug, "stage"), "preparing")
        afm, _ = af.split_fm(af.read(os.path.join(adir, "application.md")), "application.md")
        self.assertEqual(af.get_scalar(afm, "company"), "Acme")
        self.assertEqual(af.get_scalar(afm, "domain"), "careers")
        self.assertEqual(af.row_get(afm, "resume", "status"), "not_started")

    def test_start_requires_saved_stage(self):
        slug = self.save()
        self.start(slug)
        with self.assertRaises(SystemExit):
            self.start(slug)

    def test_new_project_refuses_pipeline_topology(self):
        with self.assertRaises(SystemExit):
            quiet(af.cmd_new_project, SimpleNamespace(slug="acme", domain="careers", flow="open-flow", name=None))


class TestStageMachine(PipeBase):
    def test_applied_stamps_dates_and_warns_on_unlocked_resume(self):
        slug = self.save()
        self.start(slug)
        self.stage(slug, "applied")
        fm = self.board_fm()
        self.assertEqual(af.row_get(fm, slug, "applied"), af.today())
        expected = (datetime.date.today() + datetime.timedelta(days=af.PIPE_NUDGE_DAYS)).isoformat()
        self.assertEqual(af.row_get(fm, slug, "next_nudge"), expected)
        self.assertIsNone(af.row_get(fm, slug, "shipped"))  # resume never locked

    def test_illegal_transition_refused(self):
        slug = self.save()
        with self.assertRaises(SystemExit):
            self.stage(slug, "interviewing")

    def test_terminal_clears_nudge_and_ghosted_can_return(self):
        slug = self.save()
        self.start(slug)
        self.stage(slug, "applied")
        self.stage(slug, "ghosted")
        self.assertEqual(af.row_get(self.board_fm(), slug, "next_nudge"), "null")
        self.stage(slug, "interviewing")
        self.assertEqual(af.row_get(self.board_fm(), slug, "stage"), "interviewing")


class TestPipelineDoctor(PipeBase):
    def test_clean_pipeline_is_quiet(self):
        slug = self.save()
        write(af.jd_cache_path(slug), "jd")
        self.start(slug)
        issues, notes = af.check_pipeline()
        self.assertEqual(issues, [])
        self.assertEqual(notes, [])

    def test_missing_folder_and_orphan_folder_are_issues(self):
        slug = self.save()
        self.start(slug)
        shutil.rmtree(af.app_dir(slug))  # preparing row, folder gone
        write(os.path.join(af.PIPELINE, "applications", "orphan", "application.md"), "---\nslug: orphan\n---\n")
        issues, _ = af.check_pipeline()
        self.assertTrue(any("no application folder" in i for i in issues))
        self.assertTrue(any("orphan" in i and "no board row" in i for i in issues))

    def test_nudge_and_stale_saved_notes(self):
        slug = self.save()
        self.start(slug)
        self.stage(slug, "applied")
        fm, body = af.load_board()
        fm = af.row_set(fm, slug, "next_nudge", "2026-01-01")
        old = (datetime.date.today() - datetime.timedelta(days=40)).isoformat()
        fm = af.pipe_row_add(fm, "old-save", {"stage": "saved", "saved": old})
        af.write_board(fm, body)
        _, notes = af.check_pipeline()
        self.assertTrue(any("follow-up due" in n for n in notes))
        self.assertTrue(any("old-save" in n and "start it or drop it" in n for n in notes))

    def test_resume_lint_flags_hazards(self):
        slug = self.save()
        write(af.jd_cache_path(slug), "jd")
        self.start(slug)
        adir = af.app_dir(slug)
        write(os.path.join(adir, "resume", "resume-v1.md"),
              "---\nstatus: drafting\nlast_updated: 2026-07-10\nexports: []\n---\n"
              "## My Journey\n- spearheaded X — delivered Y\n- Acme 2022 - 2023\n")
        ap = os.path.join(adir, "application.md")
        afm, abody = af.split_fm(af.read(ap), "application.md")
        afm = af.row_set(afm, "resume", "status", "drafting")
        af.write(ap, af.join_fm(afm, abody))
        issues, notes = af.check_pipeline()
        self.assertTrue(any("em dash" in i for i in issues))
        self.assertTrue(any("non-canonical heading" in i for i in issues))
        self.assertTrue(any("year-only date range" in i for i in issues))
        self.assertTrue(any("AI-tell" in n for n in notes))


class TestLockGates(PipeBase):
    def _prep_application(self, verification=True):
        slug = self.save()
        write(af.jd_cache_path(slug), "jd")
        self.start(slug)
        adir = af.app_dir(slug)
        write(os.path.join(adir, "resume", "resume-v1.md"),
              "---\nstatus: drafting\nlast_updated: 2026-07-10\nexports: [media/resume-v1.pdf]\n---\n"
              "## Work Experience\n- Cut latency 38% (Jan 2022 - Dec 2023)\n## Skills\n- Python\n")
        write(os.path.join(adir, "resume", "media", "resume-v1.pdf"), "pdf")
        jd_map = "---\nstatus: drafting\nlast_updated: 2026-07-10\n---\n## Requirements\n- Python\n"
        if verification:
            jd_map += "## Verification\n- hard requirements mirrored; hazards clean; tells clean\n"
        write(os.path.join(adir, "jd-map.md"), jd_map)
        ap = os.path.join(adir, "application.md")
        afm, abody = af.split_fm(af.read(ap), "application.md")
        afm = af.row_set(afm, "resume", "status", "drafting")
        af.write(ap, af.join_fm(afm, abody))
        return slug, adir

    def test_lock_refused_without_verification(self):
        slug, _ = self._prep_application(verification=False)
        with self.assertRaises(SystemExit):
            quiet(af.cmd_lock, SimpleNamespace(project=slug, deliverable="resume", allow_missing_exports=False))

    def test_lock_refused_without_exports(self):
        slug, adir = self._prep_application()
        os.remove(os.path.join(adir, "resume", "media", "resume-v1.pdf"))
        with self.assertRaises(SystemExit):
            quiet(af.cmd_lock, SimpleNamespace(project=slug, deliverable="resume", allow_missing_exports=False))

    def test_lock_succeeds_and_applied_records_shipped(self):
        slug, adir = self._prep_application()
        quiet(af.cmd_lock, SimpleNamespace(project=slug, deliverable="resume", allow_missing_exports=False))
        afm, _ = af.split_fm(af.read(os.path.join(adir, "application.md")), "application.md")
        self.assertEqual(af.row_get(afm, "resume", "status"), "locked")
        self.stage(slug, "applied")
        self.assertEqual(af.row_get(self.board_fm(), slug, "shipped"), "v1")


if __name__ == "__main__":
    unittest.main()
