# Testing

Control server API, Python test client, and writing test scripts.

## Control Server

Each running sthalam (desktop) or kunki (node) instance exposes a Unix socket for JSON-RPC communication. This is the primary interface for automated testing.

```python
from osvauld.client import ControlClient

# Auto-discover running instance by name
client = ControlClient.discover("owner")

# Or explicit socket path
client = ControlClient("/tmp/osvauld-debug-owner.sock")
```

## Python Client API

The `ControlClient` class in `scripts/osvauld/client.py` provides all control operations.

### Auth

```python
client.ping()                                    # Check server is responding
client.signup(username, passphrase)             # Register new user
client.login(passphrase)                         # Login
client.signup_or_login(username, passphrase)     # Signup if needed, then login
```

### Space / Page / App

```python
client.create_space(name, template_path)         # Create a space
client.import_page(space_id, page_dir)           # Import a page into a space
client.create_space_with_pages(app_path, name)   # Create space + import all pages
client.list_spaces()                             # List all spaces
client.list_pages(space_id)                      # List pages in a space
client.list_apps(page_id)                        # List apps in a page
client.open_app(page_id, app_name)               # Open an app
client.refresh_app(app_name, app_dir)            # Refresh one app dir (uses current selected page)
client.refresh_page(page_dir)                    # Refresh all app dirs under a page directory
```

### App Refresh Semantics

- App refresh is offline-first: files are read locally, committed to Scribe first, then synced asynchronously.
- `refresh_app` and `refresh_page` both flow through Scribe so running apps receive `LayerChanged` and restart in place when needed.
- If peers are offline, they catch up on reconnect through normal sync (no request/response fetch path).
- If an app is not currently open on a peer, refreshed files still sync and are used next time the app is opened.
- `refresh_app` targets the shell's current page id; ensure the desired page is selected/open before calling it.

### P2P

```python
client.add_node(connection_string)               # Add a node
client.connect_to_node(node_client)              # Connect to a node (auto)
client.list_nodes()                              # List connected nodes
client.publish_space(space_id, node_id)          # Publish space to node
client.publish_to_node(space_id)                 # Publish (auto-selects node)
client.get_shareable_link(space_id, node_id)     # Get viewer link
client.get_viewer_link(space_id)                 # Get viewer link (auto-selects node)
client.add_viewer(viewer_client, space_id)       # Add a viewer
client.add_website(connection_string)            # Connect as viewer
client.get_connection_string()                   # This instance's connection string
client.p2p_status()                              # Check P2P readiness
```

### Lua Execution

```python
result = client.eval(lua_code)                   # Execute Lua, return result
# Example:
count = client.eval("return products_layer:length()")
client.eval('add_product_via_ui("Widget", 99)')
```

### UI Automation

```python
client.ui_click(label)                           # Click by accessible-label
client.ui_type(label, text)                      # Type into input
client.ui_get_text(label)                        # Read text value
client.ui_get_screen()                           # Current screen name
client.ui_list_elements()                        # List all UI elements
```

### Wait Helpers

```python
# Wait for a condition to become true
client.wait_until(
    lambda: client.eval("return #products > 0"),
    timeout=10,
    poll=0.1
)

# Wait for data to sync to another instance
client.wait_for_sync(
    other_client,
    "return #products > 0",
    timeout=10
)
```

### Debugging

```python
client.get_logs(last=100, level=None)            # Get recent logs
client.get_state()                               # State snapshot
client.get_app_status()                          # App loading status
```

## Session Management

The `Session` class in `scripts/osvauld/session.py` manages a single sthalam instance:

```python
from osvauld.session import Session

session = Session(
    name="owner",
    data_dir="/tmp/test/owner",
    show_ui=True,        # False for headless testing
)

# Context manager for automatic cleanup
with Session("owner", "/tmp/test/owner") as s:
    s.client.signup_or_login("alice", "pass")
    s.eval("return 1 + 1")
    # ... test logic
# Process automatically stopped on exit
```

## NodeSession

The `NodeSession` class in `scripts/osvauld/scenario.py` manages a kunki node instance:

```python
from osvauld.scenario import NodeSession

node = NodeSession(
    name="node",
    data_dir="/tmp/test/node"
)
node.init("node_user", "passphrase")     # Initialize identity
node.start("passphrase")                  # Start the node process
conn_str = node.connection_string         # Get connection string for peers
node.stop()                               # Stop the process
```

## Scenario Orchestration

The `Scenario` class orchestrates multi-peer tests:

```python
from osvauld.scenario import Scenario

with Scenario(owner=1, node=1, viewer=2, show_ui=False) as s:
    # s.owner - the owner Session
    # s.node - the NodeSession (kunki)
    # s.viewers - list of viewer Sessions

    # Setup
    result = s.setup_owner(
        username="owner",
        app_path="./sample_apps/my-shop",
        app_name="Shop Owner"
    )
    space_id = result["space_id"]
    page_id = result["page_id"]

    # Connect owner to node
    s.connect_owner_to_node()
    s.publish_to_node(space_id)

    # Setup viewers
    viewer_link = s.get_viewer_link(space_id)
    s.setup_viewer(0, viewer_link, "customer1")
    s.setup_viewer(1, viewer_link, "customer2")

    # Test interactions
    s.owner.eval('add_product_via_ui("Widget", 99)')
    s.wait_sync()
    assert s.viewers[0].eval("return get_products_count()") >= 1
```

## AppTestScenario (Recommended)

`AppTestScenario` in `scripts/osvauld/scenario.py` collapses all boilerplate (TmuxManager, signup, create_space, connect, publish, add_viewer, open_app) into a single context manager. Tests focus purely on app-level logic.

```python
#!/usr/bin/env python3
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "scripts"))
from osvauld.scenario import AppTestScenario

SHOP_APP = Path(__file__).parent.parent / "sample_apps" / "my-shop"
args = AppTestScenario.parse_args("E-Commerce Test")

with AppTestScenario(
    name="ecomm_test",
    app_path=str(SHOP_APP),
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
    print("Product synced!")
```

### `PeerHandle` API

Each peer returned by `s.peer(name)` provides:
- `peer.eval(lua_code)` — execute Lua and return the result
- `peer.wait_for(fn, timeout=15, interval=0.5, desc="...")` — poll until truthy
- `peer.client` — the underlying `ControlClient` for advanced operations
- `peer.name`, `peer.role`, `peer.space_id`, `peer.page_id`, `peer.app_name`

### CLI Flags

All `AppTestScenario` tests support:
- `--keep` — keep tmux session alive after test
- `--debug` — keep session on failure for debugging
- `--release` — use release builds

For custom flags, use `AppTestScenario.add_args(parser)` with your own parser.

## Running Tests

```bash
# E2E tests (in e2e_tests/)
python e2e_tests/test_chat.py
python e2e_tests/test_ecommerce.py
python e2e_tests/test_chat_perf.py --messages 100

# With flags
python e2e_tests/test_chat.py --keep
python e2e_tests/test_ecommerce.py --debug
python e2e_tests/test_chat_perf.py --release --messages 200 --output results/run.json

# Run demo apps interactively
python scripts/run_demo.py snake
python scripts/run_demo.py chat
python scripts/run_demo.py tank
```

## Debugging

### Log Locations

```bash
# App logs
~/.local/share/osvauld/logs/*.log

# Filter for warnings (permit issues)
grep -i "warn" ~/.local/share/osvauld/logs/*.log

# Filter for errors (protocol bugs)
grep -i "error" ~/.local/share/osvauld/logs/*.log

# Production node logs
~/.local/share/osvauld-production/logs/*.log
```

### Common Issues

**Permit denied**
- Symptom: Sync fails, "not authorized" errors
- Check: `layer_patterns` in permit_template.json
- Fix: Verify `{page_id}/orders/{aud}` expansion matches layer name

**State divergence (peers have different data)**
- Symptom: Data appears on owner but not customer
- Check: `client.eval("return loro:list_layers('*')")` on both sides
- Check: P2P status with `client.p2p_status()`
- Fix: Force re-sync or check validation rejections in node logs

**Connection issues**
- Symptom: Handshake failures, timeouts
- Check: Node is running and reachable
- Fix: Verify `client.get_connection_string()` works

**Validation failed**
- Symptom: Updates rejected on node
- Check: State machine allows the transition for the role
- Fix: Review `validation.lua` logic

**Derivation not running**
- Symptom: Derived layer empty or stale
- Check: `init.lua` registered rules correctly
- Fix: `derivation:rebuild("target_layer")` to force rebuild

## Gotchas

- **Always `wait_for_sync()` after mutations** -- P2P sync is async, data doesn't appear instantly on other peers
- **Use `eval()` to inspect state**, not log parsing -- it's more reliable
- **`show_ui=False`** for headless testing (uses Slint testing backend)
- **Clean DB** (`rm -rf ~/.local/share/osvauld`) when state gets corrupted or permit templates change
- **`fresh=True`** in Scenario constructor clears the test directory before starting
- **Node must be started before owner connects** -- the Scenario class handles this order automatically
