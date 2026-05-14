"""
Scenario - Multi-peer orchestration for integration testing

A Scenario manages multiple sessions (owner, node, viewer) and handles:
- Spawning all required instances
- Setting up P2P connections between them
- Cleanup on exit

Usage:
    with Scenario(owner=1, node=1, viewer=2) as s:
        # s.owner - the owner session
        # s.node - the node session (kunki)
        # s.viewers - list of viewer sessions
        s.owner.eval("add_product('Widget', 10)")
        s.wait_sync()
        assert s.viewers[0].eval("return #get_products()") == 1
"""

import argparse
import contextlib
import json
import os
import shutil
import signal
import subprocess
import time
from datetime import datetime
from concurrent.futures import ThreadPoolExecutor, as_completed
import traceback
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional

from .session import Session
from .client import ControlClient
from .tmux import TmuxManager
from .wait import wait_for_condition, wait_for_eval, TimeoutError


# Default paths
DEFAULT_KUNKI_BINARY = (
    Path(__file__).parent.parent.parent / "target" / "debug" / "kunki"
)
DEFAULT_BASE_DIR = Path("/tmp/osvauld_test")


def _capture_ts_key(value: Any) -> Optional[float]:
    """Convert capture timestamp to a sortable float seconds value."""
    if isinstance(value, (int, float)):
        return float(value)

    if isinstance(value, str):
        try:
            return float(value)
        except ValueError:
            pass

        iso_value = value
        if iso_value.endswith("Z"):
            iso_value = iso_value[:-1] + "+00:00"
        try:
            return datetime.fromisoformat(iso_value).timestamp()
        except ValueError:
            return None

    return None


def _merge_capture_files(capture_paths: Dict[str, Path], output_path: Path) -> Path:
    """Merge per-instance capture JSONL files sorted by timestamp."""
    merged_entries = []
    seq = 0

    for instance, file_path in capture_paths.items():
        if not file_path.exists():
            continue

        with file_path.open("r", encoding="utf-8") as f:
            for line in f:
                raw = line.strip()
                if not raw:
                    continue

                ts_key = None
                out_line = raw
                try:
                    event = json.loads(raw)
                    if isinstance(event, dict):
                        event.setdefault("instance", instance)
                        ts_key = _capture_ts_key(event.get("ts"))
                        out_line = json.dumps(event, ensure_ascii=True)
                except json.JSONDecodeError:
                    pass

                merged_entries.append((ts_key is None, ts_key or 0.0, seq, out_line))
                seq += 1

    merged_entries.sort(key=lambda entry: (entry[0], entry[1], entry[2]))
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with output_path.open("w", encoding="utf-8") as f:
        for _, _, _, line in merged_entries:
            f.write(line + "\n")

    return output_path


class NodeSession:
    """Manages a kunki node instance."""

    def __init__(
        self,
        name: str,
        data_dir: str,
        socket_path: Optional[str] = None,
        kunki_binary: Optional[Path] = None,
        startup_timeout: float = 10.0,
    ):
        self.name = name
        self.data_dir = data_dir
        self.socket_path = socket_path or f"{data_dir}/{name}.sock"
        self.kunki_binary = kunki_binary or DEFAULT_KUNKI_BINARY
        self.startup_timeout = startup_timeout

        self.process: Optional[subprocess.Popen] = None
        self._client: Optional[ControlClient] = None
        self.connection_string: Optional[str] = None

    @property
    def client(self) -> ControlClient:
        """Get the ControlClient for this node."""
        if self._client is None:
            raise RuntimeError(f"Node '{self.name}' not started")
        return self._client

    def init(self, username: str, passphrase: str) -> None:
        """Initialize the node with a user identity."""
        if not self.kunki_binary.exists():
            raise FileNotFoundError(
                f"Kunki binary not found: {self.kunki_binary}\n"
                "Run: cargo build -p kunki"
            )

        # Ensure data directory exists
        os.makedirs(self.data_dir, exist_ok=True)

        result = subprocess.run(
            [
                str(self.kunki_binary),
                "--db-path",
                f"{self.data_dir}/{self.name}",
                "init",
                "--username",
                username,
                "--passphrase",
                passphrase,
            ],
            capture_output=True,
            text=True,
        )

        if result.returncode != 0:
            # Ignore "already exists" errors
            if "already exists" not in result.stderr.lower():
                raise RuntimeError(f"kunki init failed: {result.stderr}")

    def init_and_start(self, username: str = "node", passphrase: str = "test") -> str:
        """Initialize and start the node.

        Args:
            username: Username for init (default: "node")
            passphrase: Passphrase (default: "test")

        Returns:
            Connection string for this node
        """
        self.init(username, passphrase)
        self.start(passphrase)
        # connection_string is set during start() after node is ready
        return self.connection_string

    def start(self, passphrase: str) -> "NodeSession":
        """Start the kunki process."""
        if not self.kunki_binary.exists():
            raise FileNotFoundError(f"Kunki binary not found: {self.kunki_binary}")

        # Clean up stale socket
        if os.path.exists(self.socket_path):
            os.remove(self.socket_path)

        env = os.environ.copy()

        self.process = subprocess.Popen(
            [
                str(self.kunki_binary),
                "--db-path",
                f"{self.data_dir}/{self.name}",
                "start",
                "--passphrase",
                passphrase,
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
                        # Get connection string
                        self.connection_string = client.get_connection_string()
                        return self
                except (ConnectionError, RuntimeError):
                    pass
            time.sleep(0.2)

        self.stop()
        raise RuntimeError(f"Node '{self.name}' failed to start")

    def stop(self) -> None:
        """Stop the kunki process."""
        if self.process:
            self.process.terminate()
            try:
                self.process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
            self.process = None

        if os.path.exists(self.socket_path):
            os.remove(self.socket_path)

        self._client = None

    def __enter__(self) -> "NodeSession":
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.stop()
        return False


class Scenario:
    """Multi-peer test scenario orchestrator.

    Manages a collection of sessions (owner, node, viewers) and provides
    helpers for setting up P2P connections.
    """

    def __init__(
        self,
        owner: int = 0,
        node: int = 0,
        viewer: int = 0,
        base_dir: Optional[Path] = None,
        passphrase: str = "test123",
        fresh: bool = True,
        show_ui: bool = False,
    ):
        """
        Args:
            owner: Number of owner instances (typically 1)
            node: Number of node instances (kunki, typically 1)
            viewer: Number of viewer instances
            base_dir: Base directory for all data (default: /tmp/osvauld_test)
            passphrase: Passphrase for all users
            fresh: If True, clear base_dir before starting
            show_ui: If True, show UI windows (not headless)
        """
        self.owner_count = owner
        self.node_count = node
        self.viewer_count = viewer
        self.base_dir = Path(base_dir) if base_dir else DEFAULT_BASE_DIR
        self.passphrase = passphrase
        self.fresh = fresh
        self.show_ui = show_ui

        # Sessions will be populated on start
        self.owners: List[Session] = []
        self.nodes: List[NodeSession] = []
        self.viewers: List[Session] = []

        self._started = False
        self._capture_label: Optional[str] = None
        self._capture_paths: Dict[str, Path] = {}
        self._capture_clients: Dict[str, ControlClient] = {}

    @property
    def owner(self) -> Session:
        """Get the first (typically only) owner session."""
        if not self.owners:
            raise RuntimeError("No owner sessions configured")
        return self.owners[0]

    @property
    def node(self) -> NodeSession:
        """Get the first (typically only) node session."""
        if not self.nodes:
            raise RuntimeError("No node sessions configured")
        return self.nodes[0]

    @property
    def viewer(self) -> Session:
        """Get the first viewer session."""
        if not self.viewers:
            raise RuntimeError("No viewer sessions configured")
        return self.viewers[0]

    def start(self) -> "Scenario":
        """Start all configured sessions.

        Returns:
            self (for chaining)
        """
        if self._started:
            return self

        # Clean up if fresh
        if self.fresh and self.base_dir.exists():
            shutil.rmtree(self.base_dir)

        self.base_dir.mkdir(parents=True, exist_ok=True)

        try:
            # Start nodes first (they need to be running for others to connect)
            for i in range(self.node_count):
                name = f"node_{i}" if self.node_count > 1 else "node"
                data_dir = str(self.base_dir / name)

                node = NodeSession(name, data_dir)
                node.init(f"node_user_{i}", self.passphrase)
                node.start(self.passphrase)
                self.nodes.append(node)

            # Start owners
            for i in range(self.owner_count):
                name = f"owner_{i}" if self.owner_count > 1 else "owner"
                data_dir = str(self.base_dir / name)

                session = Session(name, data_dir, show_ui=self.show_ui)
                session.start()
                self.owners.append(session)

            # Start viewers
            for i in range(self.viewer_count):
                name = f"viewer_{i}" if self.viewer_count > 1 else "viewer"
                data_dir = str(self.base_dir / name)

                session = Session(name, data_dir, show_ui=self.show_ui)
                session.start()
                self.viewers.append(session)

            self._started = True
            return self

        except Exception:
            self.stop()
            raise

    def stop(self) -> None:
        """Stop all sessions."""
        for session in self.viewers:
            try:
                session.stop()
            except Exception:
                pass

        for session in self.owners:
            try:
                session.stop()
            except Exception:
                pass

        for node in self.nodes:
            try:
                node.stop()
            except Exception:
                pass

        self.owners = []
        self.nodes = []
        self.viewers = []
        self._started = False

    # ============================================================
    # Setup helpers
    # ============================================================

    def setup_owner(
        self,
        username: str = "owner",
        space_name: str = "Test Space",
        app_path: Optional[str] = None,
        app_name: Optional[str] = None,
    ) -> Dict:
        """Setup owner: signup, login, create space, open app.

        Args:
            username: Username for signup
            space_name: Name for the space
            app_path: Path to app directory (for create_space and import_page)
            app_name: Name of app to open (if specified)

        Returns:
            Dict with space_id, page_id
        """
        owner = self.owner

        # Signup and login
        owner.client.signup_or_login(username, self.passphrase)

        result = {"space_id": None, "page_id": None}

        # Create space if app_path provided
        if app_path:
            space = owner.create_space(space_name, app_path)
            result["space_id"] = space.get("id")

            # Import page
            page = owner.import_page(result["space_id"], app_path)
            result["page_id"] = page.get("page_id")

            # Open app if specified
            if app_name and result["page_id"]:
                owner.open_app(result["page_id"], app_name)
                wait_for_eval(owner)

        return result

    def connect_owner_to_node(self) -> str:
        """Connect owner to node and return node's connection string.

        Returns:
            Node's connection string
        """
        if not self.nodes:
            raise RuntimeError("No nodes available")

        conn_str = self.node.connection_string
        self.owner.client.add_node(conn_str)
        return conn_str

    def publish_to_node(self, space_id: str) -> None:
        """Publish a space to the first node."""
        if not self.nodes:
            raise RuntimeError("No nodes available")

        # Get node ID from connection string or node list
        nodes = self.owner.client.list_nodes()
        if not nodes:
            raise RuntimeError("Owner not connected to any nodes")

        node_id = nodes[0].get("node_id")  # API returns node_id, not id
        self.owner.client.publish_space(space_id, node_id)

    def get_viewer_link(self, space_id: str) -> str:
        """Get a shareable viewer link for a space.

        Returns:
            Viewer connection string
        """
        if not self.nodes:
            raise RuntimeError("No nodes available")

        nodes = self.owner.client.list_nodes()
        if not nodes:
            raise RuntimeError("Owner not connected to any nodes")

        node_id = nodes[0].get("node_id")  # API returns node_id, not id
        return self.owner.client.get_shareable_link(space_id, node_id)

    def setup_viewer(
        self,
        viewer_index: int = 0,
        connection_string: str = None,
        username: str = None,
    ) -> None:
        """Setup a viewer: signup, login, connect via connection string.

        Args:
            viewer_index: Which viewer to set up
            connection_string: Viewer link (from get_viewer_link)
            username: Username for signup
        """
        if viewer_index >= len(self.viewers):
            raise RuntimeError(f"Viewer {viewer_index} not available")

        viewer = self.viewers[viewer_index]
        username = username or f"viewer_{viewer_index}"

        # Signup and login
        viewer.client.signup_or_login(username, self.passphrase)

        # Connect to space
        if connection_string:
            viewer.client.add_website(connection_string)

    # ============================================================
    # Capture helpers
    # ============================================================

    def capture_start(self, label: str, include_logs: bool = False) -> Path:
        """Start capture on all instances (nodes + owners + viewers)."""
        if not self._started:
            raise RuntimeError("Scenario not started")
        if self._capture_label is not None:
            raise RuntimeError(
                f"Capture already active with label '{self._capture_label}'"
            )

        captures_dir = self.base_dir / "captures"
        captures_dir.mkdir(parents=True, exist_ok=True)

        capture_paths: Dict[str, Path] = {}
        capture_clients: Dict[str, ControlClient] = {}

        for node in self.nodes:
            capture_clients[node.name] = node.client

        for owner in self.owners:
            capture_clients[owner.name] = owner.client

        for viewer in self.viewers:
            capture_clients[viewer.name] = viewer.client

        for instance, client in capture_clients.items():
            capture_path = captures_dir / f"{label}_{instance}.jsonl"
            client.capture_start(str(capture_path), include_logs=include_logs)
            capture_paths[instance] = capture_path

        self._capture_label = label
        self._capture_paths = capture_paths
        self._capture_clients = capture_clients
        return captures_dir

    def capture_end(
        self, label: Optional[str] = None, merge: bool = True
    ) -> Optional[Path]:
        """Stop capture on all instances and optionally merge JSONL files."""
        if self._capture_label is None:
            raise RuntimeError("No active capture")
        if label is not None and label != self._capture_label:
            raise RuntimeError(
                f"Capture label mismatch: active '{self._capture_label}', requested '{label}'"
            )

        active_label = self._capture_label
        errors = []

        for instance, client in self._capture_clients.items():
            try:
                client.capture_end()
            except Exception as e:
                errors.append(f"{instance}: {e}")

        capture_paths = self._capture_paths
        self._capture_label = None
        self._capture_paths = {}
        self._capture_clients = {}

        if errors:
            raise RuntimeError("Capture end failed: " + "; ".join(errors))

        if not merge:
            return None

        merged_path = self.base_dir / "captures" / f"{active_label}_merged.jsonl"
        return _merge_capture_files(capture_paths, merged_path)

    # ============================================================
    # Sync helpers
    # ============================================================

    def wait_sync(self, timeout: float = 30.0) -> None:
        """Wait for sync to complete across all peers.

        This is a simple delay-based wait. For more precise sync verification,
        use wait.wait_for_sync() with specific values.
        """
        # TODO: Implement proper sync status checking via control server
        time.sleep(2.0)

    # ============================================================
    # Context manager
    # ============================================================

    def __enter__(self) -> "Scenario":
        self.start()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.stop()
        return False


class PeerHandle:
    """Thin wrapper over ControlClient for a peer in an AppTestScenario.

    Provides eval() and wait_for() for app-level testing.
    """

    def __init__(
        self,
        name: str,
        client: ControlClient,
        role: str,
        space_id: str,
        page_id: str,
        app_name: str,
    ):
        self.name = name
        self.client = client
        self.role = role
        self.space_id = space_id
        self.page_id = page_id
        self.app_name = app_name

    def eval(self, code: str) -> Any:
        """Execute Lua code and return the result."""
        return self.client.eval(code)

    def wait_for(
        self,
        condition_fn: Callable[[], Any],
        timeout: float = 15.0,
        interval: float = 0.5,
        desc: str = "condition",
    ) -> Any:
        """Poll until condition_fn returns a truthy value.

        Args:
            condition_fn: Callable returning truthy when done
            timeout: Max seconds to wait
            interval: Polling interval in seconds
            desc: Description for timeout error

        Returns:
            The truthy value from condition_fn

        Raises:
            TimeoutError: If condition not met within timeout
        """
        start = time.time()
        while time.time() - start < timeout:
            try:
                result = condition_fn()
                if result:
                    return result
            except Exception:
                pass
            time.sleep(interval)
        raise TimeoutError(
            f"Timeout waiting for {desc} on '{self.name}' (after {timeout}s)"
        )

    def set_time(self, unix_seconds: int) -> int:
        """Set runtime time to a specific Unix timestamp (test mode only).

        Args:
            unix_seconds: Unix timestamp in seconds

        Returns:
            New Unix timestamp
        """
        return self.client.set_time(unix_seconds)

    def advance_time(self, seconds: int) -> int:
        """Advance runtime time by a duration (test mode only).

        Args:
            seconds: Number of seconds to advance

        Returns:
            New Unix timestamp after advancing
        """
        return self.client.advance_time(seconds)

    def go_offline(self) -> Dict[str, Any]:
        """Simulate network loss for this peer without stopping process."""
        return self.client.go_offline()

    def go_online(self) -> Dict[str, Any]:
        """Restore network connectivity for this peer."""
        return self.client.go_online()

    # UI automation — synthetic pointer events on the live winit window.

    def ui_window_size(self) -> tuple:
        return self.client.ui_window_size()

    def ui_mouse_move(self, x: float, y: float) -> None:
        self.client.ui_mouse_move(x, y)

    def ui_mouse_press(self, x: float, y: float, button: str = "left") -> None:
        self.client.ui_mouse_press(x, y, button)

    def ui_mouse_release(self, x: float, y: float, button: str = "left") -> None:
        self.client.ui_mouse_release(x, y, button)

    def ui_mouse_drag(
        self,
        from_x: float,
        from_y: float,
        to_x: float,
        to_y: float,
        steps: int = 30,
        button: str = "left",
    ) -> Dict[str, Any]:
        """Drag from (from_x, from_y) to (to_x, to_y) over `steps` timer ticks
        (~100ms each). Reply fires after the press; motion happens
        asynchronously. Use `wait_for` to assert on post-drop state."""
        return self.client.ui_mouse_drag(from_x, from_y, to_x, to_y, steps, button)

    def ui_screenshot(self, path: str) -> str:
        return self.client.ui_screenshot(path)

    def ui_record_start(
        self,
        gif_path: str,
        states_path: str,
        captures: Optional[list] = None,
    ) -> Dict[str, Any]:
        return self.client.ui_record_start(gif_path, states_path, captures)

    def ui_record_stop(self) -> Dict[str, Any]:
        return self.client.ui_record_stop()

    @contextlib.contextmanager
    def ui_record(
        self,
        gif: str,
        states: str,
        captures: Optional[list] = None,
    ):
        """Context manager: start a recording on enter, flush on exit.

        Usage:
            with peer.ui_record(gif="/tmp/x.gif", states="/tmp/x.jsonl"):
                peer.ui_mouse_drag(...)
                peer.wait_for(...)
        """
        self.ui_record_start(gif, states, captures)
        try:
            yield
        finally:
            try:
                self.ui_record_stop()
            except Exception as e:
                # Don't mask the original exception in the with-body.
                print(f"[{self.name}] ui_record_stop failed: {e}")

    @staticmethod
    def read_record_states(path: str) -> List[Dict[str, Any]]:
        """Read a JSONL state log produced by ui_record."""
        out: List[Dict[str, Any]] = []
        with open(path) as f:
            for line in f:
                line = line.strip()
                if line:
                    out.append(json.loads(line))
        return out


class AppTestScenario:
    """One-call E2E test setup.

    Automates all boilerplate: TmuxManager, signup/login, create_space,
    connect/publish, add_viewer, open_app, wait for readiness.

    Usage:
        args = AppTestScenario.parse_args("My test")
        with AppTestScenario(
            name="my_test",
            app_path="sample_apps/my-shop",
            peers={
                "owner":    {"role": "owner",  "app": "Shop Owner"},
                "customer": {"role": "viewer", "app": "Shop Customer"},
            },
            **args,
        ) as s:
            owner = s.peer("owner")
            customer = s.peer("customer")
            owner.eval('add_product_via_ui("Widget", 99, "Test", 50)')
            customer.wait_for(
                lambda: customer.eval("return get_products_count()") >= 1,
                desc="product sync",
            )
    """

    def __init__(
        self,
        name: str,
        app_path: str,
        peers: Dict[str, Dict[str, str]],
        base_dir: Optional[str] = None,
        release: bool = False,
        test_mode: bool = False,
        profiling: bool = False,
        flame_only: bool = False,
        heaptrack: bool = False,
        keep: bool = False,
        debug: bool = False,
        fresh: bool = True,
        reuse_space_id: Optional[str] = None,
        reuse_page_id: Optional[str] = None,
    ):
        """
        Args:
            name: Session name (tmux session + base dir name)
            app_path: Path to the app directory (e.g., sample_apps/my-shop)
            peers: Dict mapping peer name to {"role": "owner"|"viewer", "app": "App Name"}
                   First peer with role "owner" creates the space.
            base_dir: Override base dir (default: /tmp/{name})
            release: Use release builds
            test_mode: Run peers with --test-mode (ManualClock)
            profiling: Enable tokio-console + flame profiling
            flame_only: Enable flame graphs only (no tokio-console overhead)
            heaptrack: Wrap binaries with heaptrack for heap profiling
            keep: Keep session alive after test (block until Ctrl+C)
            debug: Keep session on failure for debugging
            fresh: If True, clean data directories before starting (default: True)
            reuse_space_id: Reuse existing space ID instead of creating new (requires fresh=False)
            reuse_page_id: Reuse existing page ID instead of creating new (requires fresh=False)
        """
        self.name = name
        self.app_path = str(Path(app_path).resolve())
        self.peers_config = peers
        self.base_dir = Path(base_dir) if base_dir else Path(f"/tmp/{name}")
        self.release = release
        self.test_mode = test_mode
        self.profiling = profiling
        self.flame_only = flame_only
        self.heaptrack = heaptrack
        self.keep = keep
        self.debug = debug
        self.fresh = fresh
        self.reuse_space_id = reuse_space_id
        self.reuse_page_id = reuse_page_id

        # Validate reuse mode
        if (reuse_space_id or reuse_page_id) and fresh:
            raise ValueError("Cannot reuse space/page with fresh=True")

        self._tm: Optional[TmuxManager] = None
        self._handles: Dict[str, PeerHandle] = {}
        self._space_id: Optional[str] = None
        self._page_id: Optional[str] = None
        self._capture_label: Optional[str] = None
        self._capture_paths: Dict[str, Path] = {}

    def peer(self, name: str) -> PeerHandle:
        """Get a PeerHandle by name."""
        if name not in self._handles:
            raise KeyError(
                f"Peer '{name}' not found. Available: {list(self._handles.keys())}"
            )
        return self._handles[name]

    def stop_peer(self, name: str, timeout: float = 10.0) -> None:
        """Stop one peer process while keeping session alive."""
        if self._tm is None:
            raise RuntimeError("Scenario not started")
        self._tm.stop_instance(name, timeout=timeout)

    def stop_node(self, timeout: float = 10.0) -> None:
        """Stop the node process while keeping peers alive."""
        if self._tm is None:
            raise RuntimeError("Scenario not started")
        self._tm.stop_instance("node", timeout=timeout)

    def start_peer(
        self,
        name: str,
        timeout: float = 30.0,
        reconnect: bool = True,
    ) -> PeerHandle:
        """Start one peer process and re-open app for continued testing."""
        if self._tm is None:
            raise RuntimeError("Scenario not started")
        if name not in self.peers_config:
            raise KeyError(f"Unknown peer '{name}'")

        self._tm.start_instance(name, timeout=timeout)
        client = self._tm.get_client(name)
        role = self.peers_config[name]["role"]
        app_name = self.peers_config[name]["app"]

        client.signup_or_login(name)

        if role == "viewer" and reconnect:
            owner_name = next(
                pname
                for pname, pconfig in self.peers_config.items()
                if pconfig["role"] == "owner"
            )
            owner_client = self._tm.get_client(owner_name)
            conn_str = owner_client.get_connection_string()
            try:
                client.add_node(conn_str)
            except Exception:
                pass

            if self._space_id is not None:
                viewer_link = owner_client.get_viewer_link(self._space_id)
                try:
                    client.add_website(viewer_link)
                except Exception:
                    pass

        if self._page_id is None:
            raise RuntimeError("Page not created yet")
        self._open_app_ready(client, self._page_id, app_name)

        handle = PeerHandle(
            name=name,
            client=client,
            role=role,
            space_id=self.space_id,
            page_id=self.page_id,
            app_name=app_name,
        )
        self._handles[name] = handle
        return handle

    def start_node(self, timeout: float = 30.0) -> None:
        """Start the node process after stop_node()."""
        if self._tm is None:
            raise RuntimeError("Scenario not started")
        self._tm.start_instance("node", timeout=timeout)

    @property
    def owner(self) -> PeerHandle:
        """Get the first owner peer."""
        for handle in self._handles.values():
            if handle.role == "owner":
                return handle
        raise RuntimeError("No owner peer configured")

    @property
    def space_id(self) -> str:
        if self._space_id is None:
            raise RuntimeError("Space not created yet")
        return self._space_id

    @property
    def page_id(self) -> str:
        if self._page_id is None:
            raise RuntimeError("Page not created yet")
        return self._page_id

    @staticmethod
    def parse_args(description: str) -> dict:
        """Parse standard CLI args (--keep, --debug, --release).

        Returns:
            Dict suitable for passing as **kwargs to AppTestScenario().
        """
        parser = argparse.ArgumentParser(description=description)
        AppTestScenario.add_args(parser)
        args = parser.parse_args()
        return {
            "keep": args.keep,
            "debug": args.debug,
            "release": args.release,
            "test_mode": args.test_mode,
        }

    @staticmethod
    def add_args(parser: argparse.ArgumentParser) -> None:
        """Add standard flags to an argparse parser."""
        parser.add_argument(
            "--keep", action="store_true", help="Keep tmux session alive after test"
        )
        parser.add_argument(
            "--debug", action="store_true", help="Keep session on failure for debugging"
        )
        parser.add_argument("--release", action="store_true", help="Use release builds")
        parser.add_argument(
            "--test-mode",
            action="store_true",
            help="Run instances with ManualClock for deterministic time",
        )
        parser.add_argument(
            "--flame-only",
            action="store_true",
            help="Enable flame graphs (no tokio-console)",
        )
        parser.add_argument(
            "--heaptrack", action="store_true", help="Wrap binaries with heaptrack"
        )

    def __enter__(self) -> "AppTestScenario":
        self._setup()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if exc_type is not None:
            # Test failed
            print(f"\n[FAIL] {exc_val}")
            traceback.print_exception(exc_type, exc_val, exc_tb)

            if self.debug:
                self._print_debug_info()
                self._block_until_ctrl_c()

        if self.keep:
            self._print_session_info()
            self._block_until_ctrl_c()

        if self._capture_label is not None:
            try:
                self.capture_end(merge=False)
            except Exception:
                pass

        if self._tm:
            self._tm.stop()

        return exc_type is not None  # Suppress exception if we handled it

    def _setup(self) -> None:
        """Run full boilerplate setup (optimized for debug builds).

        Optimizations:
        - Conditional waits instead of fixed time.sleep()
        - Parallel viewer connection + space sync
        - Parallel app opening
        - Tighter polling (0.1s vs 0.5s)
        """
        # Determine owner and viewer peers
        owner_name = None
        owner_app = None
        viewer_peers = []

        for pname, pconfig in self.peers_config.items():
            if pconfig["role"] == "owner" and owner_name is None:
                owner_name = pname
                owner_app = pconfig["app"]
            else:
                viewer_peers.append((pname, pconfig))

        if owner_name is None:
            raise RuntimeError("At least one peer must have role 'owner'")

        # 1. Create TmuxManager and add instances
        print(f"\n{'=' * 60}")
        print(f"  {self.name}")
        print(f"{'=' * 60}")

        self._tm = TmuxManager(
            session_name=self.name,
            base_dir=self.base_dir,
            release=self.release,
            test_mode=self.test_mode,
            profiling=self.profiling,
            flame_only=self.flame_only,
            heaptrack=self.heaptrack,
        )
        self._tm.add_node("node")
        self._tm.add_shell(owner_name)
        for pname, _ in viewer_peers:
            self._tm.add_shell(pname)
        startup_timeout = (
            90.0 if (self.profiling or self.flame_only or self.heaptrack) else 30.0
        )
        self._tm.start(fresh=self.fresh, timeout=startup_timeout)

        node = self._tm.get_client("node")
        owner_client = self._tm.get_client(owner_name)
        print("  All instances ready")

        # 2. Owner: signup, create or reuse space
        owner_client.signup_or_login(owner_name)

        if self.reuse_space_id and self.reuse_page_id:
            # Reuse mode: use existing space/page from persisted data
            print(f"\n  {owner_name}: reusing existing space/page...")
            self._space_id = self.reuse_space_id
            self._page_id = self.reuse_page_id
            print(f"  Space: {self._space_id[:8]}..., Page: {self._page_id[:8]}...")
        else:
            # Fresh mode: create new space
            print(f"\n  {owner_name}: signup, create space...")
            space = owner_client.create_space_with_pages(self.app_path)
            self._space_id = space["id"]

            pages = owner_client.list_pages(self._space_id)
            assert pages, "No pages after create_space_with_pages"
            self._page_id = pages[0]["id"]
            print(f"  Space: {self._space_id[:8]}..., Page: {self._page_id[:8]}...")

        # 3. Connect to node (conditional wait, not fixed sleep)
        print(f"  {owner_name}: connect to node...")
        owner_client.connect_to_node(node)

        print("    Waiting for node authentication...")
        if not self._wait_for_node_auth(owner_client, node, timeout=10.0):
            raise RuntimeError(
                f"{owner_name} failed to authenticate with node within 10s"
            )
        print("    Node authenticated")

        # 4. Publish space (conditional wait, not fixed sleep)
        print(f"  {owner_name}: publish space to node...")
        owner_client.publish_to_node(self._space_id)

        if not self._wait_for_node_publish(node, self._space_id, timeout=10.0):
            raise RuntimeError(f"Node did not receive published space within 10s")
        print("  Published and synced to node")

        # 5. PARALLEL: Connect all viewers and wait for space sync.
        # If there are no viewers (single-peer test), skip this block.
        viewer_page_ids: Dict[str, str] = {}
        if not viewer_peers:
            print("\n  No viewers configured — skipping viewer connect step.")
        else:
            print(f"\n  Connecting viewers (parallel)...")
            viewer_link = owner_client.get_viewer_link(self._space_id)

            with ThreadPoolExecutor(max_workers=len(viewer_peers)) as executor:

                def connect_viewer(pname):
                    viewer_client = self._tm.get_client(pname)
                    viewer_client.signup_or_login(pname)
                    # Connect once and rely on _wait_for_space_sync as readiness gate.
                    # No redundant auth polling - sync wait is the proper gate.
                    viewer_client.add_website(viewer_link)
                    page_id = self._wait_for_space_sync(
                        viewer_client, pname, timeout=10.0
                    )
                    if page_id is None:
                        raise RuntimeError(
                            f"{pname} failed to sync space within 10s"
                        )
                    return pname, page_id

                future_to_peer = {
                    executor.submit(connect_viewer, pname): pname
                    for pname, _ in viewer_peers
                }

                for future in as_completed(future_to_peer.keys()):
                    pname = future_to_peer[future]
                    try:
                        peer_name, page_id = future.result()
                        viewer_page_ids[peer_name] = page_id
                    except Exception as e:
                        raise RuntimeError(f"{pname} connection failed: {e}")

            print("  All viewers connected and synced")

        # 6. PARALLEL: Open apps on all peers
        # Profiling wrappers (heaptrack/flame) slow startup; allow more time for Lua eval readiness.
        open_app_timeout = (
            15.0 if (self.profiling or self.flame_only or self.heaptrack) else 5.0
        )
        print(f"\n  Opening apps (parallel)...")

        with ThreadPoolExecutor(max_workers=len(self.peers_config)) as executor:

            def open_app(peer_name):
                client = self._tm.get_client(peer_name)
                pconfig = self.peers_config[peer_name]
                if pconfig["role"] == "owner":
                    page_id = self._page_id
                    app_name = owner_app
                else:
                    page_id = viewer_page_ids[peer_name]
                    app_name = pconfig["app"]

                # Wait for app layer to sync (viewers need this)
                if pconfig["role"] == "viewer":
                    if not self._wait_for_app_sync(
                        client, page_id, app_name, peer_name, timeout=10.0
                    ):
                        raise RuntimeError(
                            f"{peer_name} failed to sync app '{app_name}' within 10s"
                        )

                self._open_app_ready(
                    client, page_id, app_name, timeout=open_app_timeout
                )
                return peer_name

            future_to_peer = {
                executor.submit(open_app, peer_name): peer_name
                for peer_name in self.peers_config.keys()
            }

            for future in as_completed(future_to_peer.keys()):
                peer_name = future_to_peer[future]
                try:
                    future.result()
                    print(f"    {peer_name} app ready")
                except Exception as e:
                    raise RuntimeError(f"{peer_name} app failed: {e}")

        print("  All apps opened and ready")

        # 7. Build PeerHandles
        print(f"\n  Building peer handles...")
        for pname, pconfig in self.peers_config.items():
            client = self._tm.get_client(pname)
            # Resolve viewer page_id
            if pconfig["role"] == "owner":
                peer_page_id = self._page_id
            else:
                peer_page_id = viewer_page_ids[pname]

            self._handles[pname] = PeerHandle(
                name=pname,
                client=client,
                role=pconfig["role"],
                space_id=self._space_id,
                page_id=peer_page_id,
                app_name=pconfig["app"],
            )

        print(f"\n  Ready! Peers: {list(self._handles.keys())}")
        print(f"{'=' * 60}\n")

    def capture_start(self, label: str, include_logs: bool = False) -> Path:
        """Start capture on node + all configured peers.

        Returns:
            Path to the captures directory.
        """
        if self._tm is None:
            raise RuntimeError("Scenario not set up yet")
        if self._capture_label is not None:
            raise RuntimeError(
                f"Capture already active with label '{self._capture_label}'"
            )

        captures_dir = self.base_dir / "captures"
        captures_dir.mkdir(parents=True, exist_ok=True)

        capture_paths: Dict[str, Path] = {}

        instance_names = ["node", *self.peers_config.keys()]
        for instance in instance_names:
            client = self._tm.get_client(instance)
            capture_path = captures_dir / f"{label}_{instance}.jsonl"
            client.capture_start(str(capture_path), include_logs=include_logs)
            capture_paths[instance] = capture_path

        self._capture_label = label
        self._capture_paths = capture_paths
        return captures_dir

    def capture_end(
        self, label: Optional[str] = None, merge: bool = True
    ) -> Optional[Path]:
        """Stop capture on node + peers and optionally return merged file path."""
        if self._tm is None:
            raise RuntimeError("Scenario not set up yet")
        if self._capture_label is None:
            raise RuntimeError("No active capture")
        if label is not None and label != self._capture_label:
            raise RuntimeError(
                f"Capture label mismatch: active '{self._capture_label}', requested '{label}'"
            )

        active_label = self._capture_label
        errors = []
        for instance in ["node", *self.peers_config.keys()]:
            try:
                client = self._tm.get_client(instance)
                client.capture_end()
            except Exception as e:
                errors.append(f"{instance}: {e}")

        capture_paths = self._capture_paths
        self._capture_label = None
        self._capture_paths = {}

        if errors:
            raise RuntimeError("Capture end failed: " + "; ".join(errors))

        if not merge:
            return None

        merged_path = self.base_dir / "captures" / f"{active_label}_merged.jsonl"
        return _merge_capture_files(capture_paths, merged_path)

    def _wait_for_space_sync(self, client, peer_name, timeout=10.0):
        """Wait for peer to sync space and page from node.

        Polls every 0.1s instead of 0.5s (5x faster detection).

        Args:
            client: ControlClient for the peer
            peer_name: Peer name for logging
            timeout: Max seconds to wait (default: 10.0)

        Returns:
            page_id if synced, None if timeout
        """
        start = time.time()
        last_print = start

        while time.time() - start < timeout:
            try:
                spaces = client.list_spaces()
                if spaces:
                    pages = client.list_pages(spaces[0]["id"])
                    if pages:
                        elapsed = time.time() - start
                        print(f"  {peer_name} synced in {elapsed:.1f}s")
                        return pages[0]["id"]
            except Exception:
                pass

            # Progress logging every 2s
            if time.time() - start - last_print >= 2.0:
                print(f"  {peer_name}: still waiting... ({time.time() - start:.1f}s)")
                last_print = time.time() - start

            time.sleep(0.1)

        return None

    def _wait_for_app_sync(self, client, page_id, app_name, peer_name, timeout=10.0):
        """Wait for app layer to sync from node.

        App layers arrive via SyncOffer after space/page metadata.
        This waits for the app to appear in list_apps().

        Args:
            client: ControlClient for the peer
            page_id: Page ID to check
            app_name: App name to wait for
            peer_name: Peer name for logging
            timeout: Max seconds to wait (default: 10.0)

        Returns:
            True if app synced, False if timeout
        """
        start = time.time()
        last_print = start

        while time.time() - start < timeout:
            try:
                apps = client.list_apps(page_id)
                app_names = [a.get("name") for a in apps]
                if app_name in app_names:
                    elapsed = time.time() - start
                    print(f"  {peer_name} app '{app_name}' synced in {elapsed:.1f}s")
                    return True
            except Exception:
                pass

            # Progress logging every 2s
            if time.time() - start - last_print >= 2.0:
                print(
                    f"  {peer_name}: waiting for app '{app_name}'... ({time.time() - start:.1f}s)"
                )
                last_print = time.time() - start

            time.sleep(0.1)

        return False

    def _wait_for_node_auth(self, owner_client, node_client, timeout=10.0):
        """Wait for owner to authenticate with node after connect_to_node.

        Replaces blind time.sleep(2) - polls at 0.1s intervals.

        Args:
            owner_client: ControlClient for owner
            node_client: ControlClient for node
            timeout: Max seconds to wait (default: 10.0)

        Returns:
            True if authenticated, False if timeout
        """
        start = time.time()

        while time.time() - start < timeout:
            try:
                nodes = owner_client.list_nodes()
                if nodes:
                    node_id = nodes[0].get("node_id")
                    if owner_client.is_node_authenticated(node_id):
                        return True
            except Exception:
                pass
            time.sleep(0.1)

        return False

    def _wait_for_node_publish(self, node_client, space_id, timeout=10.0):
        """Wait for node to have published space with page.

        Replaces blind time.sleep(2) - polls at 0.1s intervals.

        Args:
            node_client: ControlClient for node
            space_id: Space ID to check
            timeout: Max seconds to wait (default: 10.0)

        Returns:
            True if space/page synced, False if timeout
        """
        start = time.time()

        while time.time() - start < timeout:
            try:
                spaces = node_client.list_spaces()
                for space in spaces:
                    if space.get("id") == space_id:
                        pages = node_client.list_pages(space_id)
                        if pages:
                            return True
            except Exception:
                pass
            time.sleep(0.1)

        return False

    def _open_app_ready(self, client, page_id, app_name, timeout=5.0):
        """Open app and wait for Lua runtime to be ready.

        Combines open_app + wait_for_eval with 0.1s polling.

        Args:
            client: ControlClient for peer
            page_id: Page ID to open
            app_name: App name to open
            timeout: Max seconds to wait for eval (default: 5.0)

        Raises:
            RuntimeError: If app doesn't become ready
        """
        client.open_app(page_id, app_name)

        # Wait for eval readiness with tight polling
        start = time.time()
        while time.time() - start < timeout:
            try:
                result = client.eval("return 1")
                if result == 1:
                    return
            except Exception:
                pass
            time.sleep(0.1)

        raise RuntimeError(f"App '{app_name}' not ready after {timeout}s")

    def _print_session_info(self) -> None:
        """Print tmux session and socket info."""
        print(f"\n{'=' * 60}")
        print(f"  Session kept alive: tmux attach -t {self.name}")
        print(f"{'=' * 60}")
        print(f"\n  Sockets:")
        for pname in self.peers_config:
            path = self._tm.get_socket_path(pname)
            print(f"    {pname:12s}: {path}")
        node_path = self._tm.get_socket_path("node")
        print(f"    {'node':12s}: {node_path}")

    def _print_debug_info(self) -> None:
        """Print debug state on failure."""
        self._print_session_info()
        print(f"\n  Debug state:")
        for pname in self.peers_config:
            try:
                client = self._tm.get_client(pname)
                state = client.get_state()
                print(f"    {pname}: {state}")
            except Exception as e:
                print(f"    {pname}: error getting state: {e}")

    def _block_until_ctrl_c(self) -> None:
        """Block until user presses Ctrl+C."""
        print("\n  Press Ctrl+C to stop and cleanup")
        try:
            signal.pause()
        except KeyboardInterrupt:
            pass
