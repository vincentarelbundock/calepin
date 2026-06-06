#!/usr/bin/env python3
"""Build and serve the Calepin website locally.

Runs `make website`, starts a static server, and opens Firefox.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
import time
import webbrowser
from pathlib import Path

ROOT = Path(__file__).resolve().parent
DEFAULT_PORT = 8000
DEFAULT_HOST = "localhost"
DEFAULT_DIR = "docs"


def run_website_build(extra_args: list[str]) -> None:
    command = ["make", "website"]
    command.extend(extra_args)
    subprocess.run(command, check=True, cwd=ROOT)


def open_firefox(url: str) -> None:
    try:
        browser = webbrowser.get("firefox")
        browser.open(url, new=2)
        return
    except webbrowser.Error:
        pass

    try:
        if sys.platform.startswith("darwin"):
            subprocess.run(["open", "-a", "Firefox", url], check=True)
            return
        if sys.platform.startswith("linux"):
            subprocess.run(["firefox", url], check=True)
            return
        if sys.platform.startswith("win"):
            subprocess.run(["cmd", "/c", "start", "firefox", url], check=True)
            return
    except (OSError, subprocess.CalledProcessError):
        pass

    webbrowser.open(url, new=2)


def serve_directory(directory: Path, host: str, port: int) -> subprocess.Popen:
    return subprocess.Popen(
        [
            sys.executable,
            "-m",
            "http.server",
            str(port),
            "--bind",
            host,
            "--directory",
            str(directory),
        ],
        cwd=ROOT,
        stdout=None,
        stderr=None,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=DEFAULT_PORT)
    parser.add_argument("--host", default=DEFAULT_HOST)
    parser.add_argument("--dir", default=DEFAULT_DIR)
    parser.add_argument("--no-open", action="store_true", help="Do not open Firefox")

    args = parser.parse_args()
    directory = (ROOT / args.dir).resolve()
    if not directory.is_dir():
        print(f"directory not found: {directory}", file=sys.stderr)
        return 1

    run_website_build([])

    server = serve_directory(directory, args.host, args.port)

    url = f"http://{args.host}:{args.port}/"
    if not args.no_open:
        time.sleep(0.3)
        open_firefox(url)

    try:
        print(f"Serving {directory} at {url}")
        print("Press Ctrl+C to stop.")
        server.wait()
        return 0
    except KeyboardInterrupt:
        print("\nStopping server...")
        server.terminate()
        try:
            server.wait(timeout=2)
        except subprocess.TimeoutExpired:
            server.kill()
        return 0


if __name__ == "__main__":
    raise SystemExit(main())
