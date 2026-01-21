"""
Osvauld Test Library

Provides utilities for integration testing:

- ControlClient: Socket communication with control server
- Session: Single shell instance management
- NodeSession: Kunki node instance management
- Scenario: Multi-peer orchestration (owner, node, viewer)
- Wait helpers: Polling and sync utilities

Usage:
    from lib import Scenario, wait_for_eval

    with Scenario(owner=1, node=1, viewer=1) as s:
        # Setup owner
        s.owner.signup("alice", "test123")
        s.owner.login("test123")
        space = s.owner.create_space("My Space", "/path/to/app")

        # Connect to node and publish
        s.connect_owner_to_node()
        s.publish_to_node(space["id"])

        # Get viewer link and setup viewer
        link = s.get_viewer_link(space["id"])
        s.setup_viewer(0, link)

        # Open app and test
        s.owner.open_app(page_id, "MyApp")
        wait_for_eval(s.owner)

        s.owner.eval("add_item('Widget')")
        s.wait_sync()

        items = s.viewer.eval("return get_items()")
        assert len(items) == 1
"""

from .client import ControlClient
from .session import Session
from .scenario import Scenario, NodeSession
from .tmux import TmuxManager
from .wait import (
    wait_for_condition,
    wait_for_eval,
    wait_for_value,
    wait_for_screen,
    wait_for_sync,
    TimeoutError,
)

__all__ = [
    # Client
    "ControlClient",
    # Sessions
    "Session",
    "NodeSession",
    "Scenario",
    # Tmux
    "TmuxManager",
    # Wait helpers
    "wait_for_condition",
    "wait_for_eval",
    "wait_for_value",
    "wait_for_screen",
    "wait_for_sync",
    "TimeoutError",
]
