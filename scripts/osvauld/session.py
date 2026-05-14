"""
Session - Manages a single slint_shell instance

A Session encapsulates:
- Starting the shell process with control socket
- Waiting for the socket to be ready
- Providing a ControlClient for communication
- Cleanup on exit
"""

import os
import socket
import subprocess
import time
from pathlib import Path
from typing import List, Optional

from .client import ControlClient


# Default paths
DEFAULT_SHELL_BINARY = (
    Path(__file__).parent.parent.parent / "target" / "debug" / "sthalam"
)


class Session:
    """Manages a single sthalam instance."""

    def __init__(
        self,
        name: str,
        data_dir: str,
        socket_path: Optional[str] = None,
        shell_binary: Optional[Path] = None,
        startup_timeout: float = 5.0,
        show_ui: bool = False,
        extra_env: Optional[dict] = None,
    ):
        """
        Args:
            name: Instance name (used for logging, display name)
            data_dir: Directory for database storage (STHALAM_DATA_DIR)
            socket_path: Path for control socket (default: {data_dir}/{name}.sock)
            shell_binary: Path to slint_shell binary
            startup_timeout: How long to wait for socket to be ready
            show_ui: If True, don't set SLINT_BACKEND=testing (show actual UI)
            extra_env: Additional environment variables merged into the process env
        """
        self.name = name
        self.data_dir = data_dir
        self.socket_path = socket_path or f"{data_dir}/{name}.sock"
        self.shell_binary = shell_binary or DEFAULT_SHELL_BINARY
        self.startup_timeout = startup_timeout
        self.show_ui = show_ui
        self.extra_env = extra_env or {}

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
                "Run: cargo build -p sthalam"
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
        env.update(self.extra_env)

        # Start process
        self.process = subprocess.Popen(
            [
                str(self.shell_binary),
                "-d",
                self.name,
                "--debug-socket",
                self.socket_path,
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


# Default paths for vyakarana
DEFAULT_VYAKARANA_BINARY = (
    Path(__file__).parent.parent.parent.parent  # osvauld/../ == agent_x sibling
    / "agent_x"
    / "vyakarana"
    / "_build"
    / "default"
    / "bin"
    / "vyakarana.exe"
)
DEFAULT_VYAKARANA_CORPUS = Path(__file__).parent.parent.parent.parent / "agent_x"


class VyakaranaSession:
    """Manages a vyakarana OCaml engine instance in Unix socket mode.

    Spawns ``vyakarana.exe --socket <path> <corpus_dirs...>``, waits for
    the socket to appear and accept a test connection, then exposes the
    socket path for the renderer bridge to connect.

    Usage::

        with VyakaranaSession() as vy:
            # vy.socket_path is ready
            env = {"VYAKARANA_BIN": vy.binary, "VYAKARANA_CORPUS": vy.corpus_root}
            with Session("owner", data_dir, extra_env=env, show_ui=True) as s:
                ...

    The bridge in ``renderer_raylib`` / ``renderer_slint`` reads
    ``VYAKARANA_BIN`` and ``VYAKARANA_CORPUS`` from the environment, so
    setting those before launching sthalam is all that is needed.
    """

    def __init__(
        self,
        binary: Optional[Path] = None,
        corpus_root: Optional[Path] = None,
        socket_path: Optional[str] = None,
        startup_timeout: float = 20.0,
    ):
        """
        Args:
            binary: Path to vyakarana.exe (default: agent_x build location)
            corpus_root: Root directory containing brahman/ and sessions/
                         (default: agent_x directory)
            socket_path: Unix socket path the engine will listen on
                         (default: /tmp/vy_e2e.sock)
            startup_timeout: Seconds to wait for the engine to be ready
        """
        self.binary = str(binary or DEFAULT_VYAKARANA_BINARY)
        self.corpus_root = str(corpus_root or DEFAULT_VYAKARANA_CORPUS)
        self.socket_path = socket_path or "/tmp/vy_e2e.sock"
        self.startup_timeout = startup_timeout

        self.process: Optional[subprocess.Popen] = None

    # ------------------------------------------------------------------
    # Derived corpus dirs (same layout as renderer_raylib)
    # ------------------------------------------------------------------

    @property
    def corpus_dirs(self) -> List[str]:
        root = Path(self.corpus_root)
        dirs = []
        for sub in ("brahman/kosha", "brahman/sangati"):
            p = root / sub
            if p.exists():
                dirs.append(str(p))
        sessions = root / "sessions"
        if sessions.exists():
            for entry in sorted(sessions.iterdir()):
                if entry.is_dir():
                    dirs.append(str(entry))
        return dirs

    # ------------------------------------------------------------------
    # Lifecycle
    # ------------------------------------------------------------------

    def start(self) -> "VyakaranaSession":
        """Spawn the engine and wait until the socket is accepting connections.

        Returns:
            self (for chaining)

        Raises:
            FileNotFoundError: If the binary does not exist
            RuntimeError: If the engine fails to start within startup_timeout
        """
        if not Path(self.binary).exists():
            raise FileNotFoundError(
                f"vyakarana binary not found: {self.binary}\n"
                "Run: cd agent_x/vyakarana && dune build"
            )

        # Remove stale socket
        if os.path.exists(self.socket_path):
            os.remove(self.socket_path)

        cmd = [
            self.binary,
            "--socket",
            self.socket_path,
            "--quiet-startup",
        ] + self.corpus_dirs

        self.process = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

        # Wait for socket to appear and accept a connection
        start_time = time.time()
        while time.time() - start_time < self.startup_timeout:
            if os.path.exists(self.socket_path):
                try:
                    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
                        s.settimeout(1.0)
                        s.connect(self.socket_path)
                    return self
                except (ConnectionRefusedError, OSError):
                    pass
            # Check if process died early
            if self.process.poll() is not None:
                stderr = self.process.stderr.read().decode(errors="replace")
                raise RuntimeError(
                    f"vyakarana exited early (rc={self.process.returncode}): {stderr[:500]}"
                )
            time.sleep(0.1)

        self.stop()
        raise RuntimeError(
            f"vyakarana engine did not become ready within {self.startup_timeout}s"
        )

    def stop(self) -> None:
        """Terminate the engine process and remove the socket."""
        if self.process:
            self.process.terminate()
            try:
                self.process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
            self.process = None

        if os.path.exists(self.socket_path):
            try:
                os.remove(self.socket_path)
            except OSError:
                pass

    def is_running(self) -> bool:
        """Return True if the engine process is still alive."""
        return self.process is not None and self.process.poll() is None

    # ------------------------------------------------------------------
    # Context manager
    # ------------------------------------------------------------------

    def __enter__(self) -> "VyakaranaSession":
        self.start()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.stop()
        return False  # Don't suppress exceptions
