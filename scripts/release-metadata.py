#!/usr/bin/env python3
from __future__ import annotations

import os
import subprocess

PLATFORM_MAP = {
    "x86_64-unknown-linux-gnu": "linux-x86_64",
    "aarch64-unknown-linux-gnu": "linux-aarch64",
    "x86_64-apple-darwin": "macos-x86_64",
    "aarch64-apple-darwin": "macos-aarch64",
    "x86_64-pc-windows-msvc": "windows-x86_64",
    "aarch64-pc-windows-msvc": "windows-aarch64",
}


def rust_host_triple() -> str:
    output = subprocess.check_output(["rustc", "-vV"], text=True)
    for line in output.splitlines():
        if line.startswith("host: "):
            return line.split(": ", 1)[1].strip()
    raise SystemExit("Could not determine rustc host triple")


def normalized_platform(host: str) -> str:
    if host in PLATFORM_MAP:
        return PLATFORM_MAP[host]
    cpu = host.split("-", 1)[0]
    if "windows" in host:
        os_name = "windows"
    elif "apple-darwin" in host:
        os_name = "macos"
    elif "linux" in host:
        os_name = "linux"
    else:
        os_name = host.replace("-", "_")
    return f"{os_name}-{cpu}"


def version_value() -> str:
    if os.environ.get("GITHUB_REF_TYPE") == "tag":
        return os.environ["GITHUB_REF_NAME"]
    sha = os.environ.get("GITHUB_SHA", "dev")
    return sha[:12]


def main() -> int:
    host = rust_host_triple()
    platform = normalized_platform(host)
    version = version_value()
    github_output = os.environ.get("GITHUB_OUTPUT")
    if github_output:
        with open(github_output, "a", encoding="utf-8") as handle:
            handle.write(f"version={version}\n")
            handle.write(f"platform={platform}\n")
            handle.write(f"host={host}\n")
    print(f"host={host}")
    print(f"platform={platform}")
    print(f"version={version}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
