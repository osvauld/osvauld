"""
Session - Manages a single slint_shell instance

A Session encapsulates:
- Starting the shell process with control socket
- Waiting for the socket to be ready
- Providing a ControlClient for communication
- Cleanup on exit
"""

import os
import subprocess
import time
from pathlib import Path
from typing import Optional

from .client import ControlClient


# Default paths
DEFAULT_SHELL_BINARY = Path(__file__).parent.parent.parent / "target" / "debug" / "slint_shell"


class Session:
    """Manages a single slint_shell instance."""

    def __init__(
        self,
        name: str,
        data_dir: str,
        socket_path: Optional[str] = None,
        shell_binary: Optional[Path] = None,
        startup_timeout: float = 5.0,
        show_ui: bool = False,
    ):
        """
        Args:
            name: Instance name (used for logging, display name)
            data_dir: Directory for database storage (STHALAM_DATA_DIR)
            socket_path: Path for control socket (default: {data_dir}/{name}.sock)
            shell_binary: Path to slint_shell binary
            startup_timeout: How long to wait for socket to be ready
            show_ui: If True, don't set SLINT_BACKEND=testing (show actual UI)
        """
        self.name = name
        self.data_dir = data_dir
        self.socket_path = socket_path or f"{data_dir}/{name}.sock"
        self.shell_binary = shell_binary or DEFAULT_SHELL_BINARY
        self.startup_timeout = startup_timeout
        self.show_ui = show_ui

        self.process: Optional[subprocess.Popen] = None
        self._client: Optional[ControlClient] = None

    @property
    def client(self) -> ControlClient:
        """Get the ControlClient for this session."""
        if self._client is None:
            raise RuntimeError(f"Session '{self.name}' not started")
        return self._client

    def start(self) -> "Session":
        """Start the shell process and wait for socket.

        Returns:
            self (for chaining)

        Raises:
            FileNotFoundError: If shell binary doesn't exist
            RuntimeError: If shell fails to start or socket not ready in time
        """
        if not self.shell_binary.exists():
            raise FileNotFoundError(
                f"Shell binary not found: {self.shell_binary}\n"
                "Run: cargo build -p slint_shell"
            )

        # Ensure data directory exists
        os.makedirs(self.data_dir, exist_ok=True)

        # Clean up stale socket
        if os.path.exists(self.socket_path):
            os.remove(self.socket_path)

        # Set up environment
        env = os.environ.copy()
        env["STHALAM_DATA_DIR"] = self.data_dir
        if not self.show_ui:
            env["SLINT_BACKEND"] = "testing"

        # Start process
        self.process = subprocess.Popen(
            [
                str(self.shell_binary),
                "-d", self.name,
                "--debug-socket", self.socket_path,
            ],
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

        # Wait for socket to be available
        start_time = time.time()
        while time.time() - start_time < self.startup_timeout:
            if os.path.exists(self.socket_path):
                try:
                    client = ControlClient(self.socket_path)
                    if client.ping():
                        self._client = client
                        return self
                except (ConnectionError, RuntimeError):
                    pass
            time.sleep(0.1)

        # Startup failed
        self.stop()
        raise RuntimeError(
            f"Session '{self.name}' failed to start within {self.startup_timeout}s"
        )

    def stop(self) -> None:
        """Stop the shell process and clean up."""
        if self.process:
            self.process.terminate()
            try:
                self.process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
            self.process = None

        # Clean up socket
        if os.path.exists(self.socket_path):
            os.remove(self.socket_path)

        self._client = None

    def is_running(self) -> bool:
        """Check if the shell process is running."""
        return self.process is not None and self.process.poll() is None

    # ============================================================
    # Convenience methods (delegate to client)
    # ============================================================

    def eval(self, code: str):
        """Execute Lua code."""
        return self.client.eval(code)

    def signup(self, username: str, passphrase: str):
        """Sign up a new user."""
        return self.client.signup(username, passphrase)

    def login(self, passphrase: str):
        """Login with passphrase."""
        return self.client.login(passphrase)

    def create_space(self, name: str, template_path: str):
        """Create a space."""
        return self.client.create_space(name, template_path)

    def import_page(self, space_id: str, page_dir: str):
        """Import a page."""
        return self.client.import_page(space_id, page_dir)

    def open_app(self, page_id: str, app_name: str):
        """Open an app."""
        return self.client.open_app(page_id, app_name)

    def list_spaces(self):
        """List spaces."""
        return self.client.list_spaces()

    def list_pages(self, space_id: str):
        """List pages in a space."""
        return self.client.list_pages(space_id)

    # ============================================================
    # Context manager support
    # ============================================================

    def __enter__(self) -> "Session":
        self.start()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.stop()
        return False  # Don't suppress exceptions
