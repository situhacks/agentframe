import path from "node:path";
import { observePage } from "./playwright-runtime.js";
import { writeRunJson } from "./run-evidence.js";

const DEFAULT_TARGET_TIMEOUT_MS = 1_500;

export function isSelectorOnlyTarget(target) {
  if (!target?.selector?.trim()) {
    return false;
  }

  return ![target.role, target.name, target.text, target.description].some(
    (value) => typeof value === "string" && value.trim().length > 0,
  );
}

export async function resolveBrowserTarget(page, target, options = {}) {
  if (!target || typeof target !== "object") {
    throw new Error("Browser target must be an object with role/name/text/description or selector.");
  }

  const timeout = options.timeoutMs ?? DEFAULT_TARGET_TIMEOUT_MS;
  for (const buildCandidate of buildTargetCandidates(page, target)) {
    const first = buildCandidate().first();
    if (await first.isVisible({ timeout }).catch(() => false)) {
      return first;
    }
  }

  throw new Error("No visible browser target matched the supplied target hints.");
}

export function buildBrowserPrimitives(page, options = {}) {
  const observer = options.observePage ?? observePage;
  const writer = options.writeJson ?? writeRunJson;
  const targetTimeoutMs = options.targetTimeoutMs ?? DEFAULT_TARGET_TIMEOUT_MS;
  let evidenceIndex = 0;

  async function writeEvidence(kind, evidence) {
    if (!options.runDir) {
      return undefined;
    }

    evidenceIndex += 1;
    const filePath = path.join(options.runDir, `${String(evidenceIndex).padStart(3, "0")}-${kind}.json`);
    await writer(filePath, evidence);
    return filePath;
  }

  async function resolve(target) {
    return resolveBrowserTarget(page, target, { timeoutMs: targetTimeoutMs });
  }

  async function observe(input = {}) {
    const snapshot = await observer(page, input.screenshotPath ? { screenshotPath: input.screenshotPath } : {});
    const evidencePath = await writeEvidence("observe", {
      input: primitiveDiagnostic("observe", input),
      observation: snapshot,
    });
    return {
      evidencePath,
      observation: snapshot,
      status: "ok",
    };
  }

  return {
    observe,
    click: async (input) => {
      const target = await resolve(input.target);
      await target.click();
      const evidencePath = await writeEvidence("click", primitiveDiagnostic("click", input));
      return { evidencePath, status: "ok" };
    },
    fill: async (input) => {
      const target = await resolve(input.target);
      await target.fill(input.value);
      const evidencePath = await writeEvidence("fill", primitiveDiagnostic("fill", input));
      return { evidencePath, status: "ok" };
    },
    press: async (input) => {
      if (input.target) {
        const target = await resolve(input.target);
        await target.click();
        await target.focus().catch(() => {});
      }
      await page.keyboard.press(input.key);
      const evidencePath = await writeEvidence("press", primitiveDiagnostic("press", input));
      return { evidencePath, status: "ok" };
    },
    insertText: async (input) => {
      const target = await resolve(input.target);
      await target.click();
      await target.focus().catch(() => {});
      if (input.position === "start") {
        await page.keyboard.press("Control+Home");
      }
      await page.keyboard.insertText(input.trailingBlankLine ? `${input.text.replace(/\r\n/g, "\n").trimEnd()}\n\n` : input.text);
      const evidencePath = await writeEvidence("insertText", primitiveDiagnostic("insertText", input));
      return { evidencePath, status: "ok" };
    },
    screenshot: async (input = {}) => {
      const snapshot = await observer(page, { screenshotPath: input.path });
      const evidencePath = await writeEvidence("screenshot", {
        input: primitiveDiagnostic("screenshot", input),
        observation: snapshot,
      });
      return {
        evidencePath,
        screenshotPath: snapshot.screenshotPath,
        status: "ok",
      };
    },
  };
}

// Backward-compatible alias for the retained lightweight surface.
export const buildBrowserActionPrimitives = buildBrowserPrimitives;

function buildTargetCandidates(page, target) {
  const candidates = [];
  if (target.role && target.name) {
    candidates.push(() => page.getByRole(target.role, { name: target.name }));
  } else if (target.role) {
    candidates.push(() => page.getByRole(target.role));
  }
  if (target.text) {
    candidates.push(() => page.getByText(target.text, { exact: true }));
  }
  if (target.name) {
    candidates.push(() => page.getByText(target.name, { exact: true }));
  }
  if (target.description) {
    candidates.push(() => page.getByText(target.description, { exact: true }));
  }
  if (target.selector) {
    candidates.push(() => page.locator(target.selector));
  }
  return candidates;
}

function primitiveDiagnostic(kind, input = {}) {
  return {
    hasTypedValue: kind === "fill" || kind === "insertText",
    kind,
    target: input.target,
  };
}
