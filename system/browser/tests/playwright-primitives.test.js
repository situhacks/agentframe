import assert from "node:assert/strict";
import test from "node:test";
import { buildBrowserPrimitives, isSelectorOnlyTarget, resolveBrowserTarget } from "../src/playwright-primitives.js";

function visibleLocator(name, calls) {
  return {
    first: () => visibleLocator(name, calls),
    focus: async () => {
      calls.push(`${name}:focus`);
    },
    click: async () => {
      calls.push(`${name}:click`);
    },
    fill: async (value) => {
      calls.push(`${name}:fill:${value}`);
    },
    isVisible: async () => true,
  };
}

function makePage() {
  const calls = [];
  const page = {
    getByRole: (role, options = {}) => visibleLocator(`role:${role}:${options.name ?? ""}`, calls),
    getByText: (text) => visibleLocator(`text:${text}`, calls),
    keyboard: {
      insertText: async (text) => calls.push(`keyboard:insertText:${text}`),
      press: async (key) => calls.push(`keyboard:press:${key}`),
    },
    locator: (selector) => visibleLocator(`selector:${selector}`, calls),
  };
  return { calls, page };
}

test("isSelectorOnlyTarget identifies brittle selector-only hints without banning selectors entirely", () => {
  assert.equal(isSelectorOnlyTarget({ selector: "#send" }), true);
  assert.equal(isSelectorOnlyTarget({ name: "Send", selector: "#send" }), false);
});

test("resolveBrowserTarget uses accessible target hints before selector fallback", async () => {
  const { calls, page } = makePage();
  const target = await resolveBrowserTarget(page, { role: "button", name: "New mail", selector: "#new" });
  await target.click();

  assert.deepEqual(calls, ["role:button:New mail:click"]);
});

test("buildBrowserPrimitives exposes generic browser hands without app-specific logic", async () => {
  const { calls, page } = makePage();
  const writes = [];
  const primitives = buildBrowserPrimitives(page, {
    observePage: async () => ({
      bodyTextSnippet: "Inbox",
      candidateControls: [{ name: "New mail", role: "button" }],
      screenshotPath: null,
      title: "Outlook",
      url: "https://outlook.office.com/mail/",
    }),
    runDir: "C:\\runs\\browser-demo",
    writeJson: async (filePath, evidence) => {
      writes.push({ evidence, filePath });
    },
  });

  await primitives.click({ target: { role: "button", name: "New mail" } });
  await primitives.fill({ target: { role: "textbox", name: "Subject" }, value: "Hello" });
  await primitives.press({ key: "Enter" });
  await primitives.insertText({ target: { role: "textbox", name: "Message body" }, text: "Draft body" });
  const observed = await primitives.observe();

  assert.equal(observed.status, "ok");
  assert.equal(writes.length, 5);
  assert.deepEqual(calls, [
    "role:button:New mail:click",
    "role:textbox:Subject:fill:Hello",
    "keyboard:press:Enter",
    "role:textbox:Message body:click",
    "role:textbox:Message body:focus",
    "keyboard:insertText:Draft body",
  ]);
});

test("insertText can insert at the start without replacing existing editor contents", async () => {
  const { calls, page } = makePage();
  const primitives = buildBrowserPrimitives(page);

  await primitives.insertText({
    target: { role: "textbox", name: "Message body" },
    text: "Body above signature",
    position: "start",
    trailingBlankLine: true,
  });

  assert.deepEqual(calls, [
    "role:textbox:Message body:click",
    "role:textbox:Message body:focus",
    "keyboard:press:Control+Home",
    "keyboard:insertText:Body above signature\n\n",
  ]);
  assert.equal(calls.some((call) => call.includes(":fill:")), false);
});
