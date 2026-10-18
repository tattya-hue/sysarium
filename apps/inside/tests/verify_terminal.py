#!/usr/bin/env python3
"""P2/P3の実PTY検証．高負荷ワークロードを起動せず，通常の/proc取得だけを行う．"""
import fcntl
import os
from pathlib import Path
import pty
import select
import signal
import struct
import subprocess
import termios
import time

ROOT = Path(__file__).resolve().parents[3]


def verify(args, interactive=False, terminate_signal=None):
    master, slave = pty.openpty()
    original = termios.tcgetattr(slave)

    def resize(width, height):
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))

    def control_terminal():
        os.setsid()
        fcntl.ioctl(slave, termios.TIOCSCTTY, 0)

    resize(140, 34)
    process = subprocess.Popen(
        args, cwd=ROOT, stdin=slave, stdout=slave, stderr=slave,
        env={**os.environ, "TERM": "xterm-256color"}, preexec_fn=control_terminal,
    )
    output = bytearray()
    deadline = time.monotonic() + 15
    entered_at = None
    actions = [False] * 11
    signal_sent = False
    try:
        while True:
            if select.select([master], [], [], 0.03)[0]:
                output.extend(os.read(master, 65536))
            if entered_at is None and b"\x1b[?1049h" in output:
                entered_at = time.monotonic()
            if terminate_signal is not None and entered_at is not None and not signal_sent:
                if time.monotonic() - entered_at >= 0.3:
                    os.kill(process.pid, terminate_signal)
                    signal_sent = True
            if interactive and entered_at is not None:
                elapsed = time.monotonic() - entered_at
                for index, after in enumerate([0.2, 0.4, 0.6, 0.8, 1.0, 1.2, 1.4, 1.6, 1.8, 2.0, 2.2]):
                    if elapsed >= after and not actions[index]:
                        keys = {0: b"?", 1: b"\x1b[6~", 2: b"\x1b", 3: b"i", 4: b"\x1b[6~", 6: b"\x1b", 7: b"\t", 8: b"\t", 10: b"q"}
                        if index in keys:
                            os.write(master, keys[index])
                        else:
                            resize(*((40, 24) if index == 5 else (80, 24)))
                            os.kill(process.pid, signal.SIGWINCH)
                        actions[index] = True
            if process.poll() is not None:
                while select.select([master], [], [], 0)[0]:
                    output.extend(os.read(master, 65536))
                break
            if time.monotonic() > deadline:
                raise AssertionError(f"PTY timeout: {args}\n{output.decode(errors='replace')}")
        assert process.returncode == 0, output.decode(errors="replace")
        assert entered_at is not None, "alternate screen was not entered"
        assert b"\x1b[?1049l" in output, "alternate screen was not left"
        assert b"\x1b[?25h" in output, "cursor was not restored"
        assert termios.tcgetattr(slave) == original, "terminal attributes were not restored"
        if interactive:
            assert all(actions), "live exited before Tab/resize/q sequence completed"
        elif terminate_signal is None:
            assert b"1 passed" in output, output.decode(errors="replace")
        if terminate_signal is not None:
            assert signal_sent, "signal was not delivered"
        print("PASS:", " ".join(args), signal.Signals(terminate_signal).name if terminate_signal else "", "(raw mode, screen, cursor restored)")
    finally:
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
        os.close(master)
        os.close(slave)


def main():
    subprocess.run(["cargo", "build", "-p", "inside"], cwd=ROOT, check=True)
    verify([str(ROOT / "target/debug/inside")], interactive=True)
    for stop_signal in [signal.SIGTERM, signal.SIGHUP, signal.SIGINT]:
        verify([str(ROOT / "target/debug/inside"), "live"], terminate_signal=stop_signal)
    for name in ["restores_after_error", "restores_after_panic"]:
        verify([
            "cargo", "test", "-p", "inside", f"tui::terminal::tests::{name}", "--",
            "--ignored", "--exact", "--nocapture", "--test-threads=1",
        ])
    result = subprocess.run([str(ROOT / "target/debug/inside"), "live"], capture_output=True)
    assert result.returncode == 1 and result.stdout == b"" and b"requires a terminal" in result.stderr
    print("PASS: non-TTY rejection (stdout empty)")


if __name__ == "__main__":
    main()
