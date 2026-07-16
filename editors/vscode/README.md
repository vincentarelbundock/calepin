# Calepin for VS Code

The Calepin VS Code extension adds watched code evaluation to Typst documents.
It does not provide or control preview; use Tinymist or another Typst preview
extension independently.

- `Typst: Calepin` runs `calepin watch --eval-only` for the active document in
  the background and shows a notification when watching starts. Logs remain in
  the Calepin output channel without opening the bottom panel.
- `Typst: Stop Calepin` stops the watcher started by the extension.

The extension uses its bundled Calepin binary when available, then falls back
to `calepin` on `PATH`. Set `calepin.binaryPath` to use another executable.
See https://vincentarelbundock.github.io/calepin/ for full details.
