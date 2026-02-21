//! Permit upgrade synchronization tests.
//!
//! Focuses on behavior invariants:
//! - updated page permits are distributed to connected viewers
//! - stale permit updates are rejected by version ordering
//! - offline viewers converge to latest permit after reconnect

use std::collections::HashMap;

use anyhow::Result;
use courier::coordinator::CoordinatorMessage;
use gurkha::LayerConfig;

use crate::fixtures::{init_tracing, MOCK_TIMEOUT};
use crate::scenario::Scenario;

fn permit_version(token: &str) -> u64 {
    gurkha::Permit::from_token(token)
        .ok()
        .and_then(|p| p.get_fact("version").and_then(|v| v.as_u64()))
        .unwrap_or(0)
}

fn permit_has_layer(token: &str, layer_name: &str) -> bool {
    gurkha::Permit::from_token(token)
        .ok()
        .and_then(|p| p.get_fact("layers").and_then(|v| v.as_object().cloned()))
        .map(|layers| layers.contains_key(layer_name))
        .unwrap_or(false)
}

async fn viewer_page_permit(s: &Scenario, page_id: &str, viewer_idx: usize) -> Result<String> {
    let page = s
        .viewer(viewer_idx)
        .butler
        .pages()
        .get(page_id)?
        .ok_or_else(|| anyhow::anyhow!("viewer page not found: {}", page_id))?;
    page.get_permit()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("viewer page has no permit"))
}

#[tokio::test]
async fn test_page_permit_reissue_distributes_to_connected_viewer() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .viewers(1)
        .with_tracer()
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let space_id = s.space().space_id.clone();
    let link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &link, &page_id).await?;

    let before_count = s.tracer().messages_of_type("PermitUpdate").len();
    let before_permit = viewer_page_permit(&s, &page_id, 0).await?;
    let before_version = permit_version(&before_permit);

    let mut new_layers = HashMap::new();
    let test_layer = "app:PermitUpgradeTest".to_string();
    new_layers.insert(
        test_layer.clone(),
        LayerConfig {
            sync: true,
            write: true,
            layer_type: Some("map".to_string()),
        },
    );

    let reissued = s
        .node()
        .butler
        .permits()
        .page()
        .reissue_all(&page_id, new_layers)
        .await?;
    s.node()
        .coordinator
        .cast(CoordinatorMessage::DistributePagePermitUpdates {
            page_id: page_id.clone(),
            permits: reissued,
        })
        .map_err(|e| anyhow::anyhow!("DistributePagePermitUpdates failed: {:?}", e))?;

    // Wait for the viewer to receive the updated permit via event channel
    let version = s
        .viewer_mut(0)
        .wait_permit_updated(&page_id, before_version + 1, MOCK_TIMEOUT)
        .await?;

    let updated_permit = viewer_page_permit(&s, &page_id, 0).await?;
    assert_eq!(version, before_version + 1);
    assert!(permit_has_layer(&updated_permit, &test_layer));
    assert!(
        s.tracer().messages_of_type("PermitUpdate").len() > before_count,
        "expected at least one additional PermitUpdate after distribution"
    );

    s.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn test_stale_page_permit_update_is_rejected() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .viewers(1)
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let space_id = s.space().space_id.clone();
    let link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &link, &page_id).await?;

    let viewer_did = s.viewer(0).butler.user_info().await?.did;
    let old_token = viewer_page_permit(&s, &page_id, 0).await?;
    let old_version = permit_version(&old_token);

    let mut new_layers = HashMap::new();
    let test_layer = "app:StaleRejectTest".to_string();
    new_layers.insert(
        test_layer.clone(),
        LayerConfig {
            sync: true,
            write: true,
            layer_type: Some("map".to_string()),
        },
    );
    let reissued = s
        .node()
        .butler
        .permits()
        .page()
        .reissue_all(&page_id, new_layers)
        .await?;
    let new_token = reissued
        .iter()
        .find(|(did, _)| did == &viewer_did)
        .map(|(_, token)| token.clone())
        .ok_or_else(|| anyhow::anyhow!("reissue_all did not return viewer token"))?;

    // Send newer permit first.
    s.node()
        .coordinator
        .cast(CoordinatorMessage::DistributePagePermitUpdates {
            page_id: page_id.clone(),
            permits: vec![(viewer_did.clone(), new_token.clone())],
        })
        .map_err(|e| anyhow::anyhow!("Distribute new permit failed: {:?}", e))?;

    // Wait for the viewer to receive the updated permit via event channel
    s.viewer_mut(0)
        .wait_permit_updated(&page_id, old_version + 1, MOCK_TIMEOUT)
        .await?;

    // Then send stale old token; receiver should reject it (no PermitUpdated event emitted).
    s.node()
        .coordinator
        .cast(CoordinatorMessage::DistributePagePermitUpdates {
            page_id: page_id.clone(),
            permits: vec![(viewer_did.clone(), old_token)],
        })
        .map_err(|e| anyhow::anyhow!("Distribute stale permit failed: {:?}", e))?;

    // Small delay to let the stale update be processed and rejected
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let final_token = viewer_page_permit(&s, &page_id, 0).await?;
    let node_stored = s
        .node()
        .butler
        .permits()
        .page()
        .get(&page_id, &viewer_did)?
        .ok_or_else(|| anyhow::anyhow!("node missing viewer page permit after stale update"))?;
    assert_eq!(permit_version(&final_token), old_version + 1);
    assert_eq!(permit_version(&node_stored), old_version + 1);
    assert!(permit_has_layer(&final_token, &test_layer));
    assert!(permit_has_layer(&node_stored, &test_layer));

    s.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn test_offline_viewer_node_store_keeps_latest_reissued_permit() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .app("osvauld-demos")
        .published()
        .viewers(1)
        .build()
        .await?;

    let page_id = s.space().page_id.clone();
    let space_id = s.space().space_id.clone();
    let link = s.get_viewer_link(&space_id).await?;
    s.add_viewer(0, &link, &page_id).await?;
    let viewer_did = s.viewer(0).butler.user_info().await?.did;

    s.disconnect_viewer_from_node(0).await?;

    let mut layers_v1 = HashMap::new();
    layers_v1.insert(
        "app:OfflineUpgradeV1".to_string(),
        LayerConfig {
            sync: true,
            write: true,
            layer_type: Some("map".to_string()),
        },
    );
    let permits_v1 = s
        .node()
        .butler
        .permits()
        .page()
        .reissue_all(&page_id, layers_v1)
        .await?;
    s.node()
        .coordinator
        .cast(CoordinatorMessage::DistributePagePermitUpdates {
            page_id: page_id.clone(),
            permits: permits_v1,
        })
        .map_err(|e| anyhow::anyhow!("Distribute offline v1 failed: {:?}", e))?;

    let mut layers_v2 = HashMap::new();
    let final_layer = "app:OfflineUpgradeV2".to_string();
    layers_v2.insert(
        final_layer.clone(),
        LayerConfig {
            sync: true,
            write: true,
            layer_type: Some("map".to_string()),
        },
    );
    let permits_v2 = s
        .node()
        .butler
        .permits()
        .page()
        .reissue_all(&page_id, layers_v2)
        .await?;
    let latest_token = permits_v2
        .iter()
        .find(|(did, _)| did == &viewer_did)
        .map(|(_, token)| token.clone())
        .ok_or_else(|| anyhow::anyhow!("v2 reissue missing viewer token"))?;
    let latest_version = permit_version(&latest_token);

    s.node()
        .coordinator
        .cast(CoordinatorMessage::DistributePagePermitUpdates {
            page_id: page_id.clone(),
            permits: permits_v2,
        })
        .map_err(|e| anyhow::anyhow!("Distribute offline v2 failed: {:?}", e))?;

    // No event-based waiting needed here — viewer is offline, no PermitUpdated event fires.
    // The coordinator actor processes DistributePagePermitUpdates asynchronously (ractor mailbox),
    // so a small yield lets it drain the message and write to butler.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let stored_latest = s
        .node()
        .butler
        .permits()
        .page()
        .get(&page_id, &viewer_did)?
        .ok_or_else(|| {
            anyhow::anyhow!("node missing stored viewer page permit after offline updates")
        })?;
    assert_eq!(permit_version(&stored_latest), latest_version);
    assert!(permit_has_layer(&stored_latest, &final_layer));

    s.shutdown().await;
    Ok(())
}
