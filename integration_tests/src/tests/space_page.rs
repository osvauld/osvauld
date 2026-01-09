//! Space and Page creation tests
//!
//! Tests the space and page creation flow via Butler:
//! - Create space with owner permit
//! - Create page with owner permit and layers
//! - Verify permit structure and facts



use crate::fixtures::{TEST_PAGE_LAYERS, TEST_PAGE_TEMPLATE, TEST_SPACE_TEMPLATE};
use crate::helpers::{init_tracing, setup_butler_with_identity};

#[tokio::test]
async fn test_create_space_with_permit() {
    init_tracing();

    // Setup Butler with identity (keep _temp_dir alive for test duration)
    let (butler, _signing_key, _temp_dir) = setup_butler_with_identity("alice", "password123")
        .await
        .expect("Failed to setup butler");

    // Get owner DID
    let user_info = butler.user_info().await.expect("Failed to get user info");

    // Create space via Butler
    let space = butler
        .create_space("My Project".to_string(), user_info.did.clone(), TEST_SPACE_TEMPLATE)
        .await
        .expect("Failed to create space");

    // Verify space metadata
    assert_eq!(space.name, "My Project");
    assert_eq!(space.owner_did, user_info.did);

    // Retrieve space and verify permit is stored
    let stored_space = butler
        .get_space(&space.id)
        .expect("Failed to get space")
        .expect("Space not found");

    assert_eq!(stored_space.id, space.id);
}

#[tokio::test]
async fn test_space_owner_permit_structure() {
    init_tracing();

    // Setup Butler with identity
    let (butler, _signing_key, _temp_dir) = setup_butler_with_identity("bob", "securepass")
        .await
        .expect("Failed to setup butler");

    let user_info = butler.user_info().await.expect("Failed to get user info");

    // Create space
    let space = butler
        .create_space("Test Space".to_string(), user_info.did.clone(), TEST_SPACE_TEMPLATE)
        .await
        .expect("Failed to create space");

    // Get the space with its permit (via store access)
    // The permit is stored with the space data
    let signing_key = butler.signing_key().await.expect("Failed to get signing key");

    // Issue a new space owner token directly to verify structure
    let (permit_token, _cid) = gurkha::issue_space_owner_token(
        &signing_key,
        &space.id,
        TEST_SPACE_TEMPLATE,
    ).await.expect("Failed to issue space owner token");

    // Parse and verify the permit structure
    let permit = gurkha::Permit::from_token(&permit_token).expect("Failed to parse permit");

    // Verify token type
    let token_type = permit.get_fact("token_type")
        .expect("Missing token_type")
        .as_str()
        .expect("token_type should be string");
    assert_eq!(token_type, "space_owner");

    // Verify relationship
    let relationship = permit.get_fact("relationship")
        .expect("Missing relationship")
        .as_str()
        .expect("relationship should be string");
    assert_eq!(relationship, "owner");

    // Verify space_id
    let space_id = permit.get_fact("space_id")
        .expect("Missing space_id")
        .as_str()
        .expect("space_id should be string");
    assert_eq!(space_id, space.id);

    // Verify operations
    let operations = permit.get_fact("operations")
        .expect("Missing operations");
    assert!(operations.get("own").is_some(), "Missing 'own' operation");
    assert!(operations.get("add_pages").is_some(), "Missing 'add_pages' operation");
    assert!(operations.get("share_space").is_some(), "Missing 'share_space' operation");

    // Verify delegation templates exist
    let delegation = permit.get_fact("delegation")
        .expect("Missing delegation templates");
    assert!(delegation.get("node").is_some(), "Missing 'node' delegation template");
    assert!(delegation.get("viewer").is_some(), "Missing 'viewer' delegation template");
}

#[tokio::test]
async fn test_create_page_with_permit() {
    init_tracing();

    // Setup Butler with identity
    let (butler, _signing_key, _temp_dir) = setup_butler_with_identity("carol", "mypassword")
        .await
        .expect("Failed to setup butler");

    let user_info = butler.user_info().await.expect("Failed to get user info");

    // Create space first (pages must belong to a space)
    let space = butler
        .create_space("My Space".to_string(), user_info.did.clone(), TEST_SPACE_TEMPLATE)
        .await
        .expect("Failed to create space");

    // Get layer names from template
    let layer_names: Vec<String> = TEST_PAGE_LAYERS
        .iter()
        .map(|s| s.to_string())
        .collect();

    // Create page via Butler
    let page = butler
        .create_page(
            &space.id,
            "My First Page",
            layer_names.clone(),
            TEST_PAGE_TEMPLATE,
        )
        .await
        .expect("Failed to create page");

    // Verify page metadata
    assert_eq!(page.name, "My First Page");
    assert_eq!(page.space_id, space.id);
    assert_eq!(page.owner_did, user_info.did);

    // Verify page is stored
    let stored_page = butler
        .get_page(&page.id)
        .expect("Failed to get page")
        .expect("Page not found");

    assert_eq!(stored_page.meta.id, page.id);

    // Verify permit is stored with page
    assert!(stored_page.permit.is_some(), "Page should have a permit stored");
}

#[tokio::test]
async fn test_page_owner_permit_structure() {
    init_tracing();

    // Setup Butler with identity
    let (butler, _signing_key, _temp_dir) = setup_butler_with_identity("dave", "password")
        .await
        .expect("Failed to setup butler");

    let user_info = butler.user_info().await.expect("Failed to get user info");

    // Create space
    let space = butler
        .create_space("Test Space".to_string(), user_info.did.clone(), TEST_SPACE_TEMPLATE)
        .await
        .expect("Failed to create space");

    // Create page
    let layer_names: Vec<String> = TEST_PAGE_LAYERS.iter().map(|s| s.to_string()).collect();
    let page = butler
        .create_page(
            &space.id,
            "Test Page",
            layer_names,
            TEST_PAGE_TEMPLATE,
        )
        .await
        .expect("Failed to create page");

    // Get the stored page to access its permit
    let stored_page = butler
        .get_page(&page.id)
        .expect("Failed to get page")
        .expect("Page not found");

    let permit_token = stored_page.permit.expect("Page should have permit");

    // Parse and verify the permit structure
    let permit = gurkha::Permit::from_token(&permit_token).expect("Failed to parse permit");

    // Verify token type
    let token_type = permit.get_fact("token_type")
        .expect("Missing token_type")
        .as_str()
        .expect("token_type should be string");
    assert_eq!(token_type, "page_owner");

    // Verify relationship
    let relationship = permit.get_fact("relationship")
        .expect("Missing relationship")
        .as_str()
        .expect("relationship should be string");
    assert_eq!(relationship, "owner");

    // Verify page_id
    let page_id = permit.get_fact("page_id")
        .expect("Missing page_id")
        .as_str()
        .expect("page_id should be string");
    assert_eq!(page_id, page.id);

    // Verify operations
    let operations = permit.get_fact("operations")
        .expect("Missing operations");
    assert!(operations.get("own").is_some(), "Missing 'own' operation");
    assert!(operations.get("share_page").is_some(), "Missing 'share_page' operation");

    // Verify layers
    let layers = permit.get_fact("layers")
        .expect("Missing layers");
    for layer_name in TEST_PAGE_LAYERS {
        assert!(
            layers.get(*layer_name).is_some(),
            "Missing layer: {}",
            layer_name
        );
    }

    // Verify delegation templates exist
    let delegation = permit.get_fact("delegation")
        .expect("Missing delegation templates");
    assert!(delegation.get("node").is_some(), "Missing 'node' delegation template");
    assert!(delegation.get("viewer").is_some(), "Missing 'viewer' delegation template");
}

#[tokio::test]
async fn test_page_layers_are_encrypted() {
    init_tracing();

    // Setup Butler with identity
    let (butler, _signing_key, _temp_dir) = setup_butler_with_identity("eve", "password")
        .await
        .expect("Failed to setup butler");

    let user_info = butler.user_info().await.expect("Failed to get user info");

    // Create space and page
    let space = butler
        .create_space("Encrypted Test".to_string(), user_info.did.clone(), TEST_SPACE_TEMPLATE)
        .await
        .expect("Failed to create space");

    let layer_names: Vec<String> = TEST_PAGE_LAYERS.iter().map(|s| s.to_string()).collect();
    let page = butler
        .create_page(
            &space.id,
            "Secret Page",
            layer_names,
            TEST_PAGE_TEMPLATE,
        )
        .await
        .expect("Failed to create page");

    // Decrypt and read the page - verifies encryption round-trip works
    let (decrypted_page, _aes_key) = butler
        .get_decrypted_page(&page.id)
        .await
        .expect("Failed to decrypt page");

    // Verify we got back the expected layers
    assert_eq!(decrypted_page.id, page.id);

    // Each layer should have been decrypted (though empty initially)
    let docs_json = decrypted_page.docs_to_json();
    for layer_name in TEST_PAGE_LAYERS {
        assert!(
            docs_json.get(*layer_name).is_some(),
            "Missing decrypted layer: {}",
            layer_name
        );
    }
}

#[tokio::test]
async fn test_list_spaces_and_pages() {
    init_tracing();

    // Setup Butler with identity
    let (butler, _signing_key, _temp_dir) = setup_butler_with_identity("frank", "password")
        .await
        .expect("Failed to setup butler");

    let user_info = butler.user_info().await.expect("Failed to get user info");

    // Create multiple spaces
    let space1 = butler
        .create_space("Space One".to_string(), user_info.did.clone(), TEST_SPACE_TEMPLATE)
        .await
        .expect("Failed to create space 1");

    let space2 = butler
        .create_space("Space Two".to_string(), user_info.did.clone(), TEST_SPACE_TEMPLATE)
        .await
        .expect("Failed to create space 2");

    // Create pages in first space
    let layer_names: Vec<String> = TEST_PAGE_LAYERS.iter().map(|s| s.to_string()).collect();

    let _page1 = butler
        .create_page(&space1.id, "Page A", layer_names.clone(), TEST_PAGE_TEMPLATE)
        .await
        .expect("Failed to create page 1");

    let _page2 = butler
        .create_page(&space1.id, "Page B", layer_names.clone(), TEST_PAGE_TEMPLATE)
        .await
        .expect("Failed to create page 2");

    // List spaces
    let spaces = butler.list_spaces().expect("Failed to list spaces");
    assert_eq!(spaces.len(), 2, "Should have 2 spaces");

    // List pages in space1
    let pages = butler.list_pages(&space1.id).expect("Failed to list pages");
    assert_eq!(pages.len(), 2, "Space 1 should have 2 pages");

    // Space2 should have no pages
    let pages2 = butler.list_pages(&space2.id).expect("Failed to list pages");
    assert!(pages2.is_empty(), "Space 2 should have no pages");
}
