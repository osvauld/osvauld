use osv_decl::ast::{CachePolicy, TimeModel};
use osv_decl::{compile_source, parser, semantic};

fn sample_valid() -> &'static str {
    r#"
app "Shop" version "1.0.0" {
  role owner can share, delegate, manage_access;
  role customer;

  layer orders as list {
    path "orders/{period}/{id}";
    namespace shared;
    grant explicit;
    shard by day;
    cache ui_window(200), sync_mode(incremental), retention_days(30);
    allow owner to create,read,write,sync,grant,revoke on *;
    allow customer to read on *;
  }

  derive orders_summary as map {
    from orders;
    cache ui_window(100);
    using "rules.compute_summary";
  }

  ui_app "Owner Console" {
    entry "owner/app.lua";
    allow owner;
  }
}
"#
}

#[test]
fn parse_full_file_succeeds() {
    let ast = parser::parse(sample_valid()).expect("valid grammar must parse");
    assert_eq!(ast.name, "Shop");
    assert_eq!(ast.roles.len(), 2);
    assert_eq!(ast.layers.len(), 1);
    assert_eq!(ast.derives.len(), 1);
    assert_eq!(ast.ui_apps.len(), 1);

    let layer = &ast.layers[0];
    assert!(matches!(layer.time, TimeModel::TimeSharded { .. }));
    assert!(layer
        .cache
        .iter()
        .any(|c| matches!(c, CachePolicy::RetentionDays(30))));
}

#[test]
fn compile_source_succeeds_and_lowers_schema() {
    let compiled = compile_source(sample_valid()).expect("compile should succeed");
    assert_eq!(compiled.permit.roles, vec!["owner", "customer"]);
    assert_eq!(compiled.permit.dynamic_layer_schemas.len(), 1);
    assert_eq!(compiled.permit.osv_policy.roles, vec!["owner", "customer"]);
    assert_eq!(compiled.permit.osv_policy.policy_rules.len(), 2);

    let schema = &compiled.permit.dynamic_layer_schemas[0];
    assert_eq!(schema.name, "orders");
    assert_eq!(schema.storage_strategy, "time_sharded");
    assert_eq!(schema.resolution.as_deref(), Some("day"));
    assert_eq!(schema.grant, "explicit");
}

#[test]
fn semantic_fails_when_sharded_path_missing_period() {
    let src = r#"
app "Bad" version "1.0.0" {
  role owner;
  layer logs as list {
    path "logs/{id}";
    namespace shared;
    shard by day;
    allow owner to create,read,write,sync on *;
  }
}
"#;

    let ast = parser::parse(src).expect("parser should pass");
    let errors = semantic::check(&ast).expect_err("semantic should fail");
    assert!(errors.iter().any(|e| e.code == "E2301"));
}

#[test]
fn semantic_fails_retention_without_sharding() {
    let src = r#"
app "Bad" version "1.0.0" {
  role owner;
  layer drafts as map {
    path "drafts/{id}";
    namespace creator;
    cache retention_days(7);
    allow owner to create,read,write,sync on *;
  }
}
"#;

    let ast = parser::parse(src).expect("parser should pass");
    let errors = semantic::check(&ast).expect_err("semantic should fail");
    assert!(errors.iter().any(|e| e.code == "E2303"));
}

#[test]
fn semantic_fails_on_inheritance_cycle() {
    let src = r#"
app "Bad" version "1.0.0" {
  role a inherits b;
  role b inherits a;
}
"#;

    let ast = parser::parse(src).expect("parser should pass");
    let errors = semantic::check(&ast).expect_err("semantic should fail");
    assert!(errors.iter().any(|e| e.code == "E2201"));
}

#[test]
fn semantic_fails_explicit_grant_without_grant_actions() {
    let src = r#"
app "Bad" version "1.0.0" {
  role owner;
  layer orders as list {
    path "orders/{id}";
    namespace shared;
    grant explicit;
    allow owner to create,read,write,sync on *;
  }
}
"#;

    let ast = parser::parse(src).expect("parser should pass");
    let errors = semantic::check(&ast).expect_err("semantic should fail");
    assert!(errors.iter().any(|e| e.code == "E2304"));
}

#[test]
fn parser_fails_on_unknown_top_level_decl() {
    let src = r#"
app "Bad" version "1.0.0" {
  banana x;
}
"#;

    let err = parser::parse(src).expect_err("parser should fail");
    assert_eq!(err.code, "E1101");
}
