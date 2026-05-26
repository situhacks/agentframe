import base64
import json
import tempfile
import unittest
from unittest import mock
from pathlib import Path

from system.research import gemini_deep_research as gdr


PNG_BYTES = b"\x89PNG\r\n\x1a\nfake"


class GeminiDeepResearchTests(unittest.TestCase):
    def test_extracts_ordered_text_outputs_and_ignores_image_base64(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            source_dir = Path(tmpdir) / "source-material"
            response = {
                "id": "interaction-123",
                "status": "completed",
                "outputs": [
                    {"type": "thought", "summary": [{"text": "thinking"}]},
                    {"type": "text", "text": "# First section"},
                    {
                        "type": "image",
                        "mime_type": "image/png",
                        "data": base64.b64encode(PNG_BYTES).decode("ascii"),
                    },
                    {"type": "text", "text": "## Second section"},
                ],
                "usage": {"total_tokens": 42},
            }

            extracted = gdr.extract_interaction_outputs(response, source_dir)

            self.assertEqual(
                extracted.report_text,
                "# First section\n\n## Second section",
            )
            self.assertEqual(
                extracted.extracted_text_paths,
                ["outputs[1].text", "outputs[3].text"],
            )
            self.assertEqual(
                extracted.generated_media_paths,
                ["source-material/gemini-deep-research-output-2.png"],
            )
            self.assertEqual(
                (source_dir / "gemini-deep-research-output-2.png").read_bytes(),
                PNG_BYTES,
            )

    def test_extracts_docs_style_steps_content_text_fallback(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            response = {
                "id": "interaction-plan",
                "status": "completed",
                "steps": [
                    {"content": [{"text": "draft plan"}]},
                    {"content": [{"text": "final plan"}]},
                ],
            }

            extracted = gdr.extract_interaction_outputs(
                response, Path(tmpdir) / "source-material"
            )

            self.assertEqual(extracted.report_text, "draft plan\n\nfinal plan")
            self.assertEqual(
                extracted.extracted_text_paths,
                ["steps[0].content[0].text", "steps[1].content[0].text"],
            )

    def test_write_artifact_saves_raw_json_and_fails_clear_without_report_text(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            research_dir = Path(tmpdir) / "phase-1-research"
            response = {
                "id": "interaction-image-only",
                "status": "completed",
                "outputs": [
                    {
                        "type": "image",
                        "mime_type": "image/png",
                        "data": base64.b64encode(PNG_BYTES).decode("ascii"),
                    }
                ],
                "usage": {"total_tokens": 7},
            }

            with self.assertRaisesRegex(gdr.ExtractionError, "No report text"):
                gdr.write_research_artifact(
                    response,
                    research_dir,
                    title="Research Artifact - Test",
                    raw_filename="raw.json",
                    metadata_filename="metadata.json",
                    artifact_filename="research-artifact-v1.md",
                )

            self.assertTrue((research_dir / "source-material" / "raw.json").exists())
            self.assertFalse((research_dir / "research-artifact-v1.md").exists())

    def test_write_artifact_frontmatter_points_to_raw_metadata_and_media(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            research_dir = Path(tmpdir) / "phase-1-research"
            response = {
                "id": "interaction-full",
                "status": "completed",
                "outputs": [
                    {"type": "text", "text": "# Report"},
                    {
                        "type": "image",
                        "mime_type": "image/png",
                        "data": base64.b64encode(PNG_BYTES).decode("ascii"),
                    },
                ],
                "usage": {"total_tokens": 100, "total_output_tokens": 10},
            }

            result = gdr.write_research_artifact(
                response,
                research_dir,
                title="Research Artifact - Test",
                raw_filename="raw.json",
                metadata_filename="metadata.json",
                artifact_filename="research-artifact-v1.md",
                last_updated="2026-05-09T18:30:00-07:00",
            )

            artifact_text = result.artifact_path.read_text(encoding="utf-8")
            self.assertIn("research_method: gemini_deep_research_api", artifact_text)
            self.assertIn("source-material/raw.json", artifact_text)
            self.assertIn("source-material/metadata.json", artifact_text)
            self.assertIn("source-material/gemini-deep-research-output-1.png", artifact_text)
            self.assertIn("# Report", artifact_text)

            metadata = json.loads(result.metadata_path.read_text(encoding="utf-8"))
            self.assertEqual(metadata["interaction_id"], "interaction-full")
            self.assertEqual(metadata["usage"]["total_tokens"], 100)
            self.assertEqual(metadata["generated_images"], ["source-material/gemini-deep-research-output-1.png"])
            self.assertEqual(metadata["report_chars"], len("# Report"))


    @mock.patch("urllib.request.urlopen")
    @mock.patch("time.sleep", return_value=None)
    def test_request_json_retries_transient_errors(self, mock_sleep, mock_urlopen) -> None:
        import http.client
        mock_response = mock.MagicMock()
        mock_response.read.return_value = b'{"success": true}'
        mock_response.__enter__.return_value = mock_response
        mock_urlopen.side_effect = [
            http.client.RemoteDisconnected("disconnected"),
            http.client.RemoteDisconnected("disconnected"),
            mock_response,
        ]
        
        result = gdr._request_json(method="GET", url="http://fake", api_key="test")
        self.assertEqual(result, {"success": True})
        self.assertEqual(mock_urlopen.call_count, 3)

    @mock.patch("urllib.request.urlopen")
    @mock.patch("time.sleep", return_value=None)
    def test_request_json_does_not_retry_http_error(self, mock_sleep, mock_urlopen) -> None:
        import urllib.error
        from io import BytesIO
        mock_error = urllib.error.HTTPError("http://fake", 400, "Bad Request", {}, BytesIO(b"error"))
        mock_urlopen.side_effect = mock_error
        
        with self.assertRaisesRegex(RuntimeError, "HTTP 400"):
            gdr._request_json(method="GET", url="http://fake", api_key="test")
        self.assertEqual(mock_urlopen.call_count, 1)

    @mock.patch("system.research.gemini_deep_research.load_gemini_api_key", return_value="fake-key")
    @mock.patch("system.research.gemini_deep_research.start_interaction")
    @mock.patch("system.research.gemini_deep_research.poll_interaction")
    def test_run_deep_research_writes_interaction_id_sidecar(self, mock_poll, mock_start, mock_api_key) -> None:
        mock_start.return_value = {"id": "test-interaction-123"}
        mock_poll.return_value = {
            "id": "test-interaction-123",
            "status": "completed",
            "outputs": [{"type": "text", "text": "report"}],
        }
        
        with tempfile.TemporaryDirectory() as tmpdir:
            research_dir = Path(tmpdir) / "phase-1"
            gdr.run_deep_research(
                prompt="test",
                research_dir=research_dir,
                title="Test",
                previous_interaction_id=None,
                collaborative_planning=False,
                artifact_filename="artifact.md",
                raw_filename="raw.json",
                metadata_filename="meta.json",
            )
            sidecar = research_dir / "source-material" / "gemini-deep-research-interaction-id.txt"
            self.assertTrue(sidecar.exists())
            self.assertEqual(sidecar.read_text(encoding="utf-8"), "test-interaction-123")

    @mock.patch("system.research.gemini_deep_research.load_gemini_api_key", return_value="fake-key")
    @mock.patch("system.research.gemini_deep_research.start_interaction")
    @mock.patch("system.research.gemini_deep_research.poll_interaction")
    def test_run_deep_research_resume_from_id(self, mock_poll, mock_start, mock_api_key) -> None:
        mock_poll.return_value = {
            "id": "resumed-interaction-123",
            "status": "completed",
            "outputs": [{"type": "text", "text": "resumed report"}],
        }
        
        with tempfile.TemporaryDirectory() as tmpdir:
            research_dir = Path(tmpdir) / "phase-1"
            gdr.run_deep_research(
                research_dir=research_dir,
                title="Test",
                previous_interaction_id=None,
                collaborative_planning=False,
                artifact_filename="artifact.md",
                raw_filename="raw.json",
                metadata_filename="meta.json",
                resume_from_id="resumed-interaction-123"
            )
            mock_start.assert_not_called()
            mock_poll.assert_called_once_with(api_key="fake-key", interaction_id="resumed-interaction-123")


if __name__ == "__main__":
    unittest.main()
