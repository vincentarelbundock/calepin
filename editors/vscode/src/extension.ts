import * as path from "path";
import { spawn, type ChildProcessWithoutNullStreams } from "child_process";
import * as vscode from "vscode";

const CALEPIN_PYTHON_ENV = "CALEPIN_PYTHON";

let output: vscode.OutputChannel | null = null;
let watchProcess: ChildProcessWithoutNullStreams | null = null;
let watchInput: vscode.Uri | null = null;
let watchConfig: string | null = null;

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

export function activate(context: vscode.ExtensionContext): void {
  output = vscode.window.createOutputChannel("Calepin");
  context.subscriptions.push(
    output,
    vscode.commands.registerCommand("calepin.start", (uri?: vscode.Uri) =>
      startCalepinWatch(context, uri),
    ),
    vscode.commands.registerCommand("calepin.stop", stopCalepinWatch),
  );
}

export function deactivate(): void {
  stopWatch();
}

async function startCalepinWatch(
  context: vscode.ExtensionContext,
  uri?: vscode.Uri,
): Promise<void> {
  const input = await resolveTypstFile(uri);
  if (!input || !(await saveTypstDocument(input))) return;

  const configPath = await pickConfigFile(input);
  if (configPath === undefined) return;

  if (
    watchProcess &&
    watchInput &&
    sameFsPath(watchInput.fsPath, input.fsPath) &&
    sameConfigChoice(watchConfig, configPath)
  ) {
    vscode.window.setStatusBarMessage("Calepin: already watching this document", 3000);
    return;
  }

  const binary = await findBinary(context);
  if (!binary) return;

  stopWatch();
  const args = ["watch", input.fsPath, "--eval-only"];
  if (configPath) args.push("--config", configPath);
  const process = startCalepin(
    binary,
    args,
    input,
    await calepinProcessEnv(input),
  );
  if (!process) return;

  watchProcess = process;
  watchInput = input;
  watchConfig = configPath;
  const configNote = configPath ? ` with ${path.basename(configPath)}` : "";
  void vscode.window.showInformationMessage(
    `Calepin is watching ${path.basename(input.fsPath)}${configNote} in the background.`,
  );
  vscode.window.setStatusBarMessage(
    "Calepin: watching code (run Typst: Stop Calepin to stop)",
    5000,
  );

  process.on("exit", (code) => {
    if (watchProcess !== process) return;
    watchProcess = null;
    watchInput = null;
    watchConfig = null;
    output?.appendLine(`\nCalepin code watch exited with code ${code ?? "null"}.`);
    if (code !== 0 && code !== null) {
      vscode.window.showErrorMessage(
        "Calepin code watch stopped unexpectedly. See the Calepin output for details.",
      );
    }
  });
}

function stopCalepinWatch(): void {
  if (!watchProcess) {
    vscode.window.setStatusBarMessage("Calepin: no code watch is running", 3000);
    return;
  }

  stopWatch();
  output?.appendLine("Calepin code watch stopped.");
  vscode.window.setStatusBarMessage("Calepin: code watch stopped", 3000);
}

function startCalepin(
  binary: string,
  args: string[],
  input: vscode.Uri,
  environment?: NodeJS.ProcessEnv,
): ChildProcessWithoutNullStreams | null {
  output?.appendLine(`\n$ ${formatCommand(binary, args, environment)}`);

  try {
    const process = spawn(binary, args, {
      cwd: workspaceCwd(input),
      env: { ...globalThis.process.env, ...environment },
    });
    process.stdout.on("data", (chunk: Buffer) => output?.append(chunk.toString()));
    process.stderr.on("data", (chunk: Buffer) => output?.append(chunk.toString()));
    process.on("error", (error) => {
      output?.appendLine(`\nFailed to start Calepin: ${error.message}`);
      vscode.window.showErrorMessage(
        "Failed to start Calepin. See the Calepin output for details.",
      );
    });
    return process;
  } catch (error) {
    output?.appendLine(`\nFailed to start Calepin: ${String(error)}`);
    vscode.window.showErrorMessage(
      "Failed to start Calepin. See the Calepin output for details.",
    );
    return null;
  }
}

function stopWatch(): void {
  if (watchProcess) {
    const process = watchProcess;
    watchProcess = null;
    process.kill();
  }
  watchInput = null;
  watchConfig = null;
}

/** Config path to pass as `--config`, null for no config, undefined on cancel. */
async function pickConfigFile(input: vscode.Uri): Promise<string | null | undefined> {
  const noConfig = { label: "Start", description: "watch without a config file" };
  const withConfig = {
    label: "Start with config file…",
    description: "pass --config to calepin watch",
  };
  const choice = await vscode.window.showQuickPick([noConfig, withConfig], {
    placeHolder: `Calepin: watch ${path.basename(input.fsPath)}`,
  });
  if (!choice) return undefined;
  if (choice === noConfig) return null;

  const folder = vscode.workspace.getWorkspaceFolder(input);
  const picked = await vscode.window.showOpenDialog({
    canSelectFiles: true,
    canSelectFolders: false,
    canSelectMany: false,
    filters: { "TOML files": ["toml"] },
    openLabel: "Use config",
    defaultUri: folder?.uri ?? vscode.Uri.file(path.dirname(input.fsPath)),
  });
  return picked?.[0]?.fsPath;
}

function sameConfigChoice(left: string | null, right: string | null): boolean {
  if (left === null || right === null) return left === right;
  return sameFsPath(left, right);
}

async function resolveTypstFile(uri?: vscode.Uri): Promise<vscode.Uri | null> {
  if (uri?.scheme === "file" && uri.fsPath.endsWith(".typ")) return uri;

  const active = vscode.window.activeTextEditor?.document.uri;
  if (active?.scheme === "file" && active.fsPath.endsWith(".typ")) return active;

  const picked = await vscode.window.showOpenDialog({
    canSelectFiles: true,
    canSelectFolders: false,
    canSelectMany: false,
    filters: { "Typst files": ["typ"] },
    openLabel: "Select Typst file",
  });
  return picked?.[0] ?? null;
}

async function saveTypstDocument(input: vscode.Uri): Promise<boolean> {
  const document = vscode.workspace.textDocuments.find(
    (candidate) =>
      candidate.uri.scheme === "file" && sameFsPath(candidate.uri.fsPath, input.fsPath),
  );
  if (!document?.isDirty) return true;
  if (await document.save()) return true;

  vscode.window.showErrorMessage("Save the Typst document before evaluating its code.");
  return false;
}

async function findBinary(
  context: vscode.ExtensionContext,
): Promise<string | null> {
  const configured = vscode.workspace
    .getConfiguration("calepin")
    .get<string>("binaryPath", "")
    .trim()
    .replace(/^["']+|["']+$/g, "")
    .trim();
  if (configured) return configured;

  const executable = globalThis.process.platform === "win32" ? "calepin.exe" : "calepin";
  const bundled = vscode.Uri.joinPath(context.extensionUri, "bin", executable);
  try {
    await vscode.workspace.fs.stat(bundled);
    if (globalThis.process.platform !== "win32") {
      const fs = await import("fs");
      fs.chmodSync(bundled.fsPath, 0o755);
    }
    return bundled.fsPath;
  } catch {
    return executable;
  }
}

async function calepinProcessEnv(input: vscode.Uri): Promise<NodeJS.ProcessEnv | undefined> {
  const python = await selectedPythonInterpreter(input);
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

function formatCommand(
  binary: string,
  args: string[],
  environment?: NodeJS.ProcessEnv,
): string {
  const variables = Object.entries(environment ?? {})
    .filter((entry): entry is [string, string] => typeof entry[1] === "string")
    .map(([key, value]) => `${key}=${shellQuote(value)}`);
  return [...variables, binary, ...args].map(shellQuote).join(" ");
}

function shellQuote(value: string): string {
  if (/^[A-Za-z0-9_/:=.,@%+-]+$/.test(value)) return value;
  if (globalThis.process.platform === "win32") {
    return `"${value.replace(/"/g, '\\"')}"`;
  }
  return `'${value.replace(/'/g, "'\\''")}'`;
}

function workspaceCwd(uri: vscode.Uri): string {
  const folder = vscode.workspace.getWorkspaceFolder(uri);
  return folder?.uri.fsPath ?? path.dirname(uri.fsPath);
}

function sameFsPath(left: string, right: string): boolean {
  const normalizedLeft = path.normalize(left);
  const normalizedRight = path.normalize(right);
  return globalThis.process.platform === "win32"
    ? normalizedLeft.toLowerCase() === normalizedRight.toLowerCase()
    : normalizedLeft === normalizedRight;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
