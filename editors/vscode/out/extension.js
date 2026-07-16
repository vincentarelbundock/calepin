"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const path = __importStar(require("path"));
const child_process_1 = require("child_process");
const vscode = __importStar(require("vscode"));
const CALEPIN_PYTHON_ENV = "CALEPIN_PYTHON";
let output = null;
let watchProcess = null;
let watchInput = null;
function activate(context) {
    output = vscode.window.createOutputChannel("Calepin");
    context.subscriptions.push(output, vscode.commands.registerCommand("calepin.start", (uri) => startCalepinWatch(context, uri)), vscode.commands.registerCommand("calepin.stop", stopCalepinWatch));
}
function deactivate() {
    stopWatch();
}
async function startCalepinWatch(context, uri) {
    const input = await resolveTypstFile(uri);
    if (!input || !(await saveTypstDocument(input)))
        return;
    if (watchProcess && watchInput && sameFsPath(watchInput.fsPath, input.fsPath)) {
        output?.show(true);
        vscode.window.setStatusBarMessage("Calepin: already watching this document", 3000);
        return;
    }
    const binary = await findBinary(context);
    if (!binary)
        return;
    stopWatch();
    const args = ["watch", input.fsPath, "--eval-only"];
    const process = startCalepin(binary, args, input, await calepinProcessEnv(input));
    if (!process)
        return;
    watchProcess = process;
    watchInput = input;
    vscode.window.setStatusBarMessage("Calepin: watching code (run Typst: Stop Calepin to stop)", 5000);
    process.on("exit", (code) => {
        if (watchProcess !== process)
            return;
        watchProcess = null;
        watchInput = null;
        output?.appendLine(`\nCalepin code watch exited with code ${code ?? "null"}.`);
        if (code !== 0 && code !== null) {
            vscode.window.showErrorMessage("Calepin code watch stopped unexpectedly. See the Calepin output for details.");
        }
    });
}
function stopCalepinWatch() {
    if (!watchProcess) {
        vscode.window.setStatusBarMessage("Calepin: no code watch is running", 3000);
        return;
    }
    stopWatch();
    output?.appendLine("Calepin code watch stopped.");
    vscode.window.setStatusBarMessage("Calepin: code watch stopped", 3000);
}
function startCalepin(binary, args, input, environment) {
    output?.appendLine(`\n$ ${formatCommand(binary, args, environment)}`);
    output?.show(true);
    try {
        const process = (0, child_process_1.spawn)(binary, args, {
            cwd: workspaceCwd(input),
            env: { ...globalThis.process.env, ...environment },
        });
        process.stdout.on("data", (chunk) => output?.append(chunk.toString()));
        process.stderr.on("data", (chunk) => output?.append(chunk.toString()));
        process.on("error", (error) => {
            output?.appendLine(`\nFailed to start Calepin: ${error.message}`);
            vscode.window.showErrorMessage("Failed to start Calepin. See the Calepin output for details.");
        });
        return process;
    }
    catch (error) {
        output?.appendLine(`\nFailed to start Calepin: ${String(error)}`);
        vscode.window.showErrorMessage("Failed to start Calepin. See the Calepin output for details.");
        return null;
    }
}
function stopWatch() {
    if (watchProcess) {
        const process = watchProcess;
        watchProcess = null;
        process.kill();
    }
    watchInput = null;
}
async function resolveTypstFile(uri) {
    if (uri?.scheme === "file" && uri.fsPath.endsWith(".typ"))
        return uri;
    const active = vscode.window.activeTextEditor?.document.uri;
    if (active?.scheme === "file" && active.fsPath.endsWith(".typ"))
        return active;
    const picked = await vscode.window.showOpenDialog({
        canSelectFiles: true,
        canSelectFolders: false,
        canSelectMany: false,
        filters: { "Typst files": ["typ"] },
        openLabel: "Select Typst file",
    });
    return picked?.[0] ?? null;
}
async function saveTypstDocument(input) {
    const document = vscode.workspace.textDocuments.find((candidate) => candidate.uri.scheme === "file" && sameFsPath(candidate.uri.fsPath, input.fsPath));
    if (!document?.isDirty)
        return true;
    if (await document.save())
        return true;
    vscode.window.showErrorMessage("Save the Typst document before evaluating its code.");
    return false;
}
async function findBinary(context) {
    const configured = vscode.workspace
        .getConfiguration("calepin")
        .get("binaryPath", "")
        .trim()
        .replace(/^["']+|["']+$/g, "")
        .trim();
    if (configured)
        return configured;
    const executable = globalThis.process.platform === "win32" ? "calepin.exe" : "calepin";
    const bundled = vscode.Uri.joinPath(context.extensionUri, "bin", executable);
    try {
        await vscode.workspace.fs.stat(bundled);
        if (globalThis.process.platform !== "win32") {
            const fs = await Promise.resolve().then(() => __importStar(require("fs")));
            fs.chmodSync(bundled.fsPath, 0o755);
        }
        return bundled.fsPath;
    }
    catch {
        return executable;
    }
}
async function calepinProcessEnv(input) {
    const python = await selectedPythonInterpreter(input);
    return python ? { [CALEPIN_PYTHON_ENV]: python } : undefined;
}
async function selectedPythonInterpreter(input) {
    return (await pythonInterpreterFromExtensionApi(input)) ?? (await pythonInterpreterFromCommand());
}
async function pythonInterpreterFromExtensionApi(input) {
    const extension = vscode.extensions.getExtension("ms-python.python");
    if (!extension)
        return null;
    try {
        const api = (await extension.activate());
        const details = await Promise.resolve(api?.settings?.getExecutionDetails?.(input));
        const execCommand = details?.execCommand;
        if (Array.isArray(execCommand)) {
            const python = cleanPythonPath(execCommand[0]);
            if (python)
                return python;
        }
        const environments = api?.environments;
        if (environments?.getActiveEnvironmentPath && environments.resolveEnvironment) {
            const active = await Promise.resolve(environments.getActiveEnvironmentPath(input));
            const resolved = await Promise.resolve(environments.resolveEnvironment(active));
            const python = extractPythonExecutable(resolved);
            if (python)
                return python;
        }
    }
    catch {
        return null;
    }
    return null;
}
async function pythonInterpreterFromCommand() {
    try {
        return cleanPythonPath(await vscode.commands.executeCommand("python.interpreterPath"));
    }
    catch {
        return null;
    }
}
function extractPythonExecutable(value) {
    if (typeof value === "string")
        return cleanPythonPath(value);
    if (!isRecord(value))
        return null;
    return (pathFromUnknown(value.executable) ??
        pathFromUnknown(value.interpreterPath) ??
        pathFromUnknown(value.pythonPath));
}
function pathFromUnknown(value) {
    if (typeof value === "string")
        return cleanPythonPath(value);
    if (value instanceof vscode.Uri)
        return cleanPythonPath(value.fsPath);
    if (!isRecord(value))
        return null;
    return (pathFromUnknown(value.uri) ??
        cleanPythonPath(value.fsPath) ??
        cleanPythonPath(value.path));
}
function cleanPythonPath(value) {
    if (typeof value !== "string")
        return null;
    const cleaned = value
        .trim()
        .replace(/^["']+|["']+$/g, "")
        .trim();
    if (!cleaned || cleaned.startsWith("${command:"))
        return null;
    return cleaned;
}
function formatCommand(binary, args, environment) {
    const variables = Object.entries(environment ?? {})
        .filter((entry) => typeof entry[1] === "string")
        .map(([key, value]) => `${key}=${shellQuote(value)}`);
    return [...variables, binary, ...args].map(shellQuote).join(" ");
}
function shellQuote(value) {
    if (/^[A-Za-z0-9_/:=.,@%+-]+$/.test(value))
        return value;
    if (globalThis.process.platform === "win32") {
        return `"${value.replace(/"/g, '\\"')}"`;
    }
    return `'${value.replace(/'/g, "'\\''")}'`;
}
function workspaceCwd(uri) {
    const folder = vscode.workspace.getWorkspaceFolder(uri);
    return folder?.uri.fsPath ?? path.dirname(uri.fsPath);
}
function sameFsPath(left, right) {
    const normalizedLeft = path.normalize(left);
    const normalizedRight = path.normalize(right);
    return globalThis.process.platform === "win32"
        ? normalizedLeft.toLowerCase() === normalizedRight.toLowerCase()
        : normalizedLeft === normalizedRight;
}
function isRecord(value) {
    return typeof value === "object" && value !== null;
}
//# sourceMappingURL=extension.js.map