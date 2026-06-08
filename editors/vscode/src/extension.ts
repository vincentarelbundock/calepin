import * as path from "path";
import { spawn, type ChildProcessWithoutNullStreams } from "child_process";
import * as crypto from "crypto";
import * as fs from "fs/promises";
import * as os from "os";
import * as vscode from "vscode";

let watchProcess: ChildProcessWithoutNullStreams | null = null;
let watchPanel: vscode.WebviewPanel | null = null;
let watchOutput: vscode.OutputChannel | null = null;
let watchPdfFileWatcher: vscode.FileSystemWatcher | null = null;
let watchPageSyncFileWatcher: vscode.FileSystemWatcher | null = null;
let watchInput: vscode.Uri | null = null;
let watchPageSyncPath: vscode.Uri | null = null;
let watchPageSyncEntries: PageSyncEntry[] = [];
let editorSyncTimer: ReturnType<typeof setTimeout> | null = null;
let previewManager: PreviewManager | null = null;

const CALEPIN_PYTHON_ENV = "CALEPIN_PYTHON";

type PageSyncEntry = {
  label: string;
  file: string;
  line: number;
  page: number;
};

type PythonExtensionApi = {
  settings?: {
    getExecutionDetails?: (
      resource?: vscode.Uri,
    ) => { execCommand?: unknown } | Promise<{ execCommand?: unknown } | undefined>;
  };
  environments?: {
    getActiveEnvironmentPath?: (resource?: vscode.Uri) => unknown | Promise<unknown>;
    resolveEnvironment?: (environment: unknown) => unknown | Promise<unknown>;
  };
};

type LiveDiagnostic = {
  path?: string;
  line?: number;
  column?: number;
  endLine?: number;
  endColumn?: number;
  severity?: string;
  message?: string;
  source?: string;
};

type LiveMessage = {
  type?: string;
  version?: number;
  input?: string;
  output?: string;
  format?: string;
  message?: string;
  diagnostics?: LiveDiagnostic[];
};

class PreviewManager implements vscode.Disposable {
  private readonly sessions = new Map<string, PreviewSession>();
  private readonly diagnostics = vscode.languages.createDiagnosticCollection("calepin");
  private readonly subscriptions: vscode.Disposable[] = [];

  constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly output: vscode.OutputChannel,
  ) {
    this.subscriptions.push(
      this.diagnostics,
      vscode.workspace.onDidChangeTextDocument((event) => {
        const session = this.sessions.get(event.document.uri.toString());
        session?.scheduleSnapshot();
      }),
      vscode.workspace.onDidCloseTextDocument((document) => {
        const session = this.sessions.get(document.uri.toString());
        if (session?.isDisposed()) {
          this.sessions.delete(document.uri.toString());
        }
      }),
    );
  }

  async openPreview(uri?: vscode.Uri): Promise<void> {
    const input = await resolveTypstFile(uri);
    if (!input) return;

    const key = input.toString();
    const existing = this.sessions.get(key);
    if (existing && !existing.isDisposed()) {
      existing.reveal();
      existing.sendSnapshotNow(false);
      return;
    }

    const document = await vscode.workspace.openTextDocument(input);
    const binary = await findBinary(this.context);
    if (!binary) return;
    const outputPath = await livePreviewOutputPath(input);
    const env = await calepinProcessEnv(input);
    const session = new PreviewSession(
      this.context,
      document,
      binary,
      outputPath,
      env,
      this.output,
      this.diagnostics,
      () => this.sessions.delete(key),
    );
    this.sessions.set(key, session);
    await session.start();
  }

  async restartPreview(uri?: vscode.Uri): Promise<void> {
    const input = await resolveTypstFile(uri);
    if (!input) return;
    this.sessions.get(input.toString())?.dispose();
    this.sessions.delete(input.toString());
    await this.openPreview(input);
  }

  async refreshExecution(uri?: vscode.Uri): Promise<void> {
    const input = await resolveTypstFile(uri);
    if (!input) return;
    let session = this.sessions.get(input.toString());
    if (!session || session.isDisposed()) {
      await this.openPreview(input);
      session = this.sessions.get(input.toString());
    }
    session?.sendSnapshotNow(true);
  }

  async stopPreview(uri?: vscode.Uri): Promise<void> {
    const input = uri ?? vscode.window.activeTextEditor?.document.uri;
    if (input) {
      const session = this.sessions.get(input.toString());
      if (session) {
        session.dispose();
        this.sessions.delete(input.toString());
      }
      return;
    }
    this.disposeSessions();
  }

  dispose(): void {
    this.disposeSessions();
    for (const subscription of this.subscriptions) {
      subscription.dispose();
    }
  }

  private disposeSessions(): void {
    for (const session of this.sessions.values()) {
      session.dispose();
    }
    this.sessions.clear();
  }
}

class PreviewSession implements vscode.Disposable {
  private panel: vscode.WebviewPanel | null = null;
  private process: ChildProcessWithoutNullStreams | null = null;
  private stdoutBuffer = "";
  private version = 0;
  private latestSentVersion = 0;
  private latestRenderedVersion = 0;
  private debounceTimer: ReturnType<typeof setTimeout> | null = null;
  private compileInFlight = false;
  private pendingSnapshot = false;
  private pendingExec = false;
  private disposed = false;

  constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly document: vscode.TextDocument,
    private readonly binary: string,
    private readonly outputPath: vscode.Uri,
    private readonly processEnv: NodeJS.ProcessEnv | undefined,
    private readonly output: vscode.OutputChannel,
    private readonly diagnostics: vscode.DiagnosticCollection,
    private readonly onDispose: () => void,
  ) {}

  async start(): Promise<void> {
    this.panel = vscode.window.createWebviewPanel(
      "calepinPreview",
      `Calepin Preview: ${path.basename(this.document.uri.fsPath)}`,
      vscode.ViewColumn.Beside,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [
          vscode.Uri.joinPath(this.context.extensionUri, "media", "pdfjs"),
          vscode.Uri.file(path.dirname(this.outputPath.fsPath)),
        ],
      },
    );
    this.panel.onDidDispose(() => this.dispose());
    this.panel.webview.onDidReceiveMessage((message: { type?: string; message?: string }) => {
      if (message.type === "timing" && message.message) {
        this.output.appendLine(message.message);
      }
    });
    this.panel.webview.html = pdfWatchWebviewHtml(
      this.context,
      this.panel.webview,
      this.outputPath,
    );
    this.startProcess();
    this.sendSnapshotNow(false);
  }

  reveal(): void {
    this.panel?.reveal(vscode.ViewColumn.Beside);
  }

  isDisposed(): boolean {
    return this.disposed;
  }

  scheduleSnapshot(): void {
    if (this.disposed) return;
    if (this.debounceTimer) {
      clearTimeout(this.debounceTimer);
    }
    const cfg = vscode.workspace.getConfiguration("calepin");
    const debounceMs = cfg.get<number>("preview.debounceMs", 250);
    this.debounceTimer = setTimeout(() => {
      this.debounceTimer = null;
      this.sendSnapshotNow(false);
    }, debounceMs);
  }

  sendSnapshotNow(exec = false): void {
    if (this.disposed || !this.process?.stdin.writable) return;
    if (this.compileInFlight) {
      this.pendingSnapshot = true;
      this.pendingExec = this.pendingExec || exec;
      return;
    }
    this.version += 1;
    this.latestSentVersion = this.version;
    const message = {
      type: "snapshot",
      version: this.version,
      path: this.document.uri.fsPath,
      text: this.document.getText(),
      exec,
    };
    this.compileInFlight = true;
    this.process.stdin.write(`${JSON.stringify(message)}\n`);
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    if (this.debounceTimer) {
      clearTimeout(this.debounceTimer);
      this.debounceTimer = null;
    }
    this.diagnostics.delete(this.document.uri);
    if (this.process) {
      const proc = this.process;
      this.process = null;
      if (proc.stdin.writable) {
        proc.stdin.write(`${JSON.stringify({ type: "shutdown" })}\n`);
      }
      proc.kill();
    }
    const panel = this.panel;
    this.panel = null;
    panel?.dispose();
    this.onDispose();
  }

  private startProcess(): void {
    const cfg = vscode.workspace.getConfiguration("calepin");
    const format = cfg.get<string>("preview.format", "pdf");
    const noExec = cfg.get<boolean>("preview.noExec", true);
    const extraTypstArgs = cfg
      .get<string[]>("preview.extraTypstArgs", [])
      .filter((arg) => arg.length > 0);
    const args = [
      "watch",
      this.document.uri.fsPath,
      this.outputPath.fsPath,
      "--editor-live",
      "--format",
      format,
      "--diagnostics",
      "json",
      "--quiet",
    ];
    if (noExec) {
      args.push("--no-exec");
    }
    if (extraTypstArgs.length > 0) {
      args.push("--", ...extraTypstArgs);
    }

    const cwd = workspaceCwd(this.document.uri);
    const spawnOptions = this.processEnv
      ? { cwd, env: { ...process.env, ...this.processEnv } }
      : { cwd };
    this.output.appendLine(`$ ${formatCommand(this.binary, args, this.processEnv)}`);
    const proc = spawn(this.binary, args, spawnOptions);
    this.process = proc;

    proc.stdout.on("data", (chunk: Buffer) => this.handleStdout(chunk));
    proc.stderr.on("data", (chunk: Buffer) => this.output.append(chunk.toString()));
    proc.on("error", (error) => {
      this.output.appendLine(`Failed to start Calepin Preview: ${error.message}`);
      this.showError(`Failed to start Calepin Preview: ${error.message}`);
    });
    proc.on("exit", (code) => {
      if (this.disposed || this.process !== proc) return;
      this.process = null;
      const message = `Calepin live preview exited with code ${code ?? "null"}.`;
      this.output.appendLine(message);
      this.showError(`${message} Run "Calepin: Restart Preview" to start it again.`);
    });
  }

  private handleStdout(chunk: Buffer): void {
    this.stdoutBuffer += chunk.toString();
    let newline = this.stdoutBuffer.indexOf("\n");
    while (newline >= 0) {
      const line = this.stdoutBuffer.slice(0, newline).trim();
      this.stdoutBuffer = this.stdoutBuffer.slice(newline + 1);
      if (line.length > 0) {
        this.handleJsonLine(line);
      }
      newline = this.stdoutBuffer.indexOf("\n");
    }
  }

  private handleJsonLine(line: string): void {
    let message: LiveMessage;
    try {
      message = JSON.parse(line) as LiveMessage;
    } catch (error) {
      this.output.appendLine(`Failed to parse Calepin preview JSON: ${String(error)}`);
      this.output.appendLine(line);
      return;
    }

    if (message.type === "compiled") {
      this.handleCompiled(message);
    } else if (message.type === "error") {
      this.handleError(message);
    } else if (message.type === "compiling") {
      this.panel?.webview.postMessage({ type: "showCompiling", version: message.version });
    }
  }

  private handleCompiled(message: LiveMessage): void {
    const version = typeof message.version === "number" ? message.version : 0;
    try {
      if (version < this.latestSentVersion || version < this.latestRenderedVersion) return;
      this.latestRenderedVersion = version;
      this.applyDiagnostics(message.diagnostics ?? []);
      const preserveScroll = vscode.workspace
        .getConfiguration("calepin")
        .get<boolean>("preview.preserveScroll", true);
      void this.panel?.webview.postMessage({
        type: "reload",
        version: String(version),
        preserveScroll,
      });
    } finally {
      this.finishCompile();
    }
  }

  private handleError(message: LiveMessage): void {
    const version = typeof message.version === "number" ? message.version : this.latestSentVersion;
    try {
      if (version < this.latestSentVersion) return;
      this.applyDiagnostics(message.diagnostics ?? []);
      this.showError(message.message ?? "Compilation failed", message.diagnostics ?? []);
    } finally {
      this.finishCompile();
    }
  }

  private finishCompile(): void {
    this.compileInFlight = false;
    if (!this.pendingSnapshot || this.disposed) return;
    const exec = this.pendingExec;
    this.pendingSnapshot = false;
    this.pendingExec = false;
    this.sendSnapshotNow(exec);
  }

  private applyDiagnostics(items: LiveDiagnostic[]): void {
    const grouped = new Map<string, vscode.Diagnostic[]>();
    for (const item of items) {
      const target = item.path ? vscode.Uri.file(item.path) : this.document.uri;
      const list = grouped.get(target.toString()) ?? [];
      list.push(toVscodeDiagnostic(item));
      grouped.set(target.toString(), list);
    }
    if (grouped.size === 0) {
      this.diagnostics.set(this.document.uri, []);
      return;
    }
    for (const [key, value] of grouped) {
      this.diagnostics.set(vscode.Uri.parse(key), value);
    }
  }

  private showError(message: string, diagnostics: LiveDiagnostic[] = []): void {
    void this.panel?.webview.postMessage({
      type: "showError",
      message,
      diagnostics,
    });
  }
}

function toVscodeDiagnostic(item: LiveDiagnostic): vscode.Diagnostic {
  const line = positiveProtocolInteger(item.line) ?? 1;
  const column = positiveProtocolInteger(item.column) ?? 1;
  const endLine = positiveProtocolInteger(item.endLine) ?? line;
  const endColumn = positiveProtocolInteger(item.endColumn) ?? column + 1;
  const startLine = Math.max(0, line - 1);
  const startCol = Math.max(0, column - 1);
  const range = new vscode.Range(
    startLine,
    startCol,
    Math.max(startLine, endLine - 1),
    Math.max(startCol + 1, endColumn - 1),
  );
  const diagnostic = new vscode.Diagnostic(
    range,
    item.message ?? "Calepin preview error",
    protocolSeverity(item.severity),
  );
  diagnostic.source = item.source ?? "calepin";
  return diagnostic;
}

function positiveProtocolInteger(value: unknown): number | null {
  return typeof value === "number" && Number.isInteger(value) && value > 0 ? value : null;
}

function protocolSeverity(value: unknown): vscode.DiagnosticSeverity {
  switch (value) {
    case "warning":
      return vscode.DiagnosticSeverity.Warning;
    case "info":
      return vscode.DiagnosticSeverity.Information;
    case "hint":
      return vscode.DiagnosticSeverity.Hint;
    case "error":
    default:
      return vscode.DiagnosticSeverity.Error;
  }
}

async function livePreviewOutputPath(input: vscode.Uri): Promise<vscode.Uri> {
  const workspace = vscode.workspace.getWorkspaceFolder(input)?.uri.fsPath ?? path.dirname(input.fsPath);
  const hash = crypto
    .createHash("sha256")
    .update(`${workspace}\0${input.fsPath}`)
    .digest("hex")
    .slice(0, 16);
  const dir = path.join(os.tmpdir(), "calepin-vscode", hash);
  await fs.mkdir(dir, { recursive: true });
  return vscode.Uri.file(path.join(dir, "preview.pdf"));
}

export function activate(context: vscode.ExtensionContext): void {
  watchOutput = vscode.window.createOutputChannel("Calepin Typst Watch");
  const previewOutput = vscode.window.createOutputChannel("Calepin Preview");
  previewManager = new PreviewManager(context, previewOutput);
  context.subscriptions.push(watchOutput);
  context.subscriptions.push(previewOutput, previewManager);
  context.subscriptions.push(
    vscode.commands.registerCommand("calepin.watch", (uri?: vscode.Uri) =>
      runWatch(context, uri),
    ),
    vscode.commands.registerCommand("calepin.compile", (uri?: vscode.Uri) =>
      runCompile(context, uri),
    ),
    vscode.commands.registerCommand("calepin.stop", () => runStop(context)),
    vscode.commands.registerCommand("calepin.new", () => runNew(context)),
    vscode.commands.registerCommand("calepin.openPreview", (uri?: vscode.Uri) =>
      previewManager?.openPreview(uri),
    ),
    vscode.commands.registerCommand("calepin.restartPreview", (uri?: vscode.Uri) =>
      previewManager?.restartPreview(uri),
    ),
    vscode.commands.registerCommand("calepin.stopPreview", (uri?: vscode.Uri) =>
      previewManager?.stopPreview(uri),
    ),
    vscode.commands.registerCommand("calepin.refreshExecution", (uri?: vscode.Uri) =>
      previewManager?.refreshExecution(uri),
    ),
    vscode.window.onDidChangeTextEditorSelection((event) =>
      scheduleEditorToPdfSync(event.textEditor),
    ),
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      if (editor) scheduleEditorToPdfSync(editor);
    }),
  );
}

export function deactivate(): void {
  stopWatch();
  previewManager?.dispose();
}

async function runWatch(
  context: vscode.ExtensionContext,
  uri?: vscode.Uri,
): Promise<void> {
  const input = await resolveTypstFile(uri);
  if (!input) return;

  const binary = await findBinary(context);
  if (!binary) return;

  const cfg = vscode.workspace.getConfiguration("calepin");
  const format = cfg.get<string>("watchFormat", "pdf");
  const typstArgs = cfg.get<string[]>("watchTypstArgs", []).filter((arg) => arg.length > 0);
  const env = await calepinProcessEnv(input);
  startWatchProcess(context, binary, input, format, typstArgs, env);
}

async function runCompile(
  context: vscode.ExtensionContext,
  uri?: vscode.Uri,
): Promise<void> {
  const input = await resolveTypstFile(uri);
  if (!input) return;

  const binary = await findBinary(context);
  if (!binary) return;

  const format = await pickCompileFormat();
  if (!format) return;

  const output = await pickCompileOutput(input, format);
  if (!output) return;

  runInTerminal(
    "Calepin Typst Compile",
    binary,
    ["compile", input.fsPath, output.fsPath, "--format", format],
    workspaceCwd(input),
    await calepinProcessEnv(input),
  );
}

async function pickCompileFormat(): Promise<"pdf" | "html" | null> {
  const picked = await vscode.window.showQuickPick(
    [
      { label: "PDF", description: "pdf", format: "pdf" as const },
      { label: "HTML", description: "html", format: "html" as const },
    ],
    {
      title: "Calepin Typst Compile Format",
      placeHolder: "Select output format",
    },
  );
  return picked?.format ?? null;
}

async function pickCompileOutput(
  input: vscode.Uri,
  format: "pdf" | "html",
): Promise<vscode.Uri | null> {
  const defaultUri = vscode.Uri.file(defaultOutputPath(input.fsPath, format));
  const target = await vscode.window.showSaveDialog({
    defaultUri,
    filters: {
      [format === "pdf" ? "PDF files" : "HTML files"]: [format],
    },
    saveLabel: "Compile Calepin document",
    title: "Save Calepin Output",
  });
  return target ?? null;
}

async function runNew(context: vscode.ExtensionContext): Promise<void> {
  const binary = await findBinary(context);
  if (!binary) return;

  const folder = vscode.workspace.workspaceFolders?.[0];
  const defaultUri = folder
    ? vscode.Uri.joinPath(folder.uri, "paper.typ")
    : undefined;
  const target = await vscode.window.showSaveDialog({
    defaultUri,
    filters: { "Typst files": ["typ"] },
    saveLabel: "Create Calepin document",
  });
  if (!target) return;

  runInTerminal("Calepin Typst New", binary, ["new", target.fsPath], workspaceCwd(target));
}

async function runStop(context: vscode.ExtensionContext): Promise<void> {
  stopWatch();

  const binary = await findBinary(context);
  if (!binary) return;

  const activeInput = vscode.window.activeTextEditor?.document;
  const stopInput =
    activeInput?.uri.scheme === "file" && activeInput.uri.fsPath.endsWith(".typ")
      ? activeInput.uri
      : watchInput;
  if (stopInput) {
    runInTerminal(
      "Calepin Typst Stop",
      binary,
      ["stop", stopInput.fsPath],
      workspaceCwd(stopInput),
      await calepinProcessEnv(stopInput),
    );
    return;
  }

  const workspaceFolders = vscode.workspace.workspaceFolders;
  if (!workspaceFolders || workspaceFolders.length === 0) {
    runInTerminal("Calepin Typst Stop", binary, ["stop"], process.cwd());
    return;
  }

  for (const folder of workspaceFolders) {
    runInTerminal(
      "Calepin Typst Stop",
      binary,
      ["stop"],
      folder.uri.fsPath,
      await calepinProcessEnv(folder.uri),
    );
  }
}

async function resolveTypstFile(uri?: vscode.Uri): Promise<vscode.Uri | null> {
  if (uri?.fsPath.endsWith(".typ")) return uri;

  const active = vscode.window.activeTextEditor?.document.uri;
  if (active?.fsPath.endsWith(".typ")) return active;

  const picked = await vscode.window.showOpenDialog({
    canSelectFiles: true,
    canSelectFolders: false,
    canSelectMany: false,
    filters: { "Typst files": ["typ"] },
    openLabel: "Select Typst file",
  });
  return picked?.[0] ?? null;
}

async function findBinary(
  context: vscode.ExtensionContext,
): Promise<string | null> {
  const cfg = vscode.workspace.getConfiguration("calepin");
  const explicit = cfg
    .get<string>("binaryPath", "")
    .trim()
    .replace(/^["']+|["']+$/g, "")
    .trim();
  if (explicit) return explicit;

  const exe = process.platform === "win32" ? "calepin.exe" : "calepin";
  const bundled = vscode.Uri.joinPath(context.extensionUri, "bin", exe);
  try {
    await vscode.workspace.fs.stat(bundled);
    if (process.platform !== "win32") {
      const fs = await import("fs");
      fs.chmodSync(bundled.fsPath, 0o755);
    }
    return bundled.fsPath;
  } catch {
    // Fall back to PATH below.
  }

  return exe;
}

function runInTerminal(
  name: string,
  binary: string,
  args: string[],
  cwd: string,
  env?: NodeJS.ProcessEnv,
): void {
  const terminal = vscode.window.createTerminal({ name, cwd, env });
  terminal.show();
  terminal.sendText([binary, ...args].map(shellQuote).join(" "));
}

function startWatchProcess(
  context: vscode.ExtensionContext,
  binary: string,
  input: vscode.Uri,
  format: string,
  typstArgs: string[],
  processEnv?: NodeJS.ProcessEnv,
): void {
  stopWatch();
  watchInput = input;

  const cwd = workspaceCwd(input);
  const output = vscode.Uri.file(defaultOutputPath(input.fsPath, format));
  const args = ["watch", input.fsPath, output.fsPath, "--format", format];
  if (typstArgs.length > 0) {
    args.push("--", ...typstArgs);
  }
  watchOutput?.clear();
  watchOutput?.appendLine(`$ ${formatCommand(binary, args, processEnv)}`);
  watchOutput?.show(true);

  const spawnOptions = processEnv
    ? { cwd, env: { ...process.env, ...processEnv } }
    : { cwd };
  const proc = spawn(binary, args, spawnOptions);
  watchProcess = proc;

  let opened = false;
  if (format === "pdf") {
    opened = true;
    showPdfWatchWebview(context, input, output);
    startPdfOutputWatcher(output);
    startPageSyncWatcher(input);
    void reloadPageSyncMap();
  }

  const handleChunk = (chunk: Buffer): void => {
    const text = chunk.toString();
    watchOutput?.append(text);
    const match = text.match(/(?:watching|serving) at\s+(https?:\/\/[^\s]+)/i);
    if (match && !opened && format === "html") {
      opened = true;
      showWatchWebview(match[1], input);
    }
    if (format === "pdf" && opened && /\bcompiled successfully\b/.test(text)) {
      reloadPdfPreview();
      void reloadPageSyncMap();
    }
  };

  proc.stdout.on("data", handleChunk);
  proc.stderr.on("data", handleChunk);
  proc.on("error", (error) => {
    watchOutput?.appendLine(`\nFailed to start Calepin Typst Watch: ${error.message}`);
    vscode.window.showErrorMessage(
      "Failed to start Calepin Typst Watch. See the Calepin Typst Watch output for details.",
    );
  });
  proc.on("exit", (code) => {
    if (watchProcess !== proc) return;
    watchProcess = null;
    watchOutput?.appendLine(`\nCalepin Typst Watch exited with code ${code ?? "null"}.`);
  });
}

function startPdfOutputWatcher(output: vscode.Uri): void {
  watchPdfFileWatcher?.dispose();
  const pattern = new vscode.RelativePattern(
    vscode.Uri.file(path.dirname(output.fsPath)),
    path.basename(output.fsPath),
  );
  const watcher = vscode.workspace.createFileSystemWatcher(pattern);
  watcher.onDidCreate(reloadPdfPreview);
  watcher.onDidChange(reloadPdfPreview);
  watchPdfFileWatcher = watcher;
}

function startPageSyncWatcher(input: vscode.Uri): void {
  watchPageSyncFileWatcher?.dispose();
  watchPageSyncPath = vscode.Uri.file(defaultPageSyncPath(input.fsPath));
  const pattern = new vscode.RelativePattern(
    vscode.Uri.file(path.dirname(watchPageSyncPath.fsPath)),
    path.basename(watchPageSyncPath.fsPath),
  );
  const watcher = vscode.workspace.createFileSystemWatcher(pattern);
  watcher.onDidCreate(() => void reloadPageSyncMap());
  watcher.onDidChange(() => void reloadPageSyncMap());
  watcher.onDidDelete(() => {
    watchPageSyncEntries = [];
  });
  watchPageSyncFileWatcher = watcher;
}

async function reloadPageSyncMap(): Promise<void> {
  if (!watchPageSyncPath) return;
  try {
    const bytes = await vscode.workspace.fs.readFile(watchPageSyncPath);
    const document = JSON.parse(Buffer.from(bytes).toString("utf8")) as unknown;
    watchPageSyncEntries = parsePageSyncEntries(document);
  } catch {
    watchPageSyncEntries = [];
  }
}

function parsePageSyncEntries(document: unknown): PageSyncEntry[] {
  if (!isRecord(document) || !Array.isArray(document.entries)) return [];
  return document.entries
    .map((entry) => {
      if (!isRecord(entry)) return null;
      const label = typeof entry.label === "string" ? entry.label : "";
      const file = typeof entry.file === "string" ? entry.file : "";
      const line = positiveInteger(entry.line);
      const page = positiveInteger(entry.page);
      if (!label || !file || !line || !page) return null;
      return { label, file, line, page };
    })
    .filter((entry): entry is PageSyncEntry => entry !== null)
    .sort((left, right) =>
      left.file.localeCompare(right.file) || left.line - right.line || left.page - right.page,
    );
}

function positiveInteger(value: unknown): number | null {
  return typeof value === "number" && Number.isInteger(value) && value > 0 ? value : null;
}

function reloadPdfPreview(): void {
  void watchPanel?.webview.postMessage({
    type: "reload",
    version: Date.now().toString(),
  });
}

function scheduleEditorToPdfSync(editor: vscode.TextEditor): void {
  if (!watchPanel || !watchInput || watchPageSyncEntries.length === 0) return;
  if (editor.document.uri.scheme !== "file") return;
  if (!hasPageSyncEntriesForFile(editor.document.uri)) return;

  if (editorSyncTimer) {
    clearTimeout(editorSyncTimer);
  }
  editorSyncTimer = setTimeout(() => {
    editorSyncTimer = null;
    syncEditorToPdf(editor);
  }, 100);
}

function syncEditorToPdf(editor: vscode.TextEditor): void {
  const entry = nearestEntryForSourceLine(editor.document.uri, editor.selection.active.line + 1);
  if (!entry) return;
  void watchPanel?.webview.postMessage({
    type: "syncToPdf",
    page: entry.page,
  });
}

function hasPageSyncEntriesForFile(uri: vscode.Uri): boolean {
  return watchPageSyncEntries.some((entry) => sameFsPath(uriForPageSyncEntry(entry).fsPath, uri.fsPath));
}

function nearestEntryForSourceLine(uri: vscode.Uri, line: number): PageSyncEntry | null {
  const entries = watchPageSyncEntries.filter((entry) =>
    sameFsPath(uriForPageSyncEntry(entry).fsPath, uri.fsPath),
  );
  if (entries.length === 0) return null;

  let best = entries[0];
  for (const entry of entries) {
    if (entry.line > line) break;
    best = entry;
  }
  return best;
}

function uriForPageSyncEntry(entry: PageSyncEntry): vscode.Uri {
  if (path.isAbsolute(entry.file)) {
    return vscode.Uri.file(entry.file);
  }
  const root = watchInput ? path.dirname(watchInput.fsPath) : workspaceCwd(vscode.Uri.file(entry.file));
  return vscode.Uri.file(path.resolve(root, entry.file));
}

function sameFsPath(left: string, right: string): boolean {
  const normalizedLeft = path.normalize(left);
  const normalizedRight = path.normalize(right);
  return process.platform === "win32"
    ? normalizedLeft.toLowerCase() === normalizedRight.toLowerCase()
    : normalizedLeft === normalizedRight;
}

function showPdfWatchWebview(
  context: vscode.ExtensionContext,
  input: vscode.Uri,
  output: vscode.Uri,
): void {
  const mediaRoot = vscode.Uri.joinPath(context.extensionUri, "media", "pdfjs");
  if (!watchPanel) {
    watchPanel = vscode.window.createWebviewPanel(
      "calepinWatch",
      `Calepin Typst Watch: ${path.basename(input.fsPath)}`,
      vscode.ViewColumn.Beside,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [
          mediaRoot,
          vscode.Uri.file(path.dirname(output.fsPath)),
        ],
      },
    );
    watchPanel.onDidDispose(() => {
      watchPanel = null;
      stopWatch();
    });
    watchPanel.webview.onDidReceiveMessage((message: { type?: string; message?: string }) => {
      if (message.type === "timing" && message.message) {
        watchOutput?.appendLine(message.message);
      }
    });
  } else {
    watchPanel.title = `Calepin Typst Watch: ${path.basename(input.fsPath)}`;
    watchPanel.reveal(vscode.ViewColumn.Beside);
  }

  watchPanel.webview.html = pdfWatchWebviewHtml(context, watchPanel.webview, output);
}

function showWatchWebview(url: string, input: vscode.Uri): void {
  if (!watchPanel) {
    watchPanel = vscode.window.createWebviewPanel(
      "calepinWatch",
      `Calepin Typst Watch: ${path.basename(input.fsPath)}`,
      vscode.ViewColumn.Beside,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
      },
    );
    watchPanel.onDidDispose(() => {
      watchPanel = null;
      stopWatch();
    });
  } else {
    watchPanel.title = `Calepin Typst Watch: ${path.basename(input.fsPath)}`;
    watchPanel.reveal(vscode.ViewColumn.Beside);
  }

  watchPanel.webview.html = watchWebviewHtml(watchPanel.webview, url);
}

function watchWebviewHtml(webview: vscode.Webview, url: string): string {
  const csp = [
    `default-src 'none'`,
    `frame-src http://localhost:* http://127.0.0.1:*`,
    `img-src http://localhost:* http://127.0.0.1:* data:`,
    `style-src ${webview.cspSource} 'unsafe-inline'`,
  ].join("; ");
  const previewUrl = escapeHtml(url);

  return `<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <meta http-equiv="Content-Security-Policy" content="${csp}">
  <style>
    :root {
      color-scheme: light dark;
    }
    html, body {
      margin: 0;
      padding: 0;
      height: 100%;
      background: var(--vscode-editor-background);
      color: var(--vscode-editor-foreground);
      font-family: var(--vscode-font-family);
    }
    iframe {
      display: block;
      width: 100%;
      height: 100%;
      border: 0;
    }
  </style>
</head>
<body>
  <iframe title="Calepin preview" src="${previewUrl}"></iframe>
</body>
</html>`;
}

function pdfWatchWebviewHtml(
  context: vscode.ExtensionContext,
  webview: vscode.Webview,
  output: vscode.Uri,
): string {
  const nonce = randomNonce();
  const mediaRoot = vscode.Uri.joinPath(context.extensionUri, "media", "pdfjs");
  const pdfUri = webview.asWebviewUri(output);
  const pdfJsUri = webview.asWebviewUri(vscode.Uri.joinPath(mediaRoot, "build", "pdf.min.mjs"));
  const workerUri = webview.asWebviewUri(
    vscode.Uri.joinPath(mediaRoot, "build", "pdf.worker.min.mjs"),
  );
  const viewerJsUri = webview.asWebviewUri(
    vscode.Uri.joinPath(mediaRoot, "web", "pdf_viewer.mjs"),
  );
  const viewerCssUri = webview.asWebviewUri(
    vscode.Uri.joinPath(mediaRoot, "web", "pdf_viewer.css"),
  );
  const csp = [
    `default-src 'none'`,
    `connect-src ${webview.cspSource}`,
    `img-src data: blob: ${webview.cspSource}`,
    `font-src ${webview.cspSource}`,
    `script-src 'nonce-${nonce}' ${webview.cspSource}`,
    `style-src ${webview.cspSource} 'unsafe-inline'`,
    `worker-src blob: ${webview.cspSource}`,
  ].join("; ");

  return `<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <meta http-equiv="Content-Security-Policy" content="${csp}">
  <link rel="stylesheet" href="${viewerCssUri}">
  <style>
    :root {
      color-scheme: light dark;
    }
    html, body {
      margin: 0;
      padding: 0;
      height: 100%;
      overflow: hidden;
      background: var(--vscode-editor-background);
      color: var(--vscode-editor-foreground);
      font-family: var(--vscode-font-family);
    }
    #toolbar {
      box-sizing: border-box;
      height: 36px;
      display: flex;
      align-items: center;
      gap: 6px;
      padding: 4px 8px;
      border-bottom: 1px solid var(--vscode-panel-border);
      background: var(--vscode-editor-background);
      font-size: 12px;
    }
    #status {
      flex: 1;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      color: var(--vscode-descriptionForeground);
    }
    button, input, select {
      height: 24px;
      border: 1px solid var(--vscode-input-border, var(--vscode-panel-border));
      color: var(--vscode-input-foreground);
      background: var(--vscode-input-background);
      font: inherit;
    }
    button {
      min-width: 28px;
      padding: 0 8px;
      color: var(--vscode-button-foreground);
      background: var(--vscode-button-background);
      border-color: var(--vscode-button-border, transparent);
      cursor: pointer;
    }
    button:hover {
      background: var(--vscode-button-hoverBackground);
    }
    #auto-scroll-label {
      height: 24px;
      display: flex;
      align-items: center;
      gap: 4px;
      white-space: nowrap;
      color: var(--vscode-editor-foreground);
    }
    #auto-scroll {
      width: 14px;
      height: 14px;
      margin: 0;
    }
    #page-number {
      width: 48px;
      text-align: right;
    }
    #viewerContainer {
      position: absolute;
      inset: 36px 0 0 0;
      overflow: auto;
    }
    .pdfViewer .page {
      margin: 12px auto;
      border: 0;
      box-shadow: 0 1px 8px rgba(0, 0, 0, 0.35);
    }
  </style>
</head>
<body>
  <div id="toolbar">
    <button id="prev" type="button">Prev</button>
    <input id="page-number" type="number" min="1" value="1">
    <span id="page-count">/ -</span>
    <button id="next" type="button">Next</button>
    <button id="zoom-out" type="button">-</button>
    <select id="zoom">
      <option value="auto">Auto</option>
      <option value="page-width">Width</option>
      <option value="page-fit">Page</option>
      <option value="0.75">75%</option>
      <option value="1">100%</option>
      <option value="1.25">125%</option>
      <option value="1.5">150%</option>
      <option value="2">200%</option>
      <option value="3">300%</option>
    </select>
    <button id="zoom-in" type="button">+</button>
    <label id="auto-scroll-label">
      <input id="auto-scroll" type="checkbox" checked>
      Auto-scroll
    </label>
    <span id="status">Loading Calepin preview...</span>
  </div>
  <div id="viewerContainer">
    <div id="viewer" class="pdfViewer"></div>
  </div>
  <script nonce="${nonce}" type="module">
    import * as pdfjsLib from "${pdfJsUri}";
    import * as pdfjsWorker from "${workerUri}";
    import { EventBus, PDFLinkService, PDFViewer } from "${viewerJsUri}";

    const vscode = acquireVsCodeApi();
    globalThis.pdfjsWorker = pdfjsWorker;

    const pdfSource = "${escapeJavaScript(pdfUri.toString())}";
    const container = document.getElementById("viewerContainer");
    const status = document.getElementById("status");
    const pageNumber = document.getElementById("page-number");
    const pageCount = document.getElementById("page-count");
    const zoom = document.getElementById("zoom");
    const autoScroll = document.getElementById("auto-scroll");
    const eventBus = new EventBus();
    const linkService = new PDFLinkService({ eventBus });
    const pdfViewer = new PDFViewer({
      container,
      eventBus,
      linkService,
      textLayerMode: 0,
      annotationMode: pdfjsLib.AnnotationMode.DISABLE,
    });
    linkService.setViewer(pdfViewer);

    let lastVersion = null;
    let loading = false;
    let queuedVersion = null;
    const startedAt = performance.now();
    const savedState = vscode.getState() || {};
    autoScroll.checked = savedState.autoScroll !== false;

    function secondsSince(start) {
      return ((performance.now() - start) / 1000).toFixed(1) + "s";
    }
    function logTiming(message) {
      vscode.postMessage({ type: "timing", message: "[preview] " + message });
    }
    function setPdfPage(page) {
      const bounded = Math.min(Math.max(1, page), pdfViewer.pagesCount || page || 1);
      pdfViewer.currentPageNumber = bounded;
    }

    document.getElementById("prev").addEventListener("click", () => {
      setPdfPage(Math.max(1, pdfViewer.currentPageNumber - 1));
    });
    document.getElementById("next").addEventListener("click", () => {
      setPdfPage(Math.min(pdfViewer.pagesCount, pdfViewer.currentPageNumber + 1));
    });
    document.getElementById("zoom-out").addEventListener("click", () => {
      pdfViewer.currentScale = Math.max(0.1, pdfViewer.currentScale / 1.2);
      syncZoom();
    });
    document.getElementById("zoom-in").addEventListener("click", () => {
      pdfViewer.currentScale = Math.min(10, pdfViewer.currentScale * 1.2);
      syncZoom();
    });
    pageNumber.addEventListener("change", () => {
      const page = Number(pageNumber.value);
      if (Number.isFinite(page)) {
        setPdfPage(page);
      }
    });
    zoom.addEventListener("change", () => {
      pdfViewer.currentScaleValue = zoom.value;
    });
    autoScroll.addEventListener("change", () => {
      vscode.setState(snapshotState());
    });
    eventBus.on("pagechanging", (event) => {
      pageNumber.value = String(event.pageNumber);
    });
    eventBus.on("scalechanging", syncZoom);
    eventBus.on("pagesloaded", () => {
      pageCount.textContent = "/ " + (pdfViewer.pagesCount || "-");
    });
    window.addEventListener("resize", () => {
      if (pdfViewer.currentScaleValue === "auto" || pdfViewer.currentScaleValue?.startsWith("page-")) {
        pdfViewer.currentScaleValue = pdfViewer.currentScaleValue;
      }
    });
    window.addEventListener("message", (event) => {
      const message = event.data;
      if (message && message.type === "reload") {
        const state = message.preserveScroll === false
          ? { page: 1, scaleValue: "auto", scrollTop: 0, scrollLeft: 0 }
          : snapshotState();
        void loadVersion(String(message.version || Date.now()), state);
      }
      if (message && message.type === "showCompiling") {
        status.textContent = "Compiling snapshot " + String(message.version || "") + "...";
      }
      if (message && message.type === "showError") {
        status.textContent = "Calepin preview failed: " + String(message.message || "Compilation failed");
      }
      if (message && message.type === "syncToPdf" && autoScroll.checked) {
        const page = Number(message.page);
        if (Number.isFinite(page)) {
          setPdfPage(page);
        }
      }
    });

    function syncZoom() {
      const value = pdfViewer.currentScaleValue || String(pdfViewer.currentScale || 1);
      const option = Array.from(zoom.options).find((item) => item.value === value);
      if (option) {
        zoom.value = value;
      }
    }

    function snapshotState() {
      return {
        page: Math.max(1, pdfViewer.currentPageNumber || 1),
        scaleValue: pdfViewer.currentScaleValue || "auto",
        scrollTop: container.scrollTop,
        scrollLeft: container.scrollLeft,
        autoScroll: autoScroll.checked,
      };
    }

    async function restoreState(state) {
      pdfViewer.currentScaleValue = state.scaleValue || "auto";
      setPdfPage(state.page || 1);
      requestAnimationFrame(() => {
        container.scrollTop = state.scrollTop || 0;
        container.scrollLeft = state.scrollLeft || 0;
        pageNumber.value = String(pdfViewer.currentPageNumber || 1);
        pageCount.textContent = "/ " + (pdfViewer.pagesCount || "-");
        syncZoom();
      });
    }

    function pdfUrl(version) {
      const pdf = new URL(pdfSource);
      pdf.searchParams.set("v", version);
      pdf.searchParams.set("t", Date.now().toString());
      return pdf.toString();
    }

    async function loadVersion(version, state = snapshotState()) {
      if (loading) {
        queuedVersion = version;
        return;
      }
      loading = true;
      queuedVersion = null;
      const start = performance.now();
      status.textContent = "Rendering PDF...";
      logTiming("load " + version + " start");
      try {
        const fetchStart = performance.now();
        const response = await fetch(pdfUrl(version), { cache: "no-store" });
        if (!response.ok) {
          if (response.status === 404) {
            throw new Error("PDF output is not available yet.");
          }
          throw new Error("HTTP " + response.status + " while fetching PDF");
        }
        const data = new Uint8Array(await response.arrayBuffer());
        logTiming("load " + version + " fetched " + data.byteLength + " bytes in " + secondsSince(fetchStart));
        const parseStart = performance.now();
        const task = pdfjsLib.getDocument({
          data,
          disableAutoFetch: true,
          disableRange: true,
          disableStream: true,
        });
        const pdf = await task.promise;
        logTiming("load " + version + " parsed in " + secondsSince(parseStart));
        status.textContent = "PDF loaded in " + secondsSince(start) + ". Painting...";
        const pagesReady = new Promise((resolve) => eventBus.on("pagesinit", resolve, { once: true }));
        pdfViewer.setDocument(pdf);
        linkService.setDocument(pdf, null);
        await pagesReady;
        logTiming("load " + version + " pages initialized in " + secondsSince(start));
        await restoreState(state);
        lastVersion = version;
        status.textContent = "Calepin preview (" + secondsSince(startedAt) + " initial, " + secondsSince(start) + " render)";
        logTiming("load " + version + " done in " + secondsSince(start));
      } catch (error) {
        const message = String(error && error.message ? error.message : error);
        status.textContent = message === "PDF output is not available yet."
          ? "Waiting for PDF output..."
          : "Calepin preview failed: " + message;
      } finally {
        loading = false;
        if (queuedVersion && queuedVersion !== lastVersion) {
          const next = queuedVersion;
          queuedVersion = null;
          await loadVersion(next, snapshotState());
        }
      }
    }

    status.textContent = "Waiting for PDF output...";
    void loadVersion(Date.now().toString(), { page: 1, scaleValue: "auto", scrollTop: 0, scrollLeft: 0 });
  </script>
</body>
</html>`;
}

function stopWatch(): void {
  watchPdfFileWatcher?.dispose();
  watchPdfFileWatcher = null;
  watchPageSyncFileWatcher?.dispose();
  watchPageSyncFileWatcher = null;
  watchInput = null;
  watchPageSyncPath = null;
  watchPageSyncEntries = [];
  if (editorSyncTimer) {
    clearTimeout(editorSyncTimer);
    editorSyncTimer = null;
  }
  if (watchProcess) {
    const proc = watchProcess;
    watchProcess = null;
    proc.kill();
  }
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function escapeJavaScript(value: string): string {
  return value.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}

function randomNonce(): string {
  const chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  let value = "";
  for (let i = 0; i < 32; i++) {
    value += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return value;
}

async function calepinProcessEnv(input: vscode.Uri): Promise<NodeJS.ProcessEnv | undefined> {
  const python = await selectedPythonInterpreter(input);
  // Calepin reads this as the Python default unless .calepin/config.toml sets
  // an explicit executable path for the project.
  return python ? { [CALEPIN_PYTHON_ENV]: python } : undefined;
}

async function selectedPythonInterpreter(input: vscode.Uri): Promise<string | null> {
  return (await pythonInterpreterFromExtensionApi(input)) ?? (await pythonInterpreterFromCommand());
}

async function pythonInterpreterFromExtensionApi(input: vscode.Uri): Promise<string | null> {
  const extension = vscode.extensions.getExtension("ms-python.python");
  if (!extension) return null;

  try {
    const api = (await extension.activate()) as PythonExtensionApi | undefined;
    const details = await Promise.resolve(api?.settings?.getExecutionDetails?.(input));
    const execCommand = details?.execCommand;
    if (Array.isArray(execCommand)) {
      const python = cleanPythonPath(execCommand[0]);
      if (python) return python;
    }

    const environments = api?.environments;
    if (environments?.getActiveEnvironmentPath && environments.resolveEnvironment) {
      const active = await Promise.resolve(environments.getActiveEnvironmentPath(input));
      const resolved = await Promise.resolve(environments.resolveEnvironment(active));
      const python = extractPythonExecutable(resolved);
      if (python) return python;
    }
  } catch {
    return null;
  }

  return null;
}

async function pythonInterpreterFromCommand(): Promise<string | null> {
  try {
    return cleanPythonPath(await vscode.commands.executeCommand("python.interpreterPath"));
  } catch {
    return null;
  }
}

function extractPythonExecutable(value: unknown): string | null {
  if (typeof value === "string") return cleanPythonPath(value);
  if (!isRecord(value)) return null;

  return (
    pathFromUnknown(value.executable) ??
    pathFromUnknown(value.interpreterPath) ??
    pathFromUnknown(value.pythonPath)
  );
}

function pathFromUnknown(value: unknown): string | null {
  if (typeof value === "string") return cleanPythonPath(value);
  if (value instanceof vscode.Uri) return cleanPythonPath(value.fsPath);
  if (!isRecord(value)) return null;

  return (
    pathFromUnknown(value.uri) ??
    cleanPythonPath(value.fsPath) ??
    cleanPythonPath(value.path)
  );
}

function cleanPythonPath(value: unknown): string | null {
  if (typeof value !== "string") return null;
  const cleaned = value
    .trim()
    .replace(/^["']+|["']+$/g, "")
    .trim();
  if (!cleaned || cleaned.startsWith("${command:")) return null;
  return cleaned;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function formatCommand(
  binary: string,
  args: string[],
  env?: NodeJS.ProcessEnv,
): string {
  const envParts = Object.entries(env ?? {})
    .filter((entry): entry is [string, string] => typeof entry[1] === "string")
    .map(([key, value]) => `${key}=${shellQuote(value)}`);
  const commandParts = [binary, ...args].map(shellQuote);
  return [...envParts, ...commandParts].join(" ");
}

function shellQuote(value: string): string {
  if (/^[A-Za-z0-9_/:=.,@%+-]+$/.test(value)) return value;
  if (process.platform === "win32") {
    return `"${value.replace(/"/g, '\\"')}"`;
  }
  return `'${value.replace(/'/g, "'\\''")}'`;
}

function workspaceCwd(uri: vscode.Uri): string {
  const folder = vscode.workspace.getWorkspaceFolder(uri);
  return folder?.uri.fsPath ?? path.dirname(uri.fsPath);
}

function defaultOutputPath(input: string, format: string): string {
  const ext = `.${format}`;
  const parsed = path.parse(input);
  return path.join(parsed.dir, `${parsed.name}${ext}`);
}

function defaultPageSyncPath(input: string): string {
  const parsed = path.parse(input);
  return path.join(parsed.dir, ".calepin", parsed.name, "pages.json");
}
