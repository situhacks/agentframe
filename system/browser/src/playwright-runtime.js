import path from "node:path";
import { chromium } from "playwright-core";

const DEFAULT_CANDIDATE_CONTROL_LIMIT = 30;
const OBSERVATION_CONTROL_SELECTOR = [
  "button",
  "[role='button']",
  "[role='textbox']",
  "[role='combobox']",
  "input",
  "textarea",
  "select",
  "a",
  "[contenteditable='true']",
  "[aria-label][role]",
  "[aria-label][tabindex]",
].join(", ");

const EDITABLE_ROLE_PATTERN = /^(textbox|combobox|searchbox)$/i;

export function requireOwnedDebuggingWebSocket(browser) {
  if (!browser.webSocketDebuggerUrl || browser.status === "externally_managed") {
    throw new Error("Playwright requires an owned Work Browser DevTools websocket.");
  }
  return browser.webSocketDebuggerUrl;
}

export async function connectToOwnedWorkBrowser(browser) {
  return chromium.connectOverCDP(requireOwnedDebuggingWebSocket(browser));
}

export function buildCandidateControlSummary(controls, limit = DEFAULT_CANDIDATE_CONTROL_LIMIT) {
  return controls
    .map((control) => ({
      ...control,
      name: String(control.name ?? "").trim().replace(/\s+/g, " "),
      role: String(control.role ?? "").trim().toLowerCase(),
    }))
    .filter((control) => control.name && control.role)
    .map((control, index) => ({ control, index, priority: candidateControlPriority(control) }))
    .sort((left, right) => right.priority - left.priority || left.index - right.index)
    .slice(0, limit)
    .map(({ control }) => control);
}

function candidateControlPriority(control) {
  if (EDITABLE_ROLE_PATTERN.test(control.role)) {
    return 2;
  }
  if (/button|link|option|menuitem/i.test(control.role)) {
    return 1;
  }
  return 0;
}

export function buildObservationSnapshot(input) {
  return {
    bodyTextSnippet: input.bodyTextSnippet ?? null,
    candidateControls: buildCandidateControlSummary(input.candidateControls),
    screenshotPath: input.screenshotPath ?? null,
    title: input.title,
    url: input.url,
  };
}

export async function observePage(page, options = {}) {
  const screenshotPath = options.screenshotPath ? path.resolve(options.screenshotPath) : null;
  if (screenshotPath) {
    await page.screenshot({ path: screenshotPath });
  }

  const candidateControls = await page
    .locator(OBSERVATION_CONTROL_SELECTOR)
    .evaluateAll((elements) =>
      elements.flatMap((element, index) => {
        const htmlElement = element;
        const tagName = htmlElement.tagName.toLowerCase();
        const role = htmlElement.getAttribute("role") || tagName || "control";
        const normalizedRole = role.trim().toLowerCase();
        const interactiveByTag = /^(button|input|textarea|select|a)$/.test(tagName);
        const interactiveByRole = /^(button|textbox|combobox|searchbox|link|tab|menuitem|option)$/.test(
          normalizedRole,
        );
        const interactiveByState = htmlElement.isContentEditable || htmlElement.getAttribute("tabindex") !== null;

        if (!interactiveByTag && !interactiveByRole && !interactiveByState) {
          return [];
        }

        const name =
          htmlElement.getAttribute("aria-label") ||
          htmlElement.getAttribute("title") ||
          htmlElement.innerText ||
          htmlElement.getAttribute("placeholder") ||
          htmlElement.getAttribute("name") ||
          "";

        return {
          name,
          role,
          selector: `${tagName}:nth-of-type(${index + 1})`,
        };
      }),
    )
    .catch(() => []);

  const bodyTextSnippet = await page
    .locator("body")
    .innerText({ timeout: 2_000 })
    .then((text) => text.slice(0, 2_000))
    .catch(() => null);

  return buildObservationSnapshot({
    bodyTextSnippet,
    candidateControls,
    screenshotPath,
    title: await page.title().catch(() => ""),
    url: page.url(),
  });
}
