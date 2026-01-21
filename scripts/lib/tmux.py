"""
tmux - Manages instances in tmux panes

Library for spawning slint_shell and kunki instances in tmux panes.
Provides programmatic control over instance lifecycle.

Usage:
    from lib.tmux import TmuxManager

    # Start instances
    tm = TmuxManager(base_dir="/tmp/p2p_test")
    tm.add_shell("owner")
    tm.add_shell("viewer")
    tm.add_node("node")
    tm.start()

    # Get clients for interaction
    owner = tm.get_client("owner")
    owner.signup("user", "pass")

    # Cleanup
    tm.stop()
"""

import os
import shutil
import subprocess
import time
from pathlib import Path
from typing import Dict, List, Optional

from .client import ControlClient


# Default paths
PROJECT_ROOT = Path(__file__).parent.parent.parent
DEFAULT_KUNKI_BINARY = PROJECT_ROOT / "target" / "debug" / "kunki"
DEFAULT_SHELL_BINARY = PROJECT_ROOT / "target" / "debug" / "slint_shell"


class Instance:
    """Represents a single instance (shell or node)."""

    def __init__(
        self,
        name: str,
        data_dir: Path,
        socket_path: Path,
        instance_type: str,  # "shell" or "node"
        binary: Path,
    ):
        self.name = name
        self.data_dir = data_dir
        self.socket_path = socket_path
        self.instance_type = instance_type
        self.binary = binary
        self._client: Optional[ControlClient] = None

    @property
    def client(self) -> ControlClient:
        """Get control client (creates connection if needed)."""
        if self._client is None:
            if not self.socket_path.exists():
                raise RuntimeError(f"Instance '{self.name}' not started (socket not found)")
            self._client = ControlClient(str(self.socket_path))
        return self._client

    def is_ready(self) -> bool:
        """Check if instance is ready to receive commands."""
        if not self.socket_path.exists():
            return False
        try:
            client = ControlClient(str(self.socket_path))
            return client.ping()
        except (ConnectionError, RuntimeError):
            return False


class TmuxManager:
    """Manages instances in tmux panes."""

    def __init__(
        self,
        session_name: str = "p2p_test",
        base_dir: Optional[Path] = None,
        passphrase: str = "test",
        shell_binary: Optional[Path] = None,
        kunki_binary: Optional[Path] = None,
    ):
        """
        Args:
            session_name: Name for the tmux session
            base_dir: Base directory for all instance data
            passphrase: Default passphrase for kunki init
            shell_binary: Path to slint_shell binary
            kunki_binary: Path to kunki binary
        """
        self.session_name = session_name
        self.base_dir = Path(base_dir) if base_dir else Path("/tmp/p2p_test")
        self.passphrase = passphrase
        self.shell_binary = shell_binary or DEFAULT_SHELL_BINARY
        self.kunki_binary = kunki_binary or DEFAULT_KUNKI_BINARY

        self._instances: Dict[str, Instance] = {}
        self._instance_order: List[str] = []
        self._started = False

    def add_shell(self, name: str) -> "TmuxManager":
        """Add a slint_shell instance.

        Args:
            name: Instance name (used for display and directory)

        Returns:
            self (for chaining)
        """
        if name in self._instances:
            raise ValueError(f"Instance '{name}' already exists")

        data_dir = self.base_dir / name
        socket_path = data_dir / f"{name}.sock"

        self._instances[name] = Instance(
            name=name,
            data_dir=data_dir,
            socket_path=socket_path,
            instance_type="shell",
            binary=self.shell_binary,
        )
        self._instance_order.append(name)
        return self

    def add_node(self, name: str) -> "TmuxManager":
        """Add a kunki node instance.

        Args:
            name: Instance name (used for display and directory)

        Returns:
            self (for chaining)
        """
        if name in self._instances:
            raise ValueError(f"Instance '{name}' already exists")

        data_dir = self.base_dir / name
        socket_path = data_dir / f"{name}.sock"

        self._instances[name] = Instance(
            name=name,
            data_dir=data_dir,
            socket_path=socket_path,
            instance_type="node",
            binary=self.kunki_binary,
        )
        self._instance_order.append(name)
        return self

    def check_binaries(self) -> bool:
        """Check that required binaries exist."""
        has_shell = any(i.instance_type == "shell" for i in self._instances.values())
        has_node = any(i.instance_type == "node" for i in self._instances.values())

        if has_shell and not self.shell_binary.exists():
            raise FileNotFoundError(
                f"slint_shell binary not found: {self.shell_binary}\n"
                "Run: cargo build -p slint_shell"
            )
        if has_node and not self.kunki_binary.exists():
            raise FileNotFoundError(
                f"kunki binary not found: {self.kunki_binary}\n"
                "Run: cargo build -p kunki"
            )
        return True

    def start(self, fresh: bool = True, timeout: float = 30.0) -> "TmuxManager":
        """Start all instances in tmux panes.

        Args:
            fresh: If True, clean data directories before starting
            timeout: How long to wait for instances to be ready

        Returns:
            self (for chaining)
        """
        if self._started:
            return self

        if not self._instances:
            raise RuntimeError("No instances configured")

        self.check_binaries()

        # Kill existing tmux session
        subprocess.run(
            ["tmux", "kill-session", "-t", self.session_name],
            capture_output=True,
        )

        # Clean data directories if fresh
        if fresh and self.base_dir.exists():
            shutil.rmtree(self.base_dir)

        # Create data directories
        for inst in self._instances.values():
            inst.data_dir.mkdir(parents=True, exist_ok=True)

        # Initialize kunki nodes
        for inst in self._instances.values():
            if inst.instance_type == "node":
                self._init_node(inst)

        # Create tmux session
        self._create_tmux_session()

        # Start each instance in its pane
        for i, name in enumerate(self._instance_order):
            inst = self._instances[name]
            self._start_instance_in_window(inst, i)

        # Wait for all instances to be ready
        self._wait_for_ready(timeout)

        self._started = True
        return self

    def stop(self) -> None:
        """Stop all instances and kill tmux session."""
        subprocess.run(
            ["tmux", "kill-session", "-t", self.session_name],
            capture_output=True,
        )
        self._started = False

        # Clear client references
        for inst in self._instances.values():
            inst._client = None

    def get_client(self, name: str) -> ControlClient:
        """Get control client for an instance.

        Args:
            name: Instance name

        Returns:
            ControlClient for the instance
        """
        if name not in self._instances:
            raise ValueError(f"Instance '{name}' not found")
        return self._instances[name].client

    def get_socket_path(self, name: str) -> Path:
        """Get socket path for an instance."""
        if name not in self._instances:
            raise ValueError(f"Instance '{name}' not found")
        return self._instances[name].socket_path

    def list_instances(self) -> List[str]:
        """Get list of instance names in order."""
        return list(self._instance_order)

    def _init_node(self, inst: Instance) -> None:
        """Initialize a kunki node."""
        result = subprocess.run(
            [
                str(inst.binary),
                "--db-path", str(inst.data_dir / inst.name),
                "init",
                "--username", f"{inst.name}_user",
                "--passphrase", self.passphrase,
            ],
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            if "already exists" not in result.stderr.lower():
                raise RuntimeError(f"kunki init failed: {result.stderr}")

    def _create_tmux_session(self) -> None:
        """Create tmux session with separate windows (tabs) for each instance."""
        # Create session with first window named after first instance
        first_name = self._instance_order[0]
        subprocess.run([
            "tmux", "new-session", "-d", "-s", self.session_name, "-n", first_name
        ])

        # Create additional windows for remaining instances
        for name in self._instance_order[1:]:
            subprocess.run([
                "tmux", "new-window", "-t", self.session_name, "-n", name
            ])

    def _start_instance_in_window(self, inst: Instance, window_index: int) -> None:
        """Start an instance in a specific tmux window."""
        # Clean stale socket
        if inst.socket_path.exists():
            inst.socket_path.unlink()

        if inst.instance_type == "shell":
            cmd = (
                f"cd {PROJECT_ROOT} && "
                f"STHALAM_DATA_DIR={inst.data_dir} "
                f"{inst.binary} -d {inst.name} --debug-socket {inst.socket_path}"
            )
        else:  # node
            cmd = (
                f"cd {PROJECT_ROOT} && "
                f"{inst.binary} --db-path {inst.data_dir}/{inst.name} start "
                f"--passphrase {self.passphrase} --debug-socket {inst.socket_path}"
            )

        subprocess.run([
            "tmux", "send-keys", "-t", f"{self.session_name}:{window_index}", cmd, "Enter"
        ])

    def _wait_for_ready(self, timeout: float) -> None:
        """Wait for all instances to be ready."""
        start_time = time.time()
        ready = set()

        while time.time() - start_time < timeout:
            for name, inst in self._instances.items():
                if name not in ready and inst.is_ready():
                    ready.add(name)

            if len(ready) == len(self._instances):
                return

            time.sleep(0.5)

        # Report failures
        not_ready = set(self._instances.keys()) - ready
        if not_ready:
            raise RuntimeError(
                f"Instances failed to start: {not_ready}\n"
                f"Check tmux session: tmux attach -t {self.session_name}"
            )

    def __enter__(self) -> "TmuxManager":
        self.start()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.stop()
        return False
