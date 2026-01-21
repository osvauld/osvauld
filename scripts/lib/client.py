"""
ControlClient - Socket communication with slint_shell control server

The control server uses JSON-RPC style protocol:
    Request:  {"method": "eval", "params": {"code": "return 1+1"}, "id": 1}
    Response: {"result": 2, "id": 1}
    Error:    {"error": {"message": "..."}, "id": 1}
"""

import socket
import json
from typing import Any, Dict, Optional


class ControlClient:
    """Client for communicating with slint_shell control socket."""

    def __init__(self, socket_path: str, timeout: float = 30.0):
        """
        Args:
            socket_path: Path to Unix socket (e.g., /tmp/test/owner.sock)
            timeout: Command timeout in seconds
        """
        self.socket_path = socket_path
        self.timeout = timeout
        self._id_counter = 0

    def _next_id(self) -> int:
        self._id_counter += 1
        return self._id_counter

    def send(self, method: str, params: Optional[Dict] = None) -> Any:
        """Send a command and return the result.

        Args:
            method: Command method name (e.g., "eval", "login", "list_spaces")
            params: Optional parameters dict

        Returns:
            The result field from the response

        Raises:
            ConnectionError: If socket connection fails
            RuntimeError: If server returns an error
        """
        cmd: Dict[str, Any] = {"method": method, "id": self._next_id()}
        if params:
            cmd["params"] = params

        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        try:
            sock.connect(self.socket_path)
            sock.settimeout(self.timeout)
            sock.send((json.dumps(cmd) + '\n').encode())

            # Read response (may be large)
            chunks = []
            while True:
                chunk = sock.recv(65536)
                if not chunk:
                    break
                chunks.append(chunk.decode())
                # Check if we have complete JSON
                try:
                    response = json.loads(''.join(chunks))
                    break
                except json.JSONDecodeError:
                    continue

            response = json.loads(''.join(chunks))
        except socket.error as e:
            raise ConnectionError(f"Socket error: {e}")
        finally:
            sock.close()

        # Check for error
        if "error" in response:
            err = response["error"]
            msg = err.get("message", str(err)) if isinstance(err, dict) else str(err)
            raise RuntimeError(f"{method} failed: {msg}")

        return response.get("result")

    # ============================================================
    # Auth commands
    # ============================================================

    def ping(self) -> bool:
        """Check if the control server is responding."""
        result = self.send("ping")
        return result.get("status") == "ok" if isinstance(result, dict) else False

    def signup(self, username: str, passphrase: str) -> Dict:
        """Sign up a new user."""
        return self.send("sign_up", {"username": username, "passphrase": passphrase})

    def login(self, passphrase: str) -> Dict:
        """Login with passphrase."""
        return self.send("login", {"passphrase": passphrase})

    def signup_or_login(self, username: str, passphrase: str = "test", wait_p2p: bool = True) -> Dict:
        """Sign up if new user, then login.

        Args:
            username: Username for signup
            passphrase: Passphrase (default: "test")
            wait_p2p: Wait for P2P to initialize after login (default: True)

        Returns:
            Login result
        """
        try:
            self.signup(username, passphrase)
        except RuntimeError as e:
            # Ignore "user exists" errors
            if "exists" not in str(e).lower():
                raise
        result = self.login(passphrase)

        # Wait for P2P to initialize
        if wait_p2p:
            import time
            for _ in range(30):  # Up to 30 seconds
                status = self.p2p_status()
                if status.get("p2p_ready"):
                    break
                time.sleep(1)

        return result

    # ============================================================
    # Space/Page/App commands
    # ============================================================

    def create_space_with_pages(self, path: str, name: str = None) -> Dict:
        """Create a space and import pages from path.

        Args:
            path: Path to app directory (used as template and for page import)
            name: Space name (default: derived from path)

        Returns:
            Dict with id, name, pages (list of page info)

        Note: Slint files are validated server-side before import.
        """
        from pathlib import Path as P
        app_path = P(path)

        # Derive name from path if not provided
        if not name:
            name = app_path.name

        # Create space
        space = self.create_space(name, str(app_path))
        space_id = space.get("id")

        # Import page
        page = self.import_page(space_id, str(app_path))

        return {
            "id": space_id,
            "name": space.get("name", name),
            "pages": [page],
        }

    def list_spaces(self) -> list:
        """List all spaces."""
        result = self.send("list_spaces")
        return result.get("spaces", []) if isinstance(result, dict) else []

    def list_pages(self, space_id: str) -> list:
        """List pages in a space."""
        result = self.send("list_pages", {"space_id": space_id})
        return result.get("pages", []) if isinstance(result, dict) else []

    def list_apps(self, page_id: str) -> list:
        """List apps in a page."""
        result = self.send("list_apps", {"page_id": page_id})
        return result.get("apps", []) if isinstance(result, dict) else []

    def create_space(self, name: str, template_path: str) -> Dict:
        """Create a new space from a template directory."""
        return self.send("create_space", {"name": name, "template_path": template_path})

    def import_page(self, space_id: str, page_dir: str) -> Dict:
        """Import a page from a directory."""
        return self.send("import_page", {"space_id": space_id, "page_dir": page_dir})

    def open_app(self, page_id: str, app_name: str) -> Dict:
        """Open an app by name."""
        return self.send("open_app", {"page_id": page_id, "app_name": app_name})

    def refresh_app(self, app_name: str, app_dir: str) -> Dict:
        """Refresh an app from filesystem."""
        return self.send("refresh_app", {"app_name": app_name, "app_dir": app_dir})

    def refresh_page(self, page_dir: str) -> Dict:
        """Refresh all apps in a page directory."""
        return self.send("refresh_page", {"page_dir": page_dir})

    # ============================================================
    # P2P commands
    # ============================================================

    def add_node(self, connection_string: str) -> Dict:
        """Add a sovereign node by connection string."""
        return self.send("add_node", {"connection_string": connection_string})

    def connect_to_node(self, node) -> Dict:
        """Connect to a node.

        Args:
            node: NodeSession (has connection_string) or ControlClient (has get_connection_string())

        Returns:
            Dict with node_id and connection info
        """
        if hasattr(node, 'connection_string') and node.connection_string:
            conn_str = node.connection_string
        elif hasattr(node, 'get_connection_string'):
            conn_str = node.get_connection_string()
        else:
            raise ValueError("node must have connection_string or get_connection_string()")
        return self.add_node(conn_str)

    def list_nodes(self) -> list:
        """List connected nodes."""
        result = self.send("list_nodes")
        return result.get("nodes", []) if isinstance(result, dict) else []

    def publish_space(self, space_id: str, node_id: str) -> Dict:
        """Publish a space to a node."""
        return self.send("publish_space", {"space_id": space_id, "node_id": node_id})

    def publish_to_node(self, space_id: str, node_index: int = 0) -> Dict:
        """Publish a space to a connected node.

        Args:
            space_id: Space ID to publish
            node_index: Which node to publish to (default: first)

        Returns:
            Dict with publish result
        """
        nodes = self.list_nodes()
        if not nodes:
            raise RuntimeError("No nodes connected")
        if node_index >= len(nodes):
            raise RuntimeError(f"Node index {node_index} out of range (have {len(nodes)})")
        node_id = nodes[node_index].get("node_id")
        return self.publish_space(space_id, node_id)

    def get_shareable_link(self, space_id: str, node_id: str) -> str:
        """Get a shareable viewer link for a space."""
        result = self.send("get_shareable_link", {"space_id": space_id, "node_id": node_id})
        return result.get("connection_string", "") if isinstance(result, dict) else ""

    def get_viewer_link(self, space_id: str, node_index: int = 0) -> str:
        """Get shareable viewer link for a space.

        Args:
            space_id: Space ID
            node_index: Which node to get link from (default: first)

        Returns:
            Connection string for viewers
        """
        nodes = self.list_nodes()
        if not nodes:
            raise RuntimeError("No nodes connected")
        if node_index >= len(nodes):
            raise RuntimeError(f"Node index {node_index} out of range (have {len(nodes)})")
        node_id = nodes[node_index].get("node_id")
        return self.get_shareable_link(space_id, node_id)

    def add_website(self, connection_string: str) -> Dict:
        """Connect to a space as viewer."""
        return self.send("add_website", {"connection_string": connection_string})

    def add_viewer(self, viewer: "ControlClient", space_id: str) -> Dict:
        """Add a viewer to a space.

        Args:
            viewer: ControlClient for the viewer (must be logged in)
            space_id: Space ID to share

        Returns:
            Result from viewer's add_website (includes space_id)
        """
        link = self.get_viewer_link(space_id)
        return viewer.add_website(link)

    def get_connection_string(self) -> str:
        """Get this instance's connection string."""
        result = self.send("get_connection_string")
        return result.get("connection_string", "") if isinstance(result, dict) else ""

    def p2p_status(self) -> Dict:
        """Get P2P connection status."""
        return self.send("p2p_status")

    # ============================================================
    # Lua evaluation
    # ============================================================

    def eval(self, code: str) -> Any:
        """Execute Lua code and return the result.

        Args:
            code: Lua code to execute (should include 'return' for results)

        Returns:
            The evaluated result (converted from Lua to Python types)
        """
        return self.send("eval", {"code": code})

    # ============================================================
    # UI automation
    # ============================================================

    def ui_click(self, label: str) -> None:
        """Click an element by accessible-label."""
        self.send("ui_click", {"label": label})

    def ui_type(self, label: str, text: str) -> None:
        """Type text into an element by accessible-label."""
        self.send("ui_type", {"label": label, "text": text})

    def ui_get_text(self, label: str) -> str:
        """Get text from an element by accessible-label."""
        result = self.send("ui_get_text", {"label": label})
        return result.get("value", "") if isinstance(result, dict) else str(result)

    def ui_get_screen(self) -> str:
        """Get the current screen name."""
        result = self.send("ui_get_screen")
        return result.get("screen", "") if isinstance(result, dict) else ""

    def ui_list_elements(self) -> list:
        """List all accessible UI elements (for debugging)."""
        result = self.send("ui_list_elements")
        return result.get("elements", []) if isinstance(result, dict) else []

    # ============================================================
    # Asset commands
    # ============================================================

    def upload_asset(self, page_id: str, file_path: str) -> Dict:
        """Upload an asset file to a page."""
        return self.send("upload_asset", {"page_id": page_id, "file_path": file_path})

    # ============================================================
    # Misc
    # ============================================================

    def get_app_status(self) -> Dict:
        """Get current app loading status."""
        return self.send("get_app_status")

    def get_state(self) -> Dict:
        """Get current state snapshot."""
        return self.send("state")

    def get_logs(self, last: int = 100, level: Optional[str] = None) -> list:
        """Get recent logs."""
        params = {"last": last}
        if level:
            params["level"] = level
        result = self.send("logs", params)
        return result.get("logs", []) if isinstance(result, dict) else []
