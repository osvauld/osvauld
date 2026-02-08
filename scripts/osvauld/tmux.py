"""
tmux - Manages instances in tmux panes

Library for spawning slint_shell and kunki instances in tmux panes.
Provides programmatic control over instance lifecycle.

Usage:
    from osvauld.tmux import TmuxManager

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

def get_binary_paths(release: bool = False, profiling: bool = False) -> tuple:
    """Get binary paths based on build profile.

    Args:
        release: Use release builds (fast, no debug symbols)
        profiling: Use profiling builds (fast + debug symbols for flamegraph)

    Returns:
        Tuple of (kunki_path, shell_path)
    """
    if profiling:
        target_dir = PROJECT_ROOT / "target" / "profiling"
    elif release:
        target_dir = PROJECT_ROOT / "target" / "release"
    else:
        target_dir = PROJECT_ROOT / "target" / "debug"

    return (target_dir / "kunki", target_dir / "sthalam")

DEFAULT_KUNKI_BINARY, DEFAULT_SHELL_BINARY = get_binary_paths()


class Instance:
    """Represents a single instance (shell or node)."""

    def __init__(
        self,
        name: str,
        data_dir: Path,
        socket_path: Path,
        instance_type: str,  # "shell" or "node"
        binary: Path,
        console_port: Optional[int] = None,
    ):
        self.name = name
        self.data_dir = data_dir
        self.socket_path = socket_path
        self.instance_type = instance_type
        self.binary = binary
        self.console_port = console_port
        self._client: Optional[ControlClient] = None
        self._pid: Optional[int] = None

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
        release: bool = False,
        profiling: bool = False,
        flame_only: bool = False,
        heaptrack: bool = False,
    ):
        """
        Args:
            session_name: Name for the tmux session
            base_dir: Base directory for all instance data
            passphrase: Default passphrase for kunki init
            shell_binary: Path to slint_shell binary
            kunki_binary: Path to kunki binary
            release: Use release builds (default: debug)
            profiling: Enable profiling mode (tokio-console + flame graphs)
            flame_only: Enable flame graphs only (no tokio-console overhead)
            heaptrack: Wrap binaries with heaptrack for heap profiling
        """
        self.session_name = session_name
        self.base_dir = Path(base_dir) if base_dir else Path("/tmp/p2p_test")
        self.passphrase = passphrase
        self.release = release
        self.profiling = profiling
        self.flame_only = flame_only
        self.heaptrack = heaptrack

        # Get binaries based on build profile
        if shell_binary and kunki_binary:
            self.shell_binary = shell_binary
            self.kunki_binary = kunki_binary
        else:
            self.kunki_binary, self.shell_binary = get_binary_paths(release, profiling or flame_only)

        self._instances: Dict[str, Instance] = {}
        self._instance_order: List[str] = []
        self._started = False
        self._next_console_port = 6669  # Starting port for tokio-console

    def add_shell(self, name: str, console_port: Optional[int] = None) -> "TmuxManager":
        """Add a slint_shell instance.

        Args:
            name: Instance name (used for display and directory)
            console_port: Port for tokio-console (auto-assigned if profiling and None)

        Returns:
            self (for chaining)
        """
        if name in self._instances:
            raise ValueError(f"Instance '{name}' already exists")

        data_dir = self.base_dir / name
        socket_path = data_dir / f"{name}.sock"

        # Auto-assign console port if profiling enabled
        if self.profiling and console_port is None:
            console_port = self._next_console_port
            self._next_console_port += 1

        self._instances[name] = Instance(
            name=name,
            data_dir=data_dir,
            socket_path=socket_path,
            instance_type="shell",
            binary=self.shell_binary,
            console_port=console_port,
        )
        self._instance_order.append(name)
        return self

    def add_node(self, name: str, console_port: Optional[int] = None) -> "TmuxManager":
        """Add a kunki node instance.

        Args:
            name: Instance name (used for display and directory)
            console_port: Port for tokio-console (auto-assigned if profiling and None)

        Returns:
            self (for chaining)
        """
        if name in self._instances:
            raise ValueError(f"Instance '{name}' already exists")

        data_dir = self.base_dir / name
        socket_path = data_dir / f"{name}.sock"

        # Auto-assign console port if profiling enabled
        if self.profiling and console_port is None:
            console_port = self._next_console_port
            self._next_console_port += 1

        self._instances[name] = Instance(
            name=name,
            data_dir=data_dir,
            socket_path=socket_path,
            instance_type="node",
            binary=self.kunki_binary,
            console_port=console_port,
        )
        self._instance_order.append(name)
        return self

    def check_binaries(self) -> bool:
        """Check that required binaries exist."""
        has_shell = any(i.instance_type == "shell" for i in self._instances.values())
        has_node = any(i.instance_type == "node" for i in self._instances.values())

        if has_shell and not self.shell_binary.exists():
            raise FileNotFoundError(
                f"sthalam binary not found: {self.shell_binary}\n"
                "Run: cargo build -p sthalam"
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

        # Spawn tokio-console panes if full profiling (not flame-only)
        if self.profiling and not self.flame_only:
            self._spawn_tokio_consoles()

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

    def get_instance_pid(self, name: str) -> Optional[int]:
        """Get PID of a running instance.

        Uses pgrep to find the process by its debug socket path,
        then filters to the actual binary (skips heaptrack/shell wrappers).
        """
        if name not in self._instances:
            return None

        inst = self._instances[name]
        try:
            # Find all processes matching the socket path
            result = subprocess.run(
                ['pgrep', '-f', str(inst.socket_path)],
                capture_output=True, text=True, timeout=2
            )
            if result.returncode != 0 or not result.stdout.strip():
                return None

            # Check each candidate PID — pick the actual binary, not wrappers
            binary_name = Path(inst.binary).name  # "sthalam" or "kunki"
            for pid_str in result.stdout.strip().split('\n'):
                pid = int(pid_str.strip())
                try:
                    comm = Path(f'/proc/{pid}/comm').read_text().strip()
                    if comm == binary_name:
                        return pid
                except (OSError, ValueError):
                    continue

            # Fallback: return newest PID if no binary name match
            pids = [int(p.strip()) for p in result.stdout.strip().split('\n')]
            return max(pids)
        except Exception:
            pass
        return None

    def get_all_pids(self) -> Dict[str, int]:
        """Get PIDs of all running instances."""
        pids = {}
        for name in self._instances:
            pid = self.get_instance_pid(name)
            if pid:
                pids[name] = pid
        return pids

    def get_console_ports(self) -> Dict[str, int]:
        """Get tokio-console ports for all instances (when profiling)."""
        ports = {}
        for name, inst in self._instances.items():
            if inst.console_port:
                ports[name] = inst.console_port
        return ports

    def print_console_info(self):
        """Print tokio-console connection info for all instances."""
        ports = self.get_console_ports()
        if ports:
            print("\n  tokio-console ports:")
            for name, port in sorted(ports.items()):
                print(f"    {name}: tokio-console http://localhost:{port}")

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

        # Set large scrollback buffer for debugging
        subprocess.run([
            "tmux", "set-option", "-t", self.session_name, "history-limit", "200000"
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

        # Pass through display environment for GUI apps (Raylib, Slint)
        # Support both X11 (DISPLAY) and Wayland (WAYLAND_DISPLAY, XDG_RUNTIME_DIR)
        env_vars = []
        for var in ["DISPLAY", "WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"]:
            val = os.environ.get(var, "")
            if val:
                env_vars.append(f"{var}={val}")

        # Add profiling env vars based on mode
        if self.flame_only:
            # Flame only: FLAME_OUTPUT without TOKIO_CONSOLE_PORT (no console overhead)
            flame_path = inst.data_dir / f"{inst.name}.folded"
            env_vars.append(f"FLAME_OUTPUT={flame_path}")
        elif self.profiling and inst.console_port:
            # Full profiling: both console + flame
            env_vars.append(f"TOKIO_CONSOLE_PORT={inst.console_port}")
            flame_path = inst.data_dir / f"{inst.name}.folded"
            env_vars.append(f"FLAME_OUTPUT={flame_path}")

        env_prefix = " ".join(env_vars) + " " if env_vars else ""

        # Heaptrack wrapper: saves heap profile to instance data dir
        heaptrack_prefix = ""
        if self.heaptrack:
            heaptrack_output = inst.data_dir / f"{inst.name}"
            heaptrack_prefix = f"heaptrack -o {heaptrack_output} "

        if inst.instance_type == "shell":
            cmd = (
                f"cd {PROJECT_ROOT} && "
                f"{env_prefix}"
                f"STHALAM_DATA_DIR={inst.data_dir} "
                f"{heaptrack_prefix}"
                f"{inst.binary} -d {inst.name} --debug-socket {inst.socket_path}"
            )
        else:  # node
            cmd = (
                f"cd {PROJECT_ROOT} && "
                f"{env_prefix}"
                f"{heaptrack_prefix}"
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

    def _spawn_tokio_consoles(self) -> None:
        """Spawn tokio-console panes for each instance when profiling."""
        if not self.profiling:
            return

        # Check if tokio-console is available
        result = subprocess.run(["which", "tokio-console"], capture_output=True)
        if result.returncode != 0:
            print("  [WARN] tokio-console not found. Install with: cargo install tokio-console")
            return

        # Create a new window for all tokio-consoles
        subprocess.run([
            "tmux", "new-window", "-t", self.session_name, "-n", "consoles"
        ])

        # Get the window index (it's the last one)
        console_window = len(self._instance_order)

        # Start first tokio-console
        first_inst = self._instances[self._instance_order[0]]
        if first_inst.console_port:
            cmd = f"tokio-console http://localhost:{first_inst.console_port}"
            subprocess.run([
                "tmux", "send-keys", "-t", f"{self.session_name}:{console_window}", cmd, "Enter"
            ])

        # Split and add remaining consoles
        for i, name in enumerate(self._instance_order[1:], 1):
            inst = self._instances[name]
            if inst.console_port:
                # Split horizontally for 2nd, vertically for others to make grid
                split_flag = "-h" if i == 1 else "-v"
                subprocess.run([
                    "tmux", "split-window", split_flag, "-t", f"{self.session_name}:{console_window}"
                ])
                cmd = f"tokio-console http://localhost:{inst.console_port}"
                subprocess.run([
                    "tmux", "send-keys", "-t", f"{self.session_name}:{console_window}", cmd, "Enter"
                ])

        # Even out the pane layout
        subprocess.run([
            "tmux", "select-layout", "-t", f"{self.session_name}:{console_window}", "tiled"
        ])

    def __enter__(self) -> "TmuxManager":
        self.start()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.stop()
        return False
