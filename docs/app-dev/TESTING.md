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
client.refresh_app(app_name, app_dir)            # Hot reload a single app
client.refresh_page(page_dir)                    # Hot reload all apps in page
```

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

## Writing a Test Script

Complete pattern from `scripts/test_ecommerce_sync.py`:

```python
#!/usr/bin/env python3
"""E-commerce sync test"""

from pathlib import Path
from osvauld.scenario import Scenario

SHOP_APP = Path("./sample_apps/my-shop")

def main():
    with Scenario(owner=1, node=1, viewer=1, fresh=True) as s:
        # 1. Owner setup
        result = s.setup_owner(
            username="shopowner",
            app_path=str(SHOP_APP),
            app_name="Shop Owner"
        )
        space_id = result["space_id"]
        page_id = result["page_id"]

        # 2. Connect to node and publish
        s.connect_owner_to_node()
        s.publish_to_node(space_id)

        # 3. Setup customer
        viewer_link = s.get_viewer_link(space_id)
        s.setup_viewer(0, viewer_link, "customer1")
        s.viewer.client.open_app(page_id, "Shop Customer")

        # 4. Owner adds a product
        s.owner.eval('add_product_via_ui("Widget", 99, "Test product", 50)')

        # 5. Wait for sync and verify
        s.owner.client.wait_for_sync(
            s.viewer.client,
            "return get_products_count() >= 1",
            timeout=15
        )
        print("Product synced to customer!")

        # 6. Customer places order
        s.viewer.eval('place_order_via_ui("Widget", 2)')
        s.wait_sync()

        # 7. Verify derived data
        summary_count = s.owner.eval(
            "return get_orders_summary_count()"
        )
        assert summary_count >= 1, f"Expected orders summary, got {summary_count}"
        print("Order synced and derivation working!")

if __name__ == "__main__":
    main()
```

## Running Tests

```bash
# Run demo apps interactively
python scripts/run_demo.py snake
python scripts/run_demo.py chat
python scripts/run_demo.py tank

# Run sync test scripts
python scripts/test_ecommerce_sync.py
python scripts/test_canvas_sync.py
python scripts/test_booking_sync.py
python scripts/test_demos_sync.py
python scripts/test_gallery_sync.py
python scripts/test_tank_sync.py
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
