//! Permission enforcement tests
//!
//! Tests that permits are correctly enforced during protocol operations.

use anyhow::Result;

use crate::fixtures::init_tracing;
use crate::scenario::Scenario;

/// Owner can publish — node accepts publish from owner
#[tokio::test]
async fn test_owner_can_publish() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .with_tracer()
        .build()
        .await?;

    // PublishSpaceAck means node accepted the publish
    s.tracer()
        .assert_contains_sequence(&["PublishSpace", "PublishSpaceAck"]);

    s.shutdown().await;
    Ok(())
}

/// Viewer gets scoped access — viewer connection string has correct space_id
#[tokio::test]
async fn test_viewer_scoped_access() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .viewers(1)
        .build()
        .await?;

    // Get viewer link
    let conn_string = s.get_viewer_link(&s.space().space_id).await?;

    // Parse and verify it targets the correct space
    use butler::ConnectionStringExt;
    let parsed = s
        .viewer(0)
        .butler
        .nodes()
        .parse_connection_string(&conn_string)
        .map_err(|e| anyhow::anyhow!("parse failed: {}", e))?;
    let space_id = parsed
        .space_id()
        .map_err(|e| anyhow::anyhow!("space_id failed: {}", e))?;

    assert_eq!(space_id, s.space().space_id);

    s.shutdown().await;
    Ok(())
}

/// Connection string format — has node_id and permit
#[tokio::test]
async fn test_connection_string_format() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .build()
        .await?;

    let conn_string = s.get_viewer_link(&s.space().space_id).await?;

    // Connection string should be non-empty and parseable
    assert!(!conn_string.is_empty());

    // Should contain node info — parseable with permit
    let parsed = s
        .node()
        .butler
        .nodes()
        .parse_connection_string(&conn_string)
        .map_err(|e| anyhow::anyhow!("parse failed: {}", e))?;
    assert!(!parsed.permit.is_empty(), "Should have permit");

    s.shutdown().await;
    Ok(())
}
