"""
Wait helpers for polling and synchronization

These utilities help wait for async operations to complete:
- wait_for_eval: Wait until Lua eval is available (app loaded)
- wait_for_condition: Generic polling helper
- wait_for_sync: Wait for P2P sync to propagate
"""

import time
from typing import Any, Callable, Optional, TYPE_CHECKING

if TYPE_CHECKING:
    from .session import Session


class TimeoutError(Exception):
    """Raised when a wait operation times out."""
    pass


def wait_for_condition(
    condition: Callable[[], bool],
    timeout: float = 30.0,
    interval: float = 0.5,
    description: str = "condition",
) -> None:
    """Wait until a condition is true.

    Args:
        condition: Callable that returns True when condition is met
        timeout: Maximum time to wait in seconds
        interval: Time between checks in seconds
        description: Description for error message

    Raises:
        TimeoutError: If condition not met within timeout
    """
    start = time.time()
    while time.time() - start < timeout:
        try:
            if condition():
                return
        except Exception:
            pass
        time.sleep(interval)

    raise TimeoutError(f"Timeout waiting for {description} (after {timeout}s)")


def wait_for_eval(session: "Session", timeout: float = 30.0) -> None:
    """Wait until Lua eval is available on a session.

    This is useful after opening an app - the Lua worker needs time to load.

    Args:
        session: The session to check
        timeout: Maximum time to wait

    Raises:
        TimeoutError: If eval not available within timeout
    """
    def check():
        result = session.eval("return 1")
        return result == 1

    wait_for_condition(check, timeout=timeout, description="eval ready")


def wait_for_value(
    session: "Session",
    code: str,
    expected: Any,
    timeout: float = 30.0,
    interval: float = 0.5,
) -> Any:
    """Wait until a Lua expression returns an expected value.

    Args:
        session: The session to query
        code: Lua code to evaluate (should include 'return')
        expected: Expected value (or callable predicate)
        timeout: Maximum time to wait
        interval: Time between checks

    Returns:
        The final value

    Raises:
        TimeoutError: If value doesn't match within timeout
    """
    start = time.time()
    last_value = None

    while time.time() - start < timeout:
        try:
            value = session.eval(code)
            last_value = value

            # Check if matches
            if callable(expected):
                if expected(value):
                    return value
            elif value == expected:
                return value
        except Exception:
            pass
        time.sleep(interval)

    raise TimeoutError(
        f"Timeout waiting for '{code}' to equal {expected} "
        f"(last value: {last_value}, after {timeout}s)"
    )


def wait_for_screen(session: "Session", screen: str, timeout: float = 10.0) -> None:
    """Wait until the UI is on a specific screen.

    Args:
        session: The session to check
        screen: Expected screen name (e.g., "spaces", "login")
        timeout: Maximum time to wait

    Raises:
        TimeoutError: If screen doesn't change within timeout
    """
    def check():
        return session.client.ui_get_screen() == screen

    wait_for_condition(check, timeout=timeout, description=f"screen '{screen}'")


def wait_for_sync(
    source: "Session",
    target: "Session",
    code: str,
    timeout: float = 30.0,
    interval: float = 1.0,
) -> Any:
    """Wait for a value to sync from source to target.

    Evaluates the same Lua code on both sessions and waits until
    the target has the same value as the source.

    Args:
        source: Session where the data originates
        target: Session that should receive the data via sync
        code: Lua code to evaluate (should include 'return')
        timeout: Maximum time to wait
        interval: Time between checks

    Returns:
        The synchronized value

    Raises:
        TimeoutError: If values don't match within timeout
    """
    start = time.time()
    source_value = None
    target_value = None

    while time.time() - start < timeout:
        try:
            source_value = source.eval(code)
            target_value = target.eval(code)

            if source_value == target_value and source_value is not None:
                return source_value
        except Exception:
            pass
        time.sleep(interval)

    raise TimeoutError(
        f"Timeout waiting for sync of '{code}'\n"
        f"  Source: {source_value}\n"
        f"  Target: {target_value}\n"
        f"  (after {timeout}s)"
    )
