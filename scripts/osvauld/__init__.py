"""
Osvauld Python Client Library

Provides utilities for integration testing and AI automation:

- Client: Socket communication with control server (also available as ControlClient)
- Session: Single shell instance management
- NodeSession: Kunki node instance management
- Scenario: Multi-peer orchestration (owner, node, viewer)
- TmuxManager: Multi-instance orchestration
- Wait helpers: Polling and sync utilities

Usage:
    from osvauld import Client

    # Auto-discover running instance
    owner = Client.discover("owner")

    # Or explicit socket path
    owner = Client("/tmp/osvauld-debug-owner.sock")

    # Commands
    owner.signup_or_login("alice", "test123")
    space = owner.create_space_with_pages("/path/to/app")
    owner.open_app(space["pages"][0]["page_id"], "MyApp")

    # Lua evaluation
    result = owner.eval("return 1 + 1")

    # Wait for conditions
    owner.wait_until(lambda: owner.eval("return #items > 0"))
"""

from .client import ControlClient

# Alias for cleaner imports
Client = ControlClient

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
    # Client (primary name and alias)
    "Client",
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
