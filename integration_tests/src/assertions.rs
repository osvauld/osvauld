//! Custom assertion helpers for integration tests
//!
//! These provide better error messages than generic assertions when testing
//! distributed system state.
//!
//! # Philosophy
//!
//! - Return `Result<()>` so they can be chained with `?`
//! - Include context about what was expected vs what was found
//! - Check multiple related conditions in one assertion where appropriate

use std::sync::Arc;

use anyhow::{ensure, Result};

use butler::Butler;

// =============================================================================
// Handshake Assertions
// =============================================================================

/// Assert that handshake completed between owner and node
///
/// **Context**: After InitiateHandshake and wait_for_owner_authenticated
/// **We verify**: Node has owner stored with expected username and permit
pub fn assert_owner_authenticated(
    node_butler: &Arc<Butler>,
    expected_owner_name: &str,
) -> Result<()> {
    let owner = node_butler.get_owner()?
        .ok_or_else(|| anyhow::anyhow!(
            "Node has no owner after handshake (expected: '{}')",
            expected_owner_name
        ))?;

    ensure!(
        owner.username == expected_owner_name,
        "Owner username mismatch: expected='{}' got='{}'",
        expected_owner_name, owner.username
    );

    ensure!(
        owner.permit.is_some(),
        "Owner '{}' stored on node but has no permit",
        expected_owner_name
    );

    Ok(())
}

/// Assert that sovereign node has a permit (user side, post-handshake)
///
/// **Context**: After user's handshake completes
/// **We verify**: User has stored permit from node
pub fn assert_sovereign_has_permit(butler: &Arc<Butler>, node_id: &str) -> Result<()> {
    let sovereign = butler.get_sovereign_node(node_id)?
        .ok_or_else(|| anyhow::anyhow!(
            "No sovereign node record for node_id='{}' after handshake",
            node_id
        ))?;

    ensure!(
        sovereign.permit.is_some(),
        "Sovereign node '{}' exists but has no permit after handshake",
        node_id
    );

    Ok(())
}

// =============================================================================
// Space/Page Sync Assertions
// =============================================================================

/// Assert that a space was synced from owner to node
///
/// **Context**: After PublishSpace and wait_for_space
/// **We verify**: Both owner and node have the space with matching name
pub fn assert_space_synced(
    owner: &Arc<Butler>,
    node: &Arc<Butler>,
    space_id: &str,
) -> Result<()> {
    let owner_space = owner.get_space(space_id)?
        .ok_or_else(|| anyhow::anyhow!(
            "Owner doesn't have space '{}' (should exist before sync)",
            space_id
        ))?;

    let node_space = node.get_space(space_id)?
        .ok_or_else(|| anyhow::anyhow!(
            "Node doesn't have space '{}' after PublishSpace",
            space_id
        ))?;

    ensure!(
        owner_space.name == node_space.name,
        "Space name mismatch: owner='{}' node='{}'",
        owner_space.name, node_space.name
    );

    Ok(())
}

/// Assert that a page was synced from owner to node
///
/// **Context**: After PublishSpace (which syncs pages) and wait_for_page
/// **We verify**: Both owner and node have the page with matching name
pub fn assert_page_synced(
    owner: &Arc<Butler>,
    node: &Arc<Butler>,
    page_id: &str,
) -> Result<()> {
    let owner_page = owner.get_page(page_id)?
        .ok_or_else(|| anyhow::anyhow!(
            "Owner doesn't have page '{}' (should exist before sync)",
            page_id
        ))?;

    let node_page = node.get_page(page_id)?
        .ok_or_else(|| anyhow::anyhow!(
            "Node doesn't have page '{}' after sync",
            page_id
        ))?;

    ensure!(
        owner_page.meta.name == node_page.meta.name,
        "Page name mismatch: owner='{}' node='{}'",
        owner_page.meta.name, node_page.meta.name
    );

    Ok(())
}

/// Assert that a page exists with a valid permit
///
/// **Context**: Verifying that page was created/synced with proper authorization
/// **We verify**: Page exists and has a permit attached
pub fn assert_page_has_permit(butler: &Arc<Butler>, page_id: &str) -> Result<()> {
    let page = butler.get_page(page_id)?
        .ok_or_else(|| anyhow::anyhow!(
            "Page '{}' not found",
            page_id
        ))?;

    ensure!(
        page.get_permit().is_some(),
        "Page '{}' exists but has no permit",
        page_id
    );

    Ok(())
}

/// Assert that a space exists
///
/// **Context**: Simple existence check
pub fn assert_space_exists(butler: &Arc<Butler>, space_id: &str) -> Result<()> {
    ensure!(
        butler.get_space(space_id)?.is_some(),
        "Space '{}' does not exist",
        space_id
    );
    Ok(())
}

/// Assert that a page exists
///
/// **Context**: Simple existence check
pub fn assert_page_exists(butler: &Arc<Butler>, page_id: &str) -> Result<()> {
    ensure!(
        butler.get_page(page_id)?.is_some(),
        "Page '{}' does not exist",
        page_id
    );
    Ok(())
}

// =============================================================================
// Permit Assertions
// =============================================================================

/// Assert that a permit has the expected relationship
///
/// **Context**: Verifying permit type (owner, viewer, node)
/// **We verify**: Permit parses and has expected relationship
pub fn assert_permit_relationship(permit_token: &str, expected_relationship: &str) -> Result<()> {
    let permit = gurkha::Permit::from_token(permit_token)
        .map_err(|e| anyhow::anyhow!("Failed to parse permit: {}", e))?;

    let relationship = permit.core().relationship()
        .ok_or_else(|| anyhow::anyhow!("Permit has no relationship fact"))?;

    ensure!(
        relationship == expected_relationship,
        "Permit relationship mismatch: expected='{}' got='{}'",
        expected_relationship, relationship
    );

    Ok(())
}

/// Assert that a permit is NOT a first_connection permit
///
/// **Context**: After first handshake, stored permits should not be first_connection
pub fn assert_not_first_connection(permit_token: &str) -> Result<()> {
    let permit = gurkha::Permit::from_token(permit_token)
        .map_err(|e| anyhow::anyhow!("Failed to parse permit: {}", e))?;

    ensure!(
        !permit.is_first_connection(),
        "Permit should NOT be first_connection after handshake completes"
    );

    Ok(())
}

/// Assert that a permit IS a first_connection permit
///
/// **Context**: Connection string permits should be first_connection for owners
pub fn assert_is_first_connection(permit_token: &str) -> Result<()> {
    let permit = gurkha::Permit::from_token(permit_token)
        .map_err(|e| anyhow::anyhow!("Failed to parse permit: {}", e))?;

    ensure!(
        permit.is_first_connection(),
        "Permit should be first_connection for initial owner connection"
    );

    Ok(())
}
