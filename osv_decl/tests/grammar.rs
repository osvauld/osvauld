use osv_decl::ast::*;
use osv_decl::{compile_source, parser, semantic};

// ═══════════════════════════════════════════════════════════════════════════
// Full v2 Group Chat Example (from DESIGN_OSV_V2.md)
// ═══════════════════════════════════════════════════════════════════════════

fn group_chat_v2() -> &'static str {
    r#"
app "group_chat" version "2.0.0" {
  role owner can share, delegate, accept_publish, manage_access;
  role node can relay, share, accept_publish, manage_access;
  role collaborator can manage_access;
  role layer_authority can manage_access;

  sthithi Message {
    field text required;
    field sender from peer immutable;
    field sent_at from clock immutable;
    field edited_at from clock;
    field thread_id;

    on update set edited_at from clock;
    on delete reject;
  }

  sthithi Channel {
    field name required;
    field created_by from peer immutable;
    field created_at from clock immutable;
    field description;
  }

  layer app_data as map {
    path app_data;
    namespace shared;
    allow owner to read, write, sync;
    allow node, collaborator, layer_authority to read, sync;
  }

  layer presence as map {
    path presence;
    namespace shared;
    allow owner, node, collaborator, layer_authority to read, write, sync;
    broadcast all debounce 1000;
  }

  layer channels as map for Channel {
    path channels;
    namespace shared;
    allow owner to read, write, sync;
    allow node to read, write, sync where mode is node;
    allow collaborator, layer_authority to read, sync;
    broadcast all;
  }

  layer channel_messages as list for Message {
    path channels / {id} / messages;
    namespace shared;
    dynamic by id;
    create allow owner, node, collaborator;
    discover on sync;
    shard by day;
    retain 365;

    allow owner, node, collaborator, layer_authority to read, write, sync;
    allow node to sync where mode is node;

    broadcast all;
    order by sent_at descending;
  }

  layer dm_messages as list for Message {
    path dms / {id} / messages;
    namespace page;
    grant explicit;
    dynamic by id;
    create allow owner, collaborator;
    discover on grant;

    allow owner, node, collaborator, layer_authority to read, write, sync;
    allow owner, layer_authority to grant, revoke;

    broadcast granted;
    order by sent_at descending;
  }

  derive channel_stats as map {
    from channel_messages;
    using "hooks.update_channel_stats";
  }

  permit collaborator_permit for collaborator {
    allow read, write, sync on channel_messages;
    allow read, write, sync on dm_messages;
    allow read, sync on channels;
    allow read, write, sync on presence;
    allow read, sync on app_data;
    issue for collaborator;
  }

  permit owner_permit for owner {
    allow read, write, sync, grant, revoke on channel_messages;
    allow read, write, sync, grant, revoke on dm_messages;
    allow read, write, sync on channels;
    allow read, write, sync on presence;
    allow read, write, sync on app_data;
    issue for collaborator, owner;
  }

  ui_app "Group Chat" {
    allow owner, collaborator, node;
  }
}
"#
}

#[test]
fn parse_group_chat_v2_full_example() {
    let ast = parser::parse(group_chat_v2()).expect("v2 group chat must parse");

    assert_eq!(ast.name, "group_chat");
    assert_eq!(ast.version, "2.0.0");
    assert_eq!(ast.roles.len(), 4);
    assert_eq!(ast.sthithis.len(), 2);
    assert_eq!(ast.layers.len(), 5);
    assert_eq!(ast.derives.len(), 1);
    assert_eq!(ast.permits.len(), 2);
    assert_eq!(ast.ui_apps.len(), 1);
    assert_eq!(ast.relays.len(), 0);
    assert_eq!(ast.validates.len(), 0);
}

#[test]
fn parse_v2_sthithi_message() {
    let ast = parser::parse(group_chat_v2()).expect("parse");
    let msg = ast.sthithis.iter().find(|s| s.name == "Message").unwrap();

    assert_eq!(msg.fields.len(), 5);

    let text = msg.fields.iter().find(|f| f.name == "text").unwrap();
    assert!(text.required);
    assert!(!text.immutable);
    assert!(text.default.is_none());
    assert!(text.source.is_none());

    let sender = msg.fields.iter().find(|f| f.name == "sender").unwrap();
    assert!(!sender.required);
    assert!(sender.immutable);
    assert_eq!(sender.source, Some(FieldSource::Peer));

    let sent_at = msg.fields.iter().find(|f| f.name == "sent_at").unwrap();
    assert!(sent_at.immutable);
    assert_eq!(sent_at.source, Some(FieldSource::Clock));

    let edited_at = msg.fields.iter().find(|f| f.name == "edited_at").unwrap();
    assert!(!edited_at.immutable);
    assert_eq!(edited_at.source, Some(FieldSource::Clock));

    let thread_id = msg.fields.iter().find(|f| f.name == "thread_id").unwrap();
    assert!(!thread_id.required);
    assert!(!thread_id.immutable);
    assert!(thread_id.default.is_none());
    assert!(thread_id.source.is_none());

    assert_eq!(msg.entity_rules.len(), 2);
    assert!(matches!(
        &msg.entity_rules[0],
        EntityRule::OnUpdateSet {
            field,
            source: EntityActionSource::FromClock,
        } if field == "edited_at"
    ));
    assert!(matches!(&msg.entity_rules[1], EntityRule::OnDeleteReject));
    assert_eq!(msg.transitions.len(), 0);
}

#[test]
fn parse_v2_sthithi_channel() {
    let ast = parser::parse(group_chat_v2()).expect("parse");
    let ch = ast.sthithis.iter().find(|s| s.name == "Channel").unwrap();

    assert_eq!(ch.fields.len(), 4);
    let name_field = ch.fields.iter().find(|f| f.name == "name").unwrap();
    assert!(name_field.required);

    let created_by = ch.fields.iter().find(|f| f.name == "created_by").unwrap();
    assert!(created_by.immutable);
    assert_eq!(created_by.source, Some(FieldSource::Peer));
}

#[test]
fn parse_v2_layer_with_entity_binding() {
    let ast = parser::parse(group_chat_v2()).expect("parse");
    let channels = ast.layers.iter().find(|l| l.name == "channels").unwrap();
    assert_eq!(channels.entity_binding.as_deref(), Some("Channel"));
    assert_eq!(channels.kind, LayerKind::Map);
}

#[test]
fn parse_v2_structural_path() {
    let ast = parser::parse(group_chat_v2()).expect("parse");

    let app_data = ast.layers.iter().find(|l| l.name == "app_data").unwrap();
    assert_eq!(app_data.path, "app_data");

    let ch_msgs = ast
        .layers
        .iter()
        .find(|l| l.name == "channel_messages")
        .unwrap();
    assert_eq!(ch_msgs.path, "channels/{id}/messages");

    let dm_msgs = ast.layers.iter().find(|l| l.name == "dm_messages").unwrap();
    assert_eq!(dm_msgs.path, "dms/{id}/messages");
}

#[test]
fn parse_v2_dynamic_layer() {
    let ast = parser::parse(group_chat_v2()).expect("parse");
    let ch_msgs = ast
        .layers
        .iter()
        .find(|l| l.name == "channel_messages")
        .unwrap();

    assert_eq!(ch_msgs.dynamic_by.as_deref(), Some(&["id".to_string()][..]));
    assert_eq!(
        ch_msgs.create_allow.as_deref(),
        Some(
            &[
                "owner".to_string(),
                "node".to_string(),
                "collaborator".to_string()
            ][..]
        )
    );
    assert_eq!(ch_msgs.discover, Some(DiscoverMode::Sync));
}

#[test]
fn parse_v2_retain_and_shard() {
    let ast = parser::parse(group_chat_v2()).expect("parse");
    let ch_msgs = ast
        .layers
        .iter()
        .find(|l| l.name == "channel_messages")
        .unwrap();

    assert_eq!(ch_msgs.retain, Some(365));
    assert!(matches!(
        ch_msgs.time,
        TimeModel::TimeSharded {
            resolution: Resolution::Day,
            ..
        }
    ));
}

#[test]
fn parse_v2_broadcast() {
    let ast = parser::parse(group_chat_v2()).expect("parse");

    let presence = ast.layers.iter().find(|l| l.name == "presence").unwrap();
    assert_eq!(presence.broadcasts.len(), 1);
    assert_eq!(presence.broadcasts[0].target, BroadcastTarget::All);
    assert_eq!(presence.broadcasts[0].debounce, Some(1000));

    let ch_msgs = ast
        .layers
        .iter()
        .find(|l| l.name == "channel_messages")
        .unwrap();
    assert_eq!(ch_msgs.broadcasts.len(), 1);
    assert_eq!(ch_msgs.broadcasts[0].target, BroadcastTarget::All);
    assert_eq!(ch_msgs.broadcasts[0].debounce, None);

    let dm_msgs = ast.layers.iter().find(|l| l.name == "dm_messages").unwrap();
    assert_eq!(dm_msgs.broadcasts.len(), 1);
    assert_eq!(dm_msgs.broadcasts[0].target, BroadcastTarget::Granted);
}

#[test]
fn parse_v2_order() {
    let ast = parser::parse(group_chat_v2()).expect("parse");

    let ch_msgs = ast
        .layers
        .iter()
        .find(|l| l.name == "channel_messages")
        .unwrap();
    let order = ch_msgs.order.as_ref().expect("should have order");
    assert_eq!(order.field, "sent_at");
    assert_eq!(order.direction, SortDirection::Descending);
}

#[test]
fn parse_v2_where_clause() {
    let ast = parser::parse(group_chat_v2()).expect("parse");

    let channels = ast.layers.iter().find(|l| l.name == "channels").unwrap();
    let node_rule = channels
        .access
        .iter()
        .find(|r| r.roles == vec!["node"])
        .unwrap();
    let pred = node_rule.predicate.as_ref().expect("should have predicate");
    assert_eq!(pred.predicates.len(), 1);
    assert!(matches!(
        &pred.predicates[0],
        Predicate::Is { field, value: ValueRef::Node } if field == "mode"
    ));
}

#[test]
fn parse_v2_namespace_page() {
    let ast = parser::parse(group_chat_v2()).expect("parse");

    let dm_msgs = ast.layers.iter().find(|l| l.name == "dm_messages").unwrap();
    assert_eq!(dm_msgs.namespace, LayerNamespace::Page);
}

#[test]
fn parse_v2_permit_declarations() {
    let ast = parser::parse(group_chat_v2()).expect("parse");

    let collab = ast
        .permits
        .iter()
        .find(|p| p.name == "collaborator_permit")
        .unwrap();
    assert_eq!(collab.roles, vec!["collaborator"]);
    assert_eq!(collab.statements.len(), 6);

    if let PermitStmt::Allow(allow) = &collab.statements[0] {
        assert_eq!(
            allow.actions,
            vec![Action::Read, Action::Write, Action::Sync]
        );
        assert_eq!(allow.layer, "channel_messages");
        assert!(allow.predicate.is_none());
    } else {
        panic!("expected PermitStmt::Allow");
    }

    if let PermitStmt::Issue(issue) = &collab.statements[5] {
        assert_eq!(issue.roles, vec!["collaborator"]);
    } else {
        panic!("expected PermitStmt::Issue");
    }

    let owner = ast
        .permits
        .iter()
        .find(|p| p.name == "owner_permit")
        .unwrap();
    assert_eq!(owner.roles, vec!["owner"]);

    let issue_stmt = owner
        .statements
        .iter()
        .find(|s| matches!(s, PermitStmt::Issue(_)))
        .unwrap();
    if let PermitStmt::Issue(issue) = issue_stmt {
        assert_eq!(issue.roles, vec!["collaborator", "owner"]);
    }
}

#[test]
fn compile_v2_group_chat_succeeds() {
    let compiled = compile_source(group_chat_v2()).expect("v2 group chat should compile");
    assert_eq!(compiled.permit.roles.len(), 4);
    assert_eq!(compiled.permit.dynamic_layer_schemas.len(), 5);
    assert_eq!(compiled.runtime.app_name, "group_chat");
    assert_eq!(compiled.runtime.app_version, "2.0.0");
}

#[test]
fn compile_v2_lowers_delegation_rules_from_issue_for() {
    let compiled = compile_source(group_chat_v2()).expect("v2 group chat should compile");
    let rules = &compiled.permit.osv_policy.delegation_rules;

    assert!(!rules.is_empty(), "delegation rules should be lowered");

    assert!(rules.iter().any(|r| {
        matches!(&r.from, policy_model::Subject::Role { name } if name == "owner")
            && matches!(&r.to, policy_model::Subject::Role { name } if name == "collaborator")
            && matches!(
                &r.resource,
                policy_model::ResourceSelector::Layer { name } if name == "channels/{id}/messages"
            )
            && r.actions.contains(&policy_model::Action::Read)
            && r.actions.contains(&policy_model::Action::Write)
            && r.actions.contains(&policy_model::Action::Sync)
            && r.no_escalation
            && r.max_depth == 3
    }));
}

// ═══════════════════════════════════════════════════════════════════════════
// Full v2 Shop Example (from DESIGN_OSV_V2.md section 4.2)
// ═══════════════════════════════════════════════════════════════════════════

fn shop_v2() -> &'static str {
    r#"
app "my_shop" version "2.0.0" {
  role owner can share, delegate, accept_publish, manage_access;
  role customer;

  sthithi Order {
    field status default "pending";
    field total required;
    field customer from peer immutable;
    field created_at from clock immutable;
    field updated_at from clock;

    transitions status {
      "pending" to "confirmed" by owner;
      "confirmed" to "shipped" by owner;
      "pending" to "cancelled" by customer where customer is self;
    }

    on update set updated_at from clock;
    on delete reject;
  }

  layer orders as list for Order {
    path orders / {customer} / entries;
    namespace page;
    grant explicit;
    dynamic by customer;
    create allow customer;
    discover on sync;
    shard by day;
    retain 90;

    allow owner to read, write, sync, grant, revoke;
    allow customer to read, write, sync where customer is self;

    broadcast to owner;
    broadcast to customer;

    order by created_at descending;
  }

  derive order_stats as map {
    from orders;
    using "hooks.update_order_stats";
  }

  relay on orders insert using "hooks.notify_owner";

  validate orders using "validation.check_order";

  permit customer_permit for customer {
    allow read, write on orders where customer is self;
    allow sync on orders;
    issue for customer;
  }

  permit owner_permit for owner {
    allow read, write, grant, revoke on orders;
    allow sync on orders;
    issue for customer, owner;
  }

  ui_app "Shop Owner" {
    allow owner;
  }

  ui_app "Shop Customer" {
    allow customer;
  }
}
"#
}

#[test]
fn parse_shop_v2_full() {
    let ast = parser::parse(shop_v2()).expect("v2 shop must parse");

    assert_eq!(ast.name, "my_shop");
    assert_eq!(ast.roles.len(), 2);
    assert_eq!(ast.sthithis.len(), 1);
    assert_eq!(ast.layers.len(), 1);
    assert_eq!(ast.derives.len(), 1);
    assert_eq!(ast.relays.len(), 1);
    assert_eq!(ast.validates.len(), 1);
    assert_eq!(ast.permits.len(), 2);
    assert_eq!(ast.ui_apps.len(), 2);
}

#[test]
fn parse_v2_transitions() {
    let ast = parser::parse(shop_v2()).expect("parse");
    let order = ast.sthithis.iter().find(|s| s.name == "Order").unwrap();

    assert_eq!(order.transitions.len(), 1);
    let transition = &order.transitions[0];
    assert_eq!(transition.field, "status");
    assert_eq!(transition.rules.len(), 3);

    let r0 = &transition.rules[0];
    assert_eq!(r0.from_state, "pending");
    assert_eq!(r0.to_state, "confirmed");
    assert_eq!(r0.allowed_roles, vec!["owner"]);
    assert!(r0.predicate.is_none());

    let r2 = &transition.rules[2];
    assert_eq!(r2.from_state, "pending");
    assert_eq!(r2.to_state, "cancelled");
    assert_eq!(r2.allowed_roles, vec!["customer"]);
    let pred = r2.predicate.as_ref().expect("should have predicate");
    assert!(matches!(
        &pred.predicates[0],
        Predicate::Is { field, value: ValueRef::SelfRef } if field == "customer"
    ));
}

#[test]
fn parse_v2_field_default() {
    let ast = parser::parse(shop_v2()).expect("parse");
    let order = ast.sthithis.iter().find(|s| s.name == "Order").unwrap();

    let status = order.fields.iter().find(|f| f.name == "status").unwrap();
    assert_eq!(status.default, Some(Literal::String("pending".to_string())));
}

#[test]
fn parse_v2_relay() {
    let ast = parser::parse(shop_v2()).expect("parse");
    assert_eq!(ast.relays.len(), 1);

    let relay = &ast.relays[0];
    assert_eq!(relay.layer, "orders");
    assert_eq!(relay.event, RelayEvent::Insert);
    assert_eq!(relay.handler, "hooks.notify_owner");
}

#[test]
fn parse_v2_validate() {
    let ast = parser::parse(shop_v2()).expect("parse");
    assert_eq!(ast.validates.len(), 1);

    let validate = &ast.validates[0];
    assert_eq!(validate.layer, "orders");
    assert_eq!(validate.handler, "validation.check_order");
}

#[test]
fn parse_v2_broadcast_to_roles() {
    let ast = parser::parse(shop_v2()).expect("parse");
    let orders = ast.layers.iter().find(|l| l.name == "orders").unwrap();

    assert_eq!(orders.broadcasts.len(), 2);
    assert_eq!(
        orders.broadcasts[0].target,
        BroadcastTarget::ToRoles(vec!["owner".to_string()])
    );
    assert_eq!(
        orders.broadcasts[1].target,
        BroadcastTarget::ToRoles(vec!["customer".to_string()])
    );
}

#[test]
fn parse_v2_permit_with_where() {
    let ast = parser::parse(shop_v2()).expect("parse");

    let cust_permit = ast
        .permits
        .iter()
        .find(|p| p.name == "customer_permit")
        .unwrap();

    if let PermitStmt::Allow(allow) = &cust_permit.statements[0] {
        assert_eq!(allow.actions, vec![Action::Read, Action::Write]);
        assert_eq!(allow.layer, "orders");
        let pred = allow.predicate.as_ref().expect("should have predicate");
        assert!(matches!(
            &pred.predicates[0],
            Predicate::Is { field, value: ValueRef::SelfRef } if field == "customer"
        ));
    } else {
        panic!("expected Allow");
    }
}

#[test]
fn compile_v2_shop_succeeds() {
    let compiled = compile_source(shop_v2()).expect("v2 shop should compile");
    assert_eq!(compiled.permit.roles, vec!["owner", "customer"]);
    assert_eq!(compiled.runtime.ui_apps.len(), 2);
    // Verify sanitized layer names
    assert_eq!(compiled.runtime.ui_apps[0].layer_name, "shop_owner");
    assert_eq!(compiled.runtime.ui_apps[1].layer_name, "shop_customer");
    assert_eq!(
        compiled.runtime.app_layer_names,
        vec!["shop_owner", "shop_customer"]
    );
}

#[test]
fn compile_v2_group_chat_app_layer_names() {
    let compiled = compile_source(group_chat_v2()).expect("compile");
    assert_eq!(compiled.runtime.ui_apps.len(), 1);
    assert_eq!(compiled.runtime.ui_apps[0].name, "Group Chat");
    assert_eq!(compiled.runtime.ui_apps[0].layer_name, "group_chat");
    assert_eq!(compiled.runtime.app_layer_names, vec!["group_chat"]);
}

#[test]
fn sanitize_app_name_cases() {
    use osv_decl::ast::sanitize_app_name;
    assert_eq!(sanitize_app_name("Group Chat"), "group_chat");
    assert_eq!(sanitize_app_name("Snake Game"), "snake_game");
    assert_eq!(sanitize_app_name("Snake-Raylib"), "snake_raylib");
    assert_eq!(sanitize_app_name("my_shop"), "my_shop");
    assert_eq!(sanitize_app_name("Protocol Docs"), "protocol_docs");
    assert_eq!(sanitize_app_name("Tank  Game"), "tank_game"); // double space
    assert_eq!(sanitize_app_name("shared"), "shared");
}

// ═══════════════════════════════════════════════════════════════════════════
// Individual construct parsing tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn parse_sthithi_minimal() {
    let src = r#"
app "Test" version "1.0.0" {
  sthithi Item {
    field name;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    assert_eq!(ast.sthithis.len(), 1);
    assert_eq!(ast.sthithis[0].name, "Item");
    assert_eq!(ast.sthithis[0].fields.len(), 1);
    assert_eq!(ast.sthithis[0].fields[0].name, "name");
    assert!(!ast.sthithis[0].fields[0].required);
    assert!(!ast.sthithis[0].fields[0].immutable);
}

#[test]
fn parse_field_all_modifiers() {
    let src = r#"
app "Test" version "1.0.0" {
  sthithi X {
    field a required;
    field b immutable;
    field c default "hello";
    field d default 42;
    field e default true;
    field f default false;
    field g default null;
    field h from clock;
    field i from peer;
    field j from mode;
    field k from clock immutable;
    field l required immutable;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let s = &ast.sthithis[0];
    assert_eq!(s.fields.len(), 12);

    assert!(s.fields[0].required);
    assert!(s.fields[1].immutable);
    assert_eq!(
        s.fields[2].default,
        Some(Literal::String("hello".to_string()))
    );
    assert_eq!(s.fields[3].default, Some(Literal::Int(42)));
    assert_eq!(s.fields[4].default, Some(Literal::Bool(true)));
    assert_eq!(s.fields[5].default, Some(Literal::Bool(false)));
    assert_eq!(s.fields[6].default, Some(Literal::Null));
    assert_eq!(s.fields[7].source, Some(FieldSource::Clock));
    assert_eq!(s.fields[8].source, Some(FieldSource::Peer));
    assert_eq!(s.fields[9].source, Some(FieldSource::Mode));
    assert_eq!(s.fields[10].source, Some(FieldSource::Clock));
    assert!(s.fields[10].immutable);
    assert!(s.fields[11].required);
    assert!(s.fields[11].immutable);
}

#[test]
fn parse_entity_rules() {
    let src = r#"
app "Test" version "1.0.0" {
  sthithi X {
    field ts from clock;
    on update set ts from clock;
    on delete reject;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let s = &ast.sthithis[0];
    assert_eq!(s.entity_rules.len(), 2);
    assert!(matches!(
        &s.entity_rules[0],
        EntityRule::OnUpdateSet { field, source: EntityActionSource::FromClock } if field == "ts"
    ));
    assert!(matches!(&s.entity_rules[1], EntityRule::OnDeleteReject));
}

#[test]
fn parse_predicate_is_not() {
    let src = r#"
app "Test" version "1.0.0" {
  role admin;
  layer data as map {
    path data;
    namespace shared;
    allow admin to read where status is not "deleted";
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let pred = ast.layers[0].access[0]
        .predicate
        .as_ref()
        .expect("should have predicate");
    assert!(matches!(
        &pred.predicates[0],
        Predicate::IsNot { field, value: ValueRef::StringLit(s) }
        if field == "status" && s == "deleted"
    ));
}

#[test]
fn parse_predicate_in() {
    let src = r#"
app "Test" version "1.0.0" {
  role admin;
  layer data as map {
    path data;
    namespace shared;
    allow admin to read where status in ("active", "pending");
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let pred = ast.layers[0].access[0]
        .predicate
        .as_ref()
        .expect("should have predicate");
    if let Predicate::In { field, values } = &pred.predicates[0] {
        assert_eq!(field, "status");
        assert_eq!(values.len(), 2);
        assert!(matches!(&values[0], ValueRef::StringLit(s) if s == "active"));
        assert!(matches!(&values[1], ValueRef::StringLit(s) if s == "pending"));
    } else {
        panic!("expected In predicate");
    }
}

#[test]
fn parse_predicate_above_below() {
    let src = r#"
app "Test" version "1.0.0" {
  role admin;
  layer data as map {
    path data;
    namespace shared;
    allow admin to read where age above 18 and score below 100;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let pred = ast.layers[0].access[0]
        .predicate
        .as_ref()
        .expect("should have predicate");
    assert_eq!(pred.predicates.len(), 2);
    assert!(matches!(
        &pred.predicates[0],
        Predicate::Above { field, value: ValueRef::IntLit(18) } if field == "age"
    ));
    assert!(matches!(
        &pred.predicates[1],
        Predicate::Below { field, value: ValueRef::IntLit(100) } if field == "score"
    ));
}

#[test]
fn parse_predicate_and_conjunction() {
    let src = r#"
app "Test" version "1.0.0" {
  role admin;
  layer data as map {
    path data;
    namespace shared;
    allow admin to read where owner is self and status is "active";
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let pred = ast.layers[0].access[0]
        .predicate
        .as_ref()
        .expect("should have predicate");
    assert_eq!(pred.predicates.len(), 2);
}

#[test]
fn parse_relay_all_events() {
    let src = r#"
app "Test" version "1.0.0" {
  role admin;
  layer items as list {
    path items;
    namespace shared;
    allow admin to read, write, sync;
  }
  relay on items insert using "hooks.on_insert";
  relay on items update using "hooks.on_update";
  relay on items delete using "hooks.on_delete";
}
"#;
    let ast = parser::parse(src).expect("parse");
    assert_eq!(ast.relays.len(), 3);
    assert_eq!(ast.relays[0].event, RelayEvent::Insert);
    assert_eq!(ast.relays[1].event, RelayEvent::Update);
    assert_eq!(ast.relays[2].event, RelayEvent::Delete);
}

#[test]
fn parse_discover_on_grant() {
    let src = r#"
app "Test" version "1.0.0" {
  role admin;
  layer msgs as list {
    path dms / {id} / messages;
    namespace page;
    grant explicit;
    dynamic by id;
    create allow admin;
    discover on grant;
    allow admin to read, write, sync, grant, revoke;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    assert_eq!(ast.layers[0].discover, Some(DiscoverMode::Grant));
}

#[test]
fn parse_role_named_node() {
    // `node` is a keyword but must work as a role name
    let src = r#"
app "Test" version "1.0.0" {
  role node can relay, share;
  layer data as map {
    path data;
    namespace shared;
    allow node to read, write, sync;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    assert_eq!(ast.roles[0].name, "node");
    assert_eq!(ast.layers[0].access[0].roles, vec!["node"]);
}

#[test]
fn parse_broadcast_to_multiple_roles() {
    let src = r#"
app "Test" version "1.0.0" {
  role admin;
  role viewer;
  layer data as map {
    path data;
    namespace shared;
    allow admin, viewer to read, sync;
    broadcast to admin, viewer debounce 500;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let broadcast = &ast.layers[0].broadcasts[0];
    assert_eq!(
        broadcast.target,
        BroadcastTarget::ToRoles(vec!["admin".to_string(), "viewer".to_string()])
    );
    assert_eq!(broadcast.debounce, Some(500));
}

#[test]
fn parse_order_ascending() {
    let src = r#"
app "Test" version "1.0.0" {
  role admin;
  layer data as list {
    path data;
    namespace shared;
    allow admin to read, write, sync;
    order by name ascending;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let order = ast.layers[0].order.as_ref().unwrap();
    assert_eq!(order.field, "name");
    assert_eq!(order.direction, SortDirection::Ascending);
}

#[test]
fn parse_shard_without_period_in_path() {
    // v2: shard by day is sufficient — {period} in path is NOT required
    let src = r#"
app "Test" version "2.0.0" {
  role owner;
  layer logs as list {
    path logs;
    namespace shared;
    shard by day;
    allow owner to create, read, write, sync;
  }
}
"#;
    let ast = parser::parse(src).expect("parser should pass");
    semantic::check(&ast).expect("shard without {period} is valid in v2");
}

// ═══════════════════════════════════════════════════════════════════════════
// Compile / lowering tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn compile_source_produces_correct_schemas() {
    let src = r#"
app "Shop" version "2.0.0" {
  role owner can share, delegate, manage_access;
  role customer;

  layer orders as list {
    path orders / {period} / {id};
    namespace shared;
    grant explicit;
    shard by day;
    cache ui_window(200), sync_mode(incremental), retention_days(30);
    allow owner to create, read, write, sync, grant, revoke;
    allow customer to read;
  }

  derive orders_summary as map {
    from orders;
    cache ui_window(100);
    using "rules.compute_summary";
  }

  ui_app "Owner Console" {
    allow owner;
  }
}
"#;
    let compiled = compile_source(src).expect("compile should succeed");
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
fn compile_lowering_uses_layer_path_as_resource() {
    let src = r#"
app "Test" version "2.0.0" {
  role admin;
  layer data as map {
    path my_data;
    namespace shared;
    allow admin to read, write;
  }
}
"#;
    let compiled = compile_source(src).expect("compile");
    let rule = &compiled.permit.osv_policy.policy_rules[0];
    match &rule.resource {
        policy_model::ResourceSelector::Layer { name } => {
            assert_eq!(name, "my_data");
        }
        _ => panic!("expected Layer resource"),
    }
}

#[test]
fn compile_lowers_layer_where_predicate_to_policy_condition() {
    let src = r#"
app "Test" version "2.0.0" {
  role admin;
  layer records as map {
    path records;
    namespace shared;
    allow admin to read where owner is self and mode is node;
  }
}
"#;

    let compiled = compile_source(src).expect("compile");
    let rule = &compiled.permit.osv_policy.policy_rules[0];
    let condition = rule.condition.as_ref().expect("missing condition");

    match condition {
        policy_model::ConditionExpr::And { args } => {
            assert_eq!(args.len(), 2);
        }
        _ => panic!("expected AND condition"),
    }
}

#[test]
fn compile_lowers_permit_allow_predicate_to_policy_condition() {
    let src = r#"
app "Test" version "2.0.0" {
  role owner;
  role customer;

  layer orders as list {
    path orders / {customer} / entries;
    namespace page;
    allow owner to read, write, sync;
    allow customer to read, write, sync;
  }

  permit customer_permit for customer {
    allow read, write on orders where customer is self;
    allow sync on orders;
  }
}
"#;

    let compiled = compile_source(src).expect("compile");
    let permit_rule = compiled
        .permit
        .osv_policy
        .policy_rules
        .iter()
        .find(|rule| {
            matches!(&rule.subjects[0], policy_model::Subject::Role { name } if name == "customer")
                && matches!(
                    &rule.resource,
                    policy_model::ResourceSelector::Layer { name } if name == "orders/{customer}/entries"
                )
                && rule.actions.contains(&policy_model::Action::Read)
                && rule.actions.contains(&policy_model::Action::Write)
                && !rule.actions.contains(&policy_model::Action::Sync)
        })
        .expect("expected customer permit read/write rule");

    assert!(permit_rule.condition.is_some());
}

#[test]
fn compile_emits_schema_artifact_with_entities_and_bindings() {
    let compiled = compile_source(group_chat_v2()).expect("compile");

    assert!(compiled.schema.entities.contains_key("Message"));
    assert!(compiled.schema.entities.contains_key("Channel"));
    assert_eq!(
        compiled
            .schema
            .layer_bindings
            .get("channels")
            .map(String::as_str),
        Some("Channel")
    );

    let message_schema = compiled
        .schema
        .entities
        .get("Message")
        .expect("Message schema");
    assert!(message_schema
        .fields
        .iter()
        .any(|f| f.name == "text" && f.required));
    assert!(message_schema
        .entity_rules
        .iter()
        .any(|rule| matches!(rule, osv_decl::lowering::EntityRuleSchema::OnDeleteReject)));
}

#[test]
fn compile_emits_validation_artifact_with_runtime_policies() {
    let compiled = compile_source(shop_v2()).expect("compile");

    assert!(compiled
        .validation
        .lua_validators
        .contains_key("orders/{customer}/entries"));
    assert!(!compiled.validation.relays.is_empty());
    assert!(!compiled.validation.derivations.is_empty());

    let dynamic = compiled
        .validation
        .dynamic_layers
        .get("orders/{customer}/entries")
        .expect("dynamic layer policy");
    assert_eq!(dynamic.dynamic_by, vec!["customer".to_string()]);
    assert_eq!(dynamic.create_allow, vec!["customer".to_string()]);

    let embedded_dynamic = compiled
        .permit
        .osv_policy
        .validation
        .as_ref()
        .and_then(|v| v.dynamic_layers.get("orders/{customer}/entries"))
        .expect("embedded dynamic layer policy");
    assert_eq!(embedded_dynamic.create_allow, vec!["customer".to_string()]);
    assert!(matches!(
        embedded_dynamic.discover,
        Some(policy_model::DiscoverMode::Sync)
    ));
}

// ═══════════════════════════════════════════════════════════════════════════
// Semantic error tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn semantic_fails_on_inheritance_cycle() {
    let src = r#"
app "Bad" version "2.0.0" {
  role a inherits b;
  role b inherits a;
}
"#;
    let ast = parser::parse(src).expect("parse");
    let errors = semantic::check(&ast).expect_err("should fail");
    assert!(errors.iter().any(|e| e.code == "E2201"));
}

#[test]
fn semantic_fails_period_in_path_without_shard() {
    let src = r#"
app "Bad" version "2.0.0" {
  role owner;
  layer logs as list {
    path logs / {period};
    namespace shared;
    allow owner to read;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let errors = semantic::check(&ast).expect_err("should fail");
    assert!(errors.iter().any(|e| e.code == "E2302"));
}

#[test]
fn semantic_fails_retention_without_sharding() {
    let src = r#"
app "Bad" version "2.0.0" {
  role owner;
  layer drafts as map {
    path drafts;
    namespace page;
    cache retention_days(7);
    allow owner to create, read, write, sync;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let errors = semantic::check(&ast).expect_err("should fail");
    assert!(errors.iter().any(|e| e.code == "E2303"));
}

#[test]
fn semantic_fails_explicit_grant_without_grant_actions() {
    let src = r#"
app "Bad" version "2.0.0" {
  role owner;
  layer orders as list {
    path orders;
    namespace shared;
    grant explicit;
    allow owner to create, read, write, sync;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let errors = semantic::check(&ast).expect_err("should fail");
    assert!(errors.iter().any(|e| e.code == "E2304"));
}

#[test]
fn semantic_fails_duplicate_sthithi() {
    let src = r#"
app "Bad" version "2.0.0" {
  sthithi Item { field name; }
  sthithi Item { field other; }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let errors = semantic::check(&ast).expect_err("should fail");
    assert!(errors.iter().any(|e| e.code == "E2004"));
}

#[test]
fn semantic_fails_duplicate_permit() {
    let src = r#"
app "Bad" version "2.0.0" {
  role admin;
  layer data as map {
    path data;
    namespace shared;
    allow admin to read, write, sync;
  }
  permit admin_permit for admin {
    allow read on data;
  }
  permit admin_permit for admin {
    allow write on data;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let errors = semantic::check(&ast).expect_err("should fail");
    assert!(errors.iter().any(|e| e.code == "E2005"));
}

#[test]
fn semantic_fails_duplicate_field_in_sthithi() {
    let src = r#"
app "Bad" version "2.0.0" {
  sthithi X {
    field name;
    field name;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let errors = semantic::check(&ast).expect_err("should fail");
    assert!(errors.iter().any(|e| e.code == "E2501"));
}

#[test]
fn semantic_fails_transition_unknown_field() {
    let src = r#"
app "Bad" version "2.0.0" {
  role admin;
  sthithi X {
    field name;
    transitions nonexistent {
      "a" to "b" by admin;
    }
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let errors = semantic::check(&ast).expect_err("should fail");
    assert!(errors.iter().any(|e| e.code == "E2502"));
}

#[test]
fn semantic_fails_transition_unknown_role() {
    let src = r#"
app "Bad" version "2.0.0" {
  role admin;
  sthithi X {
    field status;
    transitions status {
      "a" to "b" by ghost_role;
    }
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let errors = semantic::check(&ast).expect_err("should fail");
    assert!(errors.iter().any(|e| e.code == "E2503"));
}

#[test]
fn semantic_fails_entity_rule_unknown_field() {
    let src = r#"
app "Bad" version "2.0.0" {
  sthithi X {
    field name;
    on update set nonexistent from clock;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let errors = semantic::check(&ast).expect_err("should fail");
    assert!(errors.iter().any(|e| e.code == "E2504"));
}

#[test]
fn semantic_fails_unknown_entity_binding() {
    let src = r#"
app "Bad" version "2.0.0" {
  role admin;
  layer items as list for NonExistent {
    path items;
    namespace shared;
    allow admin to read, write, sync;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let errors = semantic::check(&ast).expect_err("should fail");
    assert!(errors.iter().any(|e| e.code == "E2505"));
}

#[test]
fn semantic_fails_dynamic_by_unknown_field() {
    let src = r#"
app "Bad" version "2.0.0" {
  role admin;
  sthithi Item { field name; }
  layer items as list for Item {
    path items / {id};
    namespace shared;
    dynamic by ghost_field;
    create allow admin;
    discover on sync;
    allow admin to read, write, sync;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let errors = semantic::check(&ast).expect_err("should fail");
    assert!(errors.iter().any(|e| e.code == "E2506"));
}

#[test]
fn semantic_fails_order_by_unknown_field() {
    let src = r#"
app "Bad" version "2.0.0" {
  role admin;
  sthithi Item { field name; }
  layer items as list for Item {
    path items;
    namespace shared;
    allow admin to read, write, sync;
    order by nonexistent descending;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let errors = semantic::check(&ast).expect_err("should fail");
    assert!(errors.iter().any(|e| e.code == "E2507"));
}

#[test]
fn semantic_fails_relay_unknown_layer() {
    let src = r#"
app "Bad" version "2.0.0" {
  relay on ghost_layer insert using "hooks.noop";
}
"#;
    let ast = parser::parse(src).expect("parse");
    let errors = semantic::check(&ast).expect_err("should fail");
    assert!(errors.iter().any(|e| e.code == "E2601"));
}

#[test]
fn semantic_fails_validate_unknown_layer() {
    let src = r#"
app "Bad" version "2.0.0" {
  validate ghost_layer using "hooks.noop";
}
"#;
    let ast = parser::parse(src).expect("parse");
    let errors = semantic::check(&ast).expect_err("should fail");
    assert!(errors.iter().any(|e| e.code == "E2602"));
}

#[test]
fn semantic_fails_permit_unknown_role() {
    let src = r#"
app "Bad" version "2.0.0" {
  role admin;
  layer data as map {
    path data;
    namespace shared;
    allow admin to read;
  }
  permit ghost_permit for ghost_role {
    allow read on data;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let errors = semantic::check(&ast).expect_err("should fail");
    assert!(errors.iter().any(|e| e.code == "E2603"));
}

#[test]
fn semantic_fails_permit_allow_unknown_layer() {
    let src = r#"
app "Bad" version "2.0.0" {
  role admin;
  permit admin_permit for admin {
    allow read on ghost_layer;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let errors = semantic::check(&ast).expect_err("should fail");
    assert!(errors.iter().any(|e| e.code == "E2604"));
}

#[test]
fn semantic_fails_permit_issue_unknown_role() {
    let src = r#"
app "Bad" version "2.0.0" {
  role admin;
  layer data as map {
    path data;
    namespace shared;
    allow admin to read;
  }
  permit admin_permit for admin {
    allow read on data;
    issue for ghost_role;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let errors = semantic::check(&ast).expect_err("should fail");
    assert!(errors.iter().any(|e| e.code == "E2605"));
}

#[test]
fn semantic_fails_discover_without_dynamic() {
    let src = r#"
app "Bad" version "2.0.0" {
  role admin;
  layer data as map {
    path data;
    namespace shared;
    discover on sync;
    allow admin to read;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let errors = semantic::check(&ast).expect_err("should fail");
    assert!(errors.iter().any(|e| e.code == "E2701"));
}

#[test]
fn semantic_fails_create_allow_without_dynamic() {
    let src = r#"
app "Bad" version "2.0.0" {
  role admin;
  layer data as map {
    path data;
    namespace shared;
    create allow admin;
    allow admin to read;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let errors = semantic::check(&ast).expect_err("should fail");
    assert!(errors.iter().any(|e| e.code == "E2702"));
}

#[test]
fn semantic_fails_broadcast_unknown_role() {
    let src = r#"
app "Bad" version "2.0.0" {
  role admin;
  layer data as map {
    path data;
    namespace shared;
    allow admin to read;
    broadcast to ghost_role;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let errors = semantic::check(&ast).expect_err("should fail");
    assert!(errors.iter().any(|e| e.code == "E2102"));
}

#[test]
fn semantic_fails_create_allow_unknown_role() {
    let src = r#"
app "Bad" version "2.0.0" {
  role admin;
  layer data as map {
    path data / {id};
    namespace shared;
    dynamic by id;
    create allow ghost_role;
    discover on sync;
    allow admin to read;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let errors = semantic::check(&ast).expect_err("should fail");
    assert!(errors.iter().any(|e| e.code == "E2102"));
}

// ═══════════════════════════════════════════════════════════════════════════
// Parser error tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn parser_fails_on_unknown_top_level_decl() {
    let src = r#"
app "Bad" version "2.0.0" {
  banana x;
}
"#;
    let err = parser::parse(src).expect_err("should fail");
    assert_eq!(err.code, "E1101");
}

#[test]
fn parser_fails_sthithi_unknown_member() {
    let src = r#"
app "Bad" version "2.0.0" {
  sthithi X {
    banana y;
  }
}
"#;
    let err = parser::parse(src).expect_err("should fail");
    assert_eq!(err.code, "E1118");
}

#[test]
fn parser_fails_field_unknown_modifier() {
    let src = r#"
app "Bad" version "2.0.0" {
  sthithi X {
    field name banana;
  }
}
"#;
    let err = parser::parse(src).expect_err("should fail");
    assert_eq!(err.code, "E1119");
}

#[test]
fn parser_fails_field_unknown_source() {
    let src = r#"
app "Bad" version "2.0.0" {
  sthithi X {
    field name from banana;
  }
}
"#;
    let err = parser::parse(src).expect_err("should fail");
    assert_eq!(err.code, "E1120");
}

#[test]
fn parser_fails_entity_rule_unknown_event() {
    let src = r#"
app "Bad" version "2.0.0" {
  sthithi X {
    field name;
    on banana set name from clock;
  }
}
"#;
    let err = parser::parse(src).expect_err("should fail");
    assert_eq!(err.code, "E1122");
}

#[test]
fn parser_fails_broadcast_unknown_target() {
    let src = r#"
app "Bad" version "2.0.0" {
  role admin;
  layer data as map {
    path data;
    namespace shared;
    allow admin to read;
    broadcast banana;
  }
}
"#;
    let err = parser::parse(src).expect_err("should fail");
    assert_eq!(err.code, "E1127");
}

#[test]
fn parser_fails_order_unknown_direction() {
    let src = r#"
app "Bad" version "2.0.0" {
  role admin;
  layer data as list {
    path data;
    namespace shared;
    allow admin to read;
    order by name banana;
  }
}
"#;
    let err = parser::parse(src).expect_err("should fail");
    assert_eq!(err.code, "E1128");
}

#[test]
fn parser_fails_discover_unknown_mode() {
    let src = r#"
app "Bad" version "2.0.0" {
  role admin;
  layer data as map {
    path data;
    namespace shared;
    dynamic by id;
    discover on banana;
    allow admin to read;
  }
}
"#;
    let err = parser::parse(src).expect_err("should fail");
    assert_eq!(err.code, "E1129");
}

#[test]
fn parser_fails_relay_unknown_event() {
    let src = r#"
app "Bad" version "2.0.0" {
  role admin;
  layer items as list {
    path items;
    namespace shared;
    allow admin to read;
  }
  relay on items banana using "hooks.noop";
}
"#;
    let err = parser::parse(src).expect_err("should fail");
    assert_eq!(err.code, "E1130");
}

#[test]
fn parser_fails_permit_unknown_statement() {
    let src = r#"
app "Bad" version "2.0.0" {
  role admin;
  permit admin_permit for admin {
    banana read on data;
  }
}
"#;
    let err = parser::parse(src).expect_err("should fail");
    assert_eq!(err.code, "E1131");
}

#[test]
fn parser_fails_predicate_unknown_operator() {
    let src = r#"
app "Bad" version "2.0.0" {
  role admin;
  layer data as map {
    path data;
    namespace shared;
    allow admin to read where status banana "active";
  }
}
"#;
    let err = parser::parse(src).expect_err("should fail");
    assert_eq!(err.code, "E1125");
}

#[test]
fn parser_fails_missing_literal_value() {
    let src = r#"
app "Bad" version "2.0.0" {
  sthithi X {
    field status default;
  }
}
"#;
    let err = parser::parse(src).expect_err("should fail");
    assert_eq!(err.code, "E1121");
}

#[test]
fn parser_rejects_string_path() {
    // v2 does not support string paths — only structural
    let src = r#"
app "Bad" version "2.0.0" {
  role admin;
  layer data as map {
    path "legacy/path";
    namespace shared;
    allow admin to read;
  }
}
"#;
    let err = parser::parse(src).expect_err("string paths should be rejected");
    assert_eq!(err.code, "E1191"); // expected identifier, got string
}

#[test]
fn parser_rejects_namespace_creator() {
    // v2 does not support namespace creator — use page
    let src = r#"
app "Bad" version "2.0.0" {
  role admin;
  layer data as map {
    path data;
    namespace creator;
    allow admin to read;
  }
}
"#;
    let err = parser::parse(src).expect_err("namespace creator should be rejected");
    assert_eq!(err.code, "E1114");
}

// ═══════════════════════════════════════════════════════════════════════════
// Lexer tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn lexer_slash_token() {
    use osv_decl::lexer::{lex, TokenKind};
    let tokens = lex("channels / {id} / messages").expect("lex");
    assert!(matches!(&tokens[0].kind, TokenKind::Ident(s) if s == "channels"));
    assert!(matches!(tokens[1].kind, TokenKind::Slash));
    assert!(matches!(tokens[2].kind, TokenKind::LBrace));
    assert!(matches!(&tokens[3].kind, TokenKind::Ident(s) if s == "id"));
    assert!(matches!(tokens[4].kind, TokenKind::RBrace));
    assert!(matches!(tokens[5].kind, TokenKind::Slash));
}

#[test]
fn lexer_comment_still_works() {
    use osv_decl::lexer::{lex, TokenKind};
    let tokens = lex("// comment\napp").expect("lex");
    assert!(matches!(&tokens[0].kind, TokenKind::Keyword(s) if s == "app"));
}

#[test]
fn lexer_v2_keywords() {
    use osv_decl::lexer::{lex, TokenKind};
    let keywords = vec![
        "sthithi",
        "field",
        "required",
        "immutable",
        "default",
        "transitions",
        "where",
        "is",
        "not",
        "and",
        "in",
        "above",
        "below",
        "broadcast",
        "all",
        "granted",
        "order",
        "ascending",
        "descending",
        "permit",
        "issue",
        "for",
        "dynamic",
        "discover",
        "retain",
        "debounce",
        "reject",
        "set",
        "update",
        "delete",
        "insert",
        "mode",
        "peer",
        "page",
        "validate",
        "node",
        "user",
        "true",
        "false",
        "null",
        "clock",
    ];
    for kw in keywords {
        let tokens = lex(kw).expect(&format!("should lex '{}'", kw));
        assert!(
            matches!(&tokens[0].kind, TokenKind::Keyword(s) if s == kw),
            "'{}' should be a keyword, got {:?}",
            kw,
            tokens[0].kind
        );
    }
}

#[test]
fn lexer_star_is_rejected() {
    use osv_decl::lexer::lex;
    let err = lex("*").expect_err("star should be rejected");
    assert_eq!(err.code, "E1005");
}

#[test]
fn lexer_creator_is_not_a_keyword() {
    use osv_decl::lexer::{lex, TokenKind};
    let tokens = lex("creator").expect("lex");
    assert!(
        matches!(&tokens[0].kind, TokenKind::Ident(s) if s == "creator"),
        "creator should be an ident, not a keyword"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Semantic success tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn semantic_v2_group_chat_passes() {
    let ast = parser::parse(group_chat_v2()).expect("parse");
    semantic::check(&ast).expect("v2 group chat should pass semantic validation");
}

#[test]
fn semantic_v2_shop_passes() {
    let ast = parser::parse(shop_v2()).expect("parse");
    semantic::check(&ast).expect("v2 shop should pass semantic validation");
}

#[test]
fn semantic_valid_entity_binding_and_dynamic() {
    let src = r#"
app "Test" version "2.0.0" {
  role admin;
  sthithi Msg {
    field text required;
    field channel_id;
    field sent_at from clock;
  }
  layer messages as list for Msg {
    path channels / {channel_id} / messages;
    namespace shared;
    dynamic by channel_id;
    create allow admin;
    discover on sync;
    allow admin to read, write, sync;
    order by sent_at descending;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    semantic::check(&ast).expect("should pass semantic validation");
}

// ═══════════════════════════════════════════════════════════════════════════
// Edge cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn parse_empty_sthithi() {
    let src = r#"
app "Test" version "2.0.0" {
  sthithi Empty {
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    assert_eq!(ast.sthithis[0].fields.len(), 0);
    assert_eq!(ast.sthithis[0].transitions.len(), 0);
    assert_eq!(ast.sthithis[0].entity_rules.len(), 0);
}

#[test]
fn parse_empty_permit() {
    let src = r#"
app "Test" version "2.0.0" {
  role admin;
  permit empty for admin {
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    assert_eq!(ast.permits[0].statements.len(), 0);
}

#[test]
fn parse_layer_minimal() {
    let src = r#"
app "Test" version "2.0.0" {
  role admin;
  layer data as map {
    path data;
    namespace shared;
    allow admin to read, write, sync;
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let layer = &ast.layers[0];
    assert!(layer.entity_binding.is_none());
    assert!(layer.dynamic_by.is_none());
    assert!(layer.create_allow.is_none());
    assert!(layer.discover.is_none());
    assert!(layer.retain.is_none());
    assert!(layer.broadcasts.is_empty());
    assert!(layer.order.is_none());
    assert!(layer.access[0].predicate.is_none());
}

#[test]
fn parse_multiple_transitions_on_different_fields() {
    let src = r#"
app "Test" version "2.0.0" {
  role admin;
  role user;
  sthithi Task {
    field status default "open";
    field priority;

    transitions status {
      "open" to "in_progress" by admin, user;
      "in_progress" to "done" by admin;
      "open" to "cancelled" by admin;
    }

    transitions priority {
      "low" to "high" by admin;
      "high" to "low" by admin;
    }
  }
}
"#;
    let ast = parser::parse(src).expect("parse");
    let task = &ast.sthithis[0];
    assert_eq!(task.transitions.len(), 2);
    assert_eq!(task.transitions[0].field, "status");
    assert_eq!(task.transitions[0].rules.len(), 3);
    assert_eq!(task.transitions[1].field, "priority");
    assert_eq!(task.transitions[1].rules.len(), 2);
}

#[test]
fn parse_empty_app() {
    let src = r#"
app "Empty" version "2.0.0" {
}
"#;
    let ast = parser::parse(src).expect("parse");
    assert_eq!(ast.name, "Empty");
    assert!(ast.roles.is_empty());
    assert!(ast.sthithis.is_empty());
    assert!(ast.layers.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// Sample app compilation tests
// ═══════════════════════════════════════════════════════════════════════════

fn read_sample_app(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("sample_apps")
        .join(name)
        .join("app.osv");
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("Failed to read {}: {}", path.display(), e);
    })
}

#[test]
fn sample_app_group_chat_compiles() {
    let src = read_sample_app("group-chat");
    let compiled = compile_source(&src).expect("group-chat app.osv should compile");
    assert_eq!(compiled.runtime.app_name, "group-chat");
    assert_eq!(compiled.runtime.ui_apps.len(), 1);
    assert_eq!(compiled.runtime.ui_apps[0].layer_name, "group_chat");
}

#[test]
fn sample_app_osvauld_demos_compiles() {
    let src = read_sample_app("osvauld-demos");
    let compiled = compile_source(&src).expect("osvauld-demos app.osv should compile");
    assert_eq!(compiled.runtime.app_name, "osvauld-demos");
    assert_eq!(compiled.runtime.ui_apps.len(), 10);
    assert_eq!(
        compiled.runtime.app_layer_names,
        vec![
            "group_chat",
            "sthalam_guide",
            "math_simulation",
            "shared",
            "snake_game",
            "tank_game",
            "protocol_docs",
            "snake_raylib",
            "sthalam",
            "tank_raylib",
        ]
    );
}

#[test]
fn sample_app_my_booking_compiles() {
    let src = read_sample_app("my-booking");
    let compiled = compile_source(&src).expect("my-booking app.osv should compile");
    assert_eq!(compiled.runtime.app_name, "my-booking");
    assert_eq!(compiled.runtime.ui_apps.len(), 2);
}

#[test]
fn sample_app_my_shop_compiles() {
    let src = read_sample_app("my-shop");
    let compiled = compile_source(&src).expect("my-shop app.osv should compile");
    assert_eq!(compiled.runtime.app_name, "my-shop");
    assert_eq!(compiled.runtime.ui_apps.len(), 3);
}
