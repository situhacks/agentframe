'use strict';
// AgentFrame bridge: a file inbox that lets agents manage Claude Code chat tabs in this VS Code window.
// No dependencies, ASCII only. Fixed actions only: there is no generic "run this command" action.
//
//   <vault>/system/logs/panel-bridge/requests/<id>.json   in  {"action": "...", "args": {...}, "ts": <ms>}
//   <vault>/system/logs/panel-bridge/results/<id>.json    out {"id", "ok", "action", "result" | "error", "ms"}
//   <vault>/system/logs/panel-bridge/bridge.log           one line per request, never the prompt text

const vscode = require('vscode');
const fs = require('fs');
const path = require('path');

const VERSION = '0.1.0';
const MAX_AGE_MS = 2 * 60 * 1000;
const POLL_MS = 2000;
const CLAUDE_VIEW = 'claudeVSCodePanel';
const OPEN_COMMAND = 'claude-vscode.editor.open';
const GROUP_COMMAND = 'claude-vscode.addSessionTabToGroup';

let state = null;

function sleep(ms) {
  return new Promise(function (resolve) { setTimeout(resolve, ms); });
}

// The vault root is the first workspace folder, with a trailing .claude/worktrees/<name> removed.
function vaultRoot() {
  const folders = vscode.workspace.workspaceFolders;
  if (!folders || folders.length === 0) return null;
  let root = folders[0].uri.fsPath;
  root = root.replace(/[\\\/]\.claude[\\\/]worktrees[\\\/][^\\\/]+$/i, '');
  if (!fs.existsSync(path.join(root, 'system', 'af'))) return null;
  if (!fs.existsSync(path.join(root, 'AGENTS.md'))) return null;
  return root;
}

function writeAtomic(file, text) {
  const tmp = file + '.' + process.pid + '.tmp';
  fs.writeFileSync(tmp, text, 'utf8');
  fs.renameSync(tmp, file);
}

function isClaudeTab(tab) {
  const input = tab.input;
  return !!(input && typeof input.viewType === 'string' && input.viewType.indexOf(CLAUDE_VIEW) >= 0);
}

function inputKind(input) {
  if (!input) return 'unknown';
  if (typeof input.viewType === 'string') return 'webview';
  if (input.modified && input.original) return 'diff';
  if (input.uri) return 'text-or-custom';
  return 'other';
}

function listTabs() {
  const out = [];
  const groups = vscode.window.tabGroups.all;
  for (let g = 0; g < groups.length; g++) {
    const tabs = groups[g].tabs;
    for (let t = 0; t < tabs.length; t++) {
      const tab = tabs[t];
      const row = {
        group: g,
        index: t,
        label: tab.label,
        isActive: !!tab.isActive,
        groupActive: !!groups[g].isActive,
        kind: inputKind(tab.input)
      };
      if (tab.input && typeof tab.input.viewType === 'string') row.viewType = tab.input.viewType;
      row.claude = isClaudeTab(tab);
      out.push(row);
    }
  }
  return out;
}

function claudeTabsByLabel(label) {
  const found = [];
  const groups = vscode.window.tabGroups.all;
  for (let g = 0; g < groups.length; g++) {
    const tabs = groups[g].tabs;
    for (let t = 0; t < tabs.length; t++) {
      if (isClaudeTab(tabs[t]) && tabs[t].label === label) found.push(tabs[t]);
    }
  }
  return found;
}

function str(value, name) {
  if (value === undefined || value === null) return undefined;
  if (typeof value !== 'string') throw new Error(name + ' must be a string');
  return value;
}

const ACTIONS = {
  ping: async function () {
    const ext = vscode.extensions.getExtension('anthropic.claude-code');
    return {
      bridgeVersion: VERSION,
      workspaceRoot: state.root,
      vscodeVersion: vscode.version,
      claudeExtensionVersion: ext ? ext.packageJSON.version : null,
      claudeExtensionActive: ext ? ext.isActive : false,
      pid: process.pid
    };
  },

  tabs: async function () {
    return { tabs: listTabs() };
  },

  open: async function (args) {
    const sessionId = str(args.sessionId, 'sessionId');
    const prompt = str(args.prompt, 'prompt');
    let groupId = args.groupId;
    if (groupId !== undefined && groupId !== null && typeof groupId !== 'string') throw new Error('groupId must be a string');
    if (groupId === null) groupId = undefined;
    await vscode.commands.executeCommand(OPEN_COMMAND, sessionId, prompt, undefined, groupId);
    return { opened: true, sessionId: sessionId || null, promptGiven: prompt !== undefined };
  },

  reveal: async function (args) {
    const sessionId = str(args.sessionId, 'sessionId');
    if (!sessionId) throw new Error('sessionId is required');
    await vscode.commands.executeCommand(OPEN_COMMAND, sessionId);
    return { revealed: true, sessionId: sessionId };
  },

  'close-tab': async function (args) {
    const label = str(args.label, 'label');
    if (!label) throw new Error('label is required');
    // Anything that is not a Claude chat tab is never in this list, so it can never be closed here.
    let matches = claudeTabsByLabel(label);
    const total = matches.length;
    if (args.all === false) matches = matches.slice(0, 1);
    let closed = 0;
    for (let i = 0; i < matches.length; i++) {
      const ok = await vscode.window.tabGroups.close(matches[i]);
      if (ok) closed++;
    }
    return { matched: total, closed: closed };
  },

  'group-pick': async function (args) {
    let steps = args.steps;
    if (typeof steps !== 'number' || steps < 0 || steps > 50 || Math.floor(steps) !== steps) throw new Error('steps must be an integer from 0 to 50');
    // Not awaited: the command resolves only after the quick pick is answered.
    const pending = Promise.resolve(vscode.commands.executeCommand(GROUP_COMMAND)).catch(function () { return null; });
    await sleep(700);
    for (let i = 0; i < steps; i++) {
      await vscode.commands.executeCommand('workbench.action.quickOpenSelectNext');
      await sleep(120);
    }
    await vscode.commands.executeCommand('workbench.action.acceptSelectedQuickOpenItem');
    await Promise.race([pending, sleep(1500)]);
    return { picked: true, steps: steps };
  }
};

function logLine(id, action, outcome) {
  try {
    fs.appendFileSync(state.logFile, new Date().toISOString() + ' ' + id + ' ' + action + ' ' + outcome + '\n', 'utf8');
  } catch (e) { /* the log is best effort */ }
}

async function handle(file) {
  const id = path.basename(file, '.json');
  const started = Date.now();
  let action = '?';
  let outcome;
  let body;
  try {
    let text;
    let mtime;
    try {
      mtime = fs.statSync(file).mtimeMs;
      text = fs.readFileSync(file, 'utf8');
    } catch (e) {
      return; // gone already: another pass answered it
    }
    let req;
    try {
      req = JSON.parse(text.charCodeAt(0) === 0xFEFF ? text.slice(1) : text);
    } catch (e) {
      throw new Error('request is not valid JSON');
    }
    action = typeof req.action === 'string' ? req.action : '?';
    const asked = typeof req.ts === 'number' ? Math.min(req.ts, mtime) : mtime;
    if (Date.now() - asked > MAX_AGE_MS) throw new Error('request older than 2 minutes, not run');
    const fn = Object.prototype.hasOwnProperty.call(ACTIONS, action) ? ACTIONS[action] : null;
    if (!fn) throw new Error('unknown action');
    const args = req.args && typeof req.args === 'object' ? req.args : {};
    const result = await fn(args);
    body = { id: id, ok: true, action: action, result: result };
    outcome = 'ok';
  } catch (e) {
    body = { id: id, ok: false, action: action, error: String(e && e.message ? e.message : e) };
    outcome = 'error';
  }
  body.ms = Date.now() - started;
  body.at = new Date().toISOString();
  try {
    writeAtomic(path.join(state.resultsDir, id + '.json'), JSON.stringify(body));
  } catch (e) {
    outcome = 'error(result not written)';
  }
  try { fs.unlinkSync(file); } catch (e) { /* already deleted */ }
  logLine(id, action, outcome);
}

// Requests are handled one at a time, in file-name order, so two actions never overlap.
function scan() {
  if (!state || state.scanning) return;
  state.scanning = true;
  let names = [];
  try {
    names = fs.readdirSync(state.requestsDir).filter(function (n) { return /\.json$/i.test(n); }).sort();
  } catch (e) {
    names = [];
  }
  let chain = Promise.resolve();
  names.forEach(function (n) {
    chain = chain.then(function () { return handle(path.join(state.requestsDir, n)); });
  });
  chain.then(function () { if (state) state.scanning = false; }, function () { if (state) state.scanning = false; });
}

function activate(context) {
  const root = vaultRoot();
  if (!root) return; // not the vault window: do nothing
  const base = path.join(root, 'system', 'logs', 'panel-bridge');
  state = {
    root: root,
    requestsDir: path.join(base, 'requests'),
    resultsDir: path.join(base, 'results'),
    logFile: path.join(base, 'bridge.log'),
    scanning: false
  };
  fs.mkdirSync(state.requestsDir, { recursive: true });
  fs.mkdirSync(state.resultsDir, { recursive: true });
  logLine('-', 'activate', 'ok version ' + VERSION);

  try {
    const watcher = vscode.workspace.createFileSystemWatcher(new vscode.RelativePattern(vscode.Uri.file(state.requestsDir), '*.json'));
    watcher.onDidCreate(scan);
    watcher.onDidChange(scan);
    context.subscriptions.push(watcher);
  } catch (e) { /* the poll below still works */ }
  const timer = setInterval(scan, POLL_MS);
  context.subscriptions.push({ dispose: function () { clearInterval(timer); } });
  scan();
}

function deactivate() {
  state = null;
}

module.exports = { activate: activate, deactivate: deactivate };
