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
import os
import shutil
import signal
import subprocess
import time
import traceback
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional

from .session import Session
from .client import ControlClient
from .tmux import TmuxManager
from .wait import wait_for_condition, wait_for_eval, TimeoutError


# Default paths
DEFAULT_KUNKI_BINARY = Path(__file__).parent.parent.parent / "target" / "debug" / "kunki"
DEFAULT_BASE_DIR = Path("/tmp/osvauld_test")


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
                "--db-path", f"{self.data_dir}/{self.name}",
                "init",
                "--username", username,
                "--passphrase", passphrase,
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
                "--db-path", f"{self.data_dir}/{self.name}",
                "start",
                "--passphrase", passphrase,
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
        raise TimeoutError(f"Timeout waiting for {desc} on '{self.name}' (after {timeout}s)")


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
        profiling: bool = False,
        flame_only: bool = False,
        heaptrack: bool = False,
        keep: bool = False,
        debug: bool = False,
    ):
        """
        Args:
            name: Session name (tmux session + base dir name)
            app_path: Path to the app directory (e.g., sample_apps/my-shop)
            peers: Dict mapping peer name to {"role": "owner"|"viewer", "app": "App Name"}
                   First peer with role "owner" creates the space.
            base_dir: Override base dir (default: /tmp/{name})
            release: Use release builds
            profiling: Enable tokio-console + flame profiling
            flame_only: Enable flame graphs only (no tokio-console overhead)
            heaptrack: Wrap binaries with heaptrack for heap profiling
            keep: Keep session alive after test (block until Ctrl+C)
            debug: Keep session on failure for debugging
        """
        self.name = name
        self.app_path = str(Path(app_path).resolve())
        self.peers_config = peers
        self.base_dir = Path(base_dir) if base_dir else Path(f"/tmp/{name}")
        self.release = release
        self.profiling = profiling
        self.flame_only = flame_only
        self.heaptrack = heaptrack
        self.keep = keep
        self.debug = debug

        self._tm: Optional[TmuxManager] = None
        self._handles: Dict[str, PeerHandle] = {}
        self._space_id: Optional[str] = None
        self._page_id: Optional[str] = None

    def peer(self, name: str) -> PeerHandle:
        """Get a PeerHandle by name."""
        if name not in self._handles:
            raise KeyError(f"Peer '{name}' not found. Available: {list(self._handles.keys())}")
        return self._handles[name]

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
        }

    @staticmethod
    def add_args(parser: argparse.ArgumentParser) -> None:
        """Add standard flags to an argparse parser."""
        parser.add_argument("--keep", action="store_true",
                            help="Keep tmux session alive after test")
        parser.add_argument("--debug", action="store_true",
                            help="Keep session on failure for debugging")
        parser.add_argument("--release", action="store_true",
                            help="Use release builds")

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

        if self._tm:
            self._tm.stop()

        return exc_type is not None  # Suppress exception if we handled it

    def _setup(self) -> None:
        """Run the full boilerplate setup."""
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
            profiling=self.profiling,
            flame_only=self.flame_only,
            heaptrack=self.heaptrack,
        )
        self._tm.add_node("node")
        self._tm.add_shell(owner_name)
        for pname, _ in viewer_peers:
            self._tm.add_shell(pname)
        self._tm.start()

        node = self._tm.get_client("node")
        owner_client = self._tm.get_client(owner_name)
        print("  All instances ready")

        # 2. Owner: signup, create space
        print(f"\n  {owner_name}: signup, create space...")
        owner_client.signup_or_login(owner_name)
        space = owner_client.create_space_with_pages(self.app_path)
        self._space_id = space["id"]

        pages = owner_client.list_pages(self._space_id)
        assert pages, "No pages after create_space_with_pages"
        self._page_id = pages[0]["id"]
        print(f"  Space: {self._space_id[:8]}..., Page: {self._page_id[:8]}...")

        # 3. Connect to node, publish
        print(f"  {owner_name}: connect to node, publish...")
        owner_client.connect_to_node(node)
        time.sleep(2)
        owner_client.publish_to_node(self._space_id)
        time.sleep(2)
        print("  Published")

        # 4. Setup each viewer
        for pname, pconfig in viewer_peers:
            viewer_client = self._tm.get_client(pname)
            print(f"  {pname}: signup, subscribe...")
            viewer_client.signup_or_login(pname)
            owner_client.add_viewer(viewer_client, self._space_id)

            # Wait for viewer to sync space
            viewer_page_id = None
            for i in range(30):
                time.sleep(0.5)
                spaces = viewer_client.list_spaces()
                if spaces:
                    viewer_pages = viewer_client.list_pages(spaces[0]["id"])
                    if viewer_pages:
                        viewer_page_id = viewer_pages[0]["id"]
                        print(f"  {pname} synced in {(i+1)*0.5:.1f}s")
                        break
            if not viewer_page_id:
                raise RuntimeError(f"{pname} failed to sync space")

        # 5. Open apps on all peers
        print("  Opening apps...")
        owner_client.open_app(self._page_id, owner_app)
        time.sleep(2)

        for pname, pconfig in viewer_peers:
            viewer_client = self._tm.get_client(pname)
            # Viewer's page_id may differ from owner's
            spaces = viewer_client.list_spaces()
            viewer_pages = viewer_client.list_pages(spaces[0]["id"])
            viewer_page_id = viewer_pages[0]["id"]
            viewer_client.open_app(viewer_page_id, pconfig["app"])
            time.sleep(1)
        print("  All apps opened")

        # 6. Wait for eval readiness on all peers
        print("  Waiting for eval readiness...")
        for pname in self.peers_config:
            client = self._tm.get_client(pname)
            for i in range(30):
                try:
                    client.eval("return 1")
                    break
                except Exception:
                    time.sleep(0.5)
            else:
                raise RuntimeError(f"{pname} eval not ready after 15s")

        # 7. Build PeerHandles
        for pname, pconfig in self.peers_config.items():
            client = self._tm.get_client(pname)
            # Resolve viewer page_id
            if pconfig["role"] == "owner":
                peer_page_id = self._page_id
            else:
                spaces = client.list_spaces()
                peer_pages = client.list_pages(spaces[0]["id"])
                peer_page_id = peer_pages[0]["id"]

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
