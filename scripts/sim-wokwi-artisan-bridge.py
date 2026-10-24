#!/usr/bin/env python3
"""Bridge the Wokwi for VS Code RFC2217 serial server to a local PTY for Artisan."""

from __future__ import annotations

import argparse
import errno
import os
import pty
import select
import signal
import sys
import termios
import time
import tty
from pathlib import Path
from types import FrameType

try:
    import serial
    from serial import SerialException
except ImportError:
    print(
        "pyserial is required: python3 -m pip install pyserial",
        file=sys.stderr,
    )
    raise SystemExit(2)


class WokwiArtisanBridge:
    def __init__(
        self,
        host: str,
        port: int,
        baud: int,
        link: Path,
        reconnect_delay: float,
        verbose: bool,
        trace_path: Path | None,
    ) -> None:
        self.host = host
        self.port = port
        self.baud = baud
        self.link = link
        self.reconnect_delay = reconnect_delay
        self.verbose = verbose
        self.trace_path = trace_path
        self.trace_file = None
        self.master_fd = -1
        self.slave_path = ""
        self.serial_port = None
        self.stop_requested = False

    def request_stop(self, _signum: int, _frame: FrameType | None) -> None:
        self.stop_requested = True

    def log(self, message: str) -> None:
        print(message, flush=True)

    def debug(self, message: str) -> None:
        if self.verbose:
            print(message, file=sys.stderr, flush=True)

    def trace(self, direction: str, data: bytes) -> None:
        if self.trace_file is None:
            return
        self.trace_file.write(f"{time.monotonic():.3f} {direction} {data!r}\n")
        self.trace_file.flush()

    def open_pty(self) -> None:
        master_fd, slave_fd = pty.openpty()
        try:
            tty.setraw(slave_fd)
            attrs = termios.tcgetattr(slave_fd)
            attrs[2] |= termios.CLOCAL | termios.CREAD
            termios.tcsetattr(slave_fd, termios.TCSANOW, attrs)
            self.slave_path = os.ttyname(slave_fd)
        finally:
            os.close(slave_fd)

        os.set_blocking(master_fd, False)
        self.master_fd = master_fd

        if self.link.is_symlink():
            self.link.unlink()
        elif self.link.exists():
            raise SystemExit(
                f"{self.link} exists and is not a symlink; choose another --link path"
            )

        self.link.symlink_to(self.slave_path)

    def cleanup(self) -> None:
        if self.trace_file is not None:
            self.trace_file.close()
            self.trace_file = None

        if self.master_fd >= 0:
            try:
                os.close(self.master_fd)
            except OSError:
                pass
            self.master_fd = -1

        if self.link.is_symlink():
            try:
                if os.readlink(self.link) == self.slave_path:
                    self.link.unlink()
            except OSError:
                pass

    def connect(self):
        url = f"rfc2217://{self.host}:{self.port}"
        self.debug(f"Connecting to {url}")
        return serial.serial_for_url(
            url,
            baudrate=self.baud,
            timeout=0.02,
            rtscts=False,
            dsrdtr=False,
        )

    def close_serial(self) -> None:
        if self.serial_port is not None:
            try:
                self.serial_port.close()
            except (SerialException, OSError):
                pass
            self.serial_port = None

    def write_master(self, data: bytes) -> None:
        view = memoryview(data)
        while view:
            try:
                written = os.write(self.master_fd, view)
            except OSError as exc:
                if exc.errno == errno.EIO:
                    return
                if exc.errno in (errno.EAGAIN, errno.EWOULDBLOCK):
                    _, writable, _ = select.select([], [self.master_fd], [], 0.1)
                    if not writable:
                        return
                    continue
                raise
            view = view[written:]

    def pump(self) -> None:
        if self.serial_port is None:
            return

        data = self.serial_port.read(4096)
        if data:
            self.trace("S", data)
            self.write_master(data)

        while True:
            try:
                readable, _, _ = select.select([self.master_fd], [], [], 0)
            except InterruptedError:
                continue
            if not readable:
                return

            try:
                chunk = os.read(self.master_fd, 4096)
            except BlockingIOError:
                return
            except OSError as exc:
                if exc.errno == errno.EIO:
                    time.sleep(0.01)
                    return
                raise

            if not chunk:
                return

            self.trace("A", chunk)
            self.serial_port.write(chunk)
            self.serial_port.flush()

    def run(self) -> None:
        if self.trace_path is not None:
            try:
                self.trace_file = self.trace_path.open("w", encoding="utf-8")
            except OSError as exc:
                raise SystemExit(
                    f"cannot open trace file {self.trace_path}: {exc}"
                ) from exc

        self.open_pty()
        self.log(f"Artisan port: {self.link} -> {self.slave_path}")
        self.log(f"Wokwi RFC2217: rfc2217://{self.host}:{self.port} ({self.baud} 8N1)")
        self.log("Start the Wokwi simulator in VS Code and keep its tab visible.")
        self.log("Close the Wokwi serial monitor before Artisan connects.")
        self.log("Press Ctrl-C to stop the bridge.")

        last_error = ""
        while not self.stop_requested:
            if self.serial_port is None:
                try:
                    self.serial_port = self.connect()
                    self.log("Connected to the Wokwi RFC2217 server.")
                    last_error = ""
                except (SerialException, OSError) as exc:
                    message = str(exc)
                    if message != last_error:
                        print(
                            f"Waiting for the Wokwi RFC2217 server: {message}",
                            file=sys.stderr,
                            flush=True,
                        )
                        last_error = message
                    time.sleep(self.reconnect_delay)
                    continue

            try:
                self.pump()
            except (SerialException, OSError) as exc:
                print(
                    f"Wokwi serial disconnected: {exc}; reconnecting",
                    file=sys.stderr,
                    flush=True,
                )
                self.close_serial()

        self.close_serial()
        self.log("Bridge stopped.")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Expose the Wokwi for VS Code RFC2217 serial server as a local PTY "
            "so the Artisan desktop app can connect to the simulated firmware."
        )
    )
    parser.add_argument(
        "--rfc2217",
        default="127.0.0.1:4000",
        help="Wokwi RFC2217 host:port (default: 127.0.0.1:4000)",
    )
    parser.add_argument(
        "--baud",
        type=int,
        default=115200,
        help="Serial baud rate (default: 115200)",
    )
    parser.add_argument(
        "--link",
        default="/tmp/libreroaster-wokwi",
        help="PTY symlink to create for Artisan (default: /tmp/libreroaster-wokwi)",
    )
    parser.add_argument(
        "--reconnect-delay",
        type=float,
        default=1.0,
        help="Seconds between RFC2217 connection attempts (default: 1.0)",
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Print connection diagnostics",
    )
    parser.add_argument(
        "--trace",
        type=Path,
        default=None,
        help="Write timestamped raw serial traffic to this file",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        host, port_text = args.rfc2217.rsplit(":", 1)
        port = int(port_text)
    except ValueError:
        print("--rfc2217 must be HOST:PORT", file=sys.stderr)
        return 2

    bridge = WokwiArtisanBridge(
        host=host,
        port=port,
        baud=args.baud,
        link=Path(args.link),
        reconnect_delay=args.reconnect_delay,
        verbose=args.verbose,
        trace_path=args.trace,
    )
    signal.signal(signal.SIGINT, bridge.request_stop)
    signal.signal(signal.SIGTERM, bridge.request_stop)

    try:
        bridge.run()
    finally:
        bridge.cleanup()

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
