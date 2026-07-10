// Layout reference for Path B (DOCX) - runnable via the vendored docx skill's docx-js.
// Copy per application, fill with the head resume-v{N}.md content, set OUT, run with node.
// Structure mirrors resources/resume-template.html: experience-first, no Professional Summary,
// Calibri, right-tab dates, round bullets, plain hyphens (never em/en dashes).
const fs = require("fs");
const {
  Document, Packer, Paragraph, TextRun, AlignmentType, BorderStyle,
  TabStopType, TabStopPosition, LevelFormat,
} = require("docx");

const OUT = process.argv[2] || "resume-v1.docx"; // e.g. .../applications/{slug}/resume/media/resume-v1.docx

// US Letter, 0.6" margins
const PAGE = { size: { width: 12240, height: 15840 }, margin: { top: 864, right: 864, bottom: 864, left: 864 } };
const FONT = "Calibri";

function name(t) {
  return new Paragraph({ alignment: AlignmentType.CENTER, spacing: { after: 20 },
    children: [new TextRun({ text: t.toUpperCase(), bold: true, size: 34, font: FONT })] });
}
function contact(t) {
  return new Paragraph({ alignment: AlignmentType.CENTER, spacing: { after: 120 },
    children: [new TextRun({ text: t, size: 19, font: FONT })] });
}
function section(t) {
  return new Paragraph({
    spacing: { before: 160, after: 60 },
    border: { bottom: { style: BorderStyle.SINGLE, size: 6, color: "000000", space: 1 } },
    children: [new TextRun({ text: t.toUpperCase(), bold: true, size: 22, font: FONT })],
  });
}
// org line: bold company (left) + location/dates (right, tab)
function org(left, right) {
  return new Paragraph({
    spacing: { before: 80, after: 0 },
    tabStops: [{ type: TabStopType.RIGHT, position: TabStopPosition.MAX }],
    children: [
      new TextRun({ text: left, bold: true, size: 21, font: FONT }),
      new TextRun({ text: "\t" + right, size: 21, font: FONT }),
    ],
  });
}
function role(left, right) {
  return new Paragraph({
    spacing: { after: 20 },
    tabStops: [{ type: TabStopType.RIGHT, position: TabStopPosition.MAX }],
    children: [
      new TextRun({ text: left, italics: true, size: 20, font: FONT }),
      new TextRun({ text: "\t" + right, size: 20, font: FONT }),
    ],
  });
}
function label(t) {
  return new Paragraph({ spacing: { before: 40, after: 20 }, children: [new TextRun({ text: t, italics: true, size: 20, font: FONT })] });
}
function project(t) {
  return new Paragraph({ numbering: { reference: "b", level: 0 }, spacing: { before: 40, after: 0 },
    children: [new TextRun({ text: t, bold: true, size: 20, font: FONT })] });
}
function bullet(t, level = 0) {
  return new Paragraph({ numbering: { reference: "b", level }, spacing: { after: 0 },
    children: [new TextRun({ text: t, size: 20, font: FONT })] });
}
function line(t) {
  return new Paragraph({ spacing: { after: 0 }, children: [new TextRun({ text: t, size: 20, font: FONT })] });
}

const numbering = {
  config: [{
    reference: "b",
    levels: [
      { level: 0, format: LevelFormat.BULLET, text: "•", alignment: AlignmentType.LEFT,
        style: { paragraphProperties: { indent: { left: 260, hanging: 160 } } } },
      { level: 1, format: LevelFormat.BULLET, text: "•", alignment: AlignmentType.LEFT,
        style: { paragraphProperties: { indent: { left: 560, hanging: 160 } } } },
    ],
  }],
};

const doc = new Document({
  styles: { default: { document: { run: { font: FONT, size: 20 } } } },
  numbering,
  sections: [{ properties: { page: PAGE }, children: [
    name("Jordan Velez"),
    contact("jordan.velez@email.com  |  linkedin.com/in/jordanvelez  |  github.com/jvelez  |  (416) 555-0187  |  Toronto, ON"),

    section("Work Experience"),
    org("Northbridge Consulting", "Toronto, ON"),
    role("Senior Consultant - AI Strategy & Delivery", "Sep 2022 - Present"),
    label("Highlighted Project Experience:"),
    project("AI Enablement Lead on GenAI Rollout - National Retail Bank"),
    bullet("Led adoption across four teams, choosing embedded per-team coaching over central training to fit real workflows, lifting weekly active usage from 30% to 80% in three weeks.", 1),
    bullet("Shipped research agents on Azure AI Foundry that cut a core deliverable's turnaround 80% at higher rated quality.", 1),
    project("Workstream Lead on Loyalty Program Design - Provincial Crown Corporation"),
    bullet("Drove alignment across six business units and 35+ stakeholders so the functional design held through delivery without major rework.", 1),
    org("SwiftCart", "Vancouver, BC"),
    role("Business Analyst - New Verticals", "Aug 2021 - Aug 2022"),
    bullet("Ran weekly partnership reviews with national retail partners, surfacing fixes that drove a 120% average sales uplift."),

    section("Applied AI Projects"),
    project("AgentKit - Self-improving multi-agent marketing harness (Claude Code)"),
    bullet("Built a multi-agent system that runs a campaign end to end and sharpens itself each run, cutting production time from six hours to under two.", 1),

    section("Education"),
    org("University of British Columbia - Sauder School of Business", "Vancouver, BC"),
    role("Bachelor of Commerce - Management Information Systems", "2021"),

    section("Skills & Certifications"),
    line("AI & Agents: Agentic orchestration, Model Context Protocol (MCP), RAG pipelines, LLM evaluation"),
    line("Delivery: Functional PM, change management and adoption, stakeholder alignment"),
    line("Analysis & Tools: SQL, Python, Tableau, Excel financial modelling"),
  ]}],
});

Packer.toBuffer(doc).then(b => { fs.writeFileSync(OUT, b); console.log("wrote " + OUT); });
