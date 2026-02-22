use crate::ast::*;
use crate::diagnostics::{Diagnostic, Span};
use crate::lexer::{lex, Token, TokenKind};

pub fn parse(source: &str) -> Result<AppSpec, Diagnostic> {
    let tokens = lex(source)?;
    let mut p = Parser { tokens, idx: 0 };
    p.parse_file()
}

struct Parser {
    tokens: Vec<Token>,
    idx: usize,
}

impl Parser {
    // ── Top Level ────────────────────────────────────────────────────────

    fn parse_file(&mut self) -> Result<AppSpec, Diagnostic> {
        self.expect_kw("app")?;
        let name = self.expect_string()?;
        self.expect_kw("version")?;
        let version = self.expect_string()?;
        self.expect(TokenKind::LBrace)?;

        let mut roles = Vec::new();
        let mut sthithis = Vec::new();
        let mut layers = Vec::new();
        let mut derives = Vec::new();
        let mut relays = Vec::new();
        let mut validates = Vec::new();
        let mut permits = Vec::new();
        let mut ui_apps = Vec::new();

        while !self.is(TokenKind::RBrace) {
            if self.is_kw("role") {
                roles.push(self.parse_role_decl()?);
            } else if self.is_kw("sthithi") {
                sthithis.push(self.parse_sthithi_decl()?);
            } else if self.is_kw("layer") {
                layers.push(self.parse_layer_decl()?);
            } else if self.is_kw("derive") {
                derives.push(self.parse_derive_decl()?);
            } else if self.is_kw("relay") {
                relays.push(self.parse_relay_decl()?);
            } else if self.is_kw("validate") {
                validates.push(self.parse_validate_decl()?);
            } else if self.is_kw("permit") {
                permits.push(self.parse_permit_decl()?);
            } else if self.is_kw("ui_app") {
                ui_apps.push(self.parse_ui_app_decl()?);
            } else {
                return Err(Diagnostic::new(
                    "E1101",
                    "expected top-level declaration: role, sthithi, layer, derive, relay, validate, permit, or ui_app",
                    Some(self.current_span()),
                ));
            }
        }

        self.expect(TokenKind::RBrace)?;
        self.expect(TokenKind::Eof)?;

        Ok(AppSpec {
            name,
            version,
            roles,
            sthithis,
            layers,
            derives,
            relays,
            validates,
            permits,
            ui_apps,
        })
    }

    // ── Role Declaration ─────────────────────────────────────────────────

    fn parse_role_decl(&mut self) -> Result<RoleDecl, Diagnostic> {
        self.expect_kw("role")?;
        let name = self.expect_ident()?;
        let mut inherits = None;
        let mut capabilities = Vec::new();

        if self.is_kw("inherits") {
            self.bump();
            inherits = Some(self.expect_ident()?);
        }

        if self.is_kw("can") {
            self.bump();
            loop {
                capabilities.push(self.parse_capability()?);
                if self.is(TokenKind::Comma) {
                    self.bump();
                    continue;
                }
                break;
            }
        }

        self.expect(TokenKind::Semi)?;
        Ok(RoleDecl {
            name,
            inherits,
            capabilities,
        })
    }

    // ── Sthithi (Entity Schema) Declaration ──────────────────────────────

    fn parse_sthithi_decl(&mut self) -> Result<SthithiDecl, Diagnostic> {
        self.expect_kw("sthithi")?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::LBrace)?;

        let mut fields = Vec::new();
        let mut transitions = Vec::new();
        let mut entity_rules = Vec::new();

        while !self.is(TokenKind::RBrace) {
            if self.is_kw("field") {
                fields.push(self.parse_field_decl()?);
            } else if self.is_kw("transitions") {
                transitions.push(self.parse_transition_block()?);
            } else if self.is_kw("on") {
                entity_rules.push(self.parse_entity_rule()?);
            } else {
                return Err(Diagnostic::new(
                    "E1118",
                    "expected sthithi member: field, transitions, or on",
                    Some(self.current_span()),
                ));
            }
        }

        self.expect(TokenKind::RBrace)?;

        Ok(SthithiDecl {
            name,
            fields,
            transitions,
            entity_rules,
        })
    }

    fn parse_field_decl(&mut self) -> Result<FieldDecl, Diagnostic> {
        self.expect_kw("field")?;
        let name = self.expect_ident()?;

        let mut required = false;
        let mut immutable = false;
        let mut default = None;
        let mut source = None;

        // Parse field modifiers in any order until `;`
        while !self.is(TokenKind::Semi) {
            if self.is_kw("required") {
                self.bump();
                required = true;
            } else if self.is_kw("immutable") {
                self.bump();
                immutable = true;
            } else if self.is_kw("default") {
                self.bump();
                default = Some(self.parse_literal()?);
            } else if self.is_kw("from") {
                self.bump();
                source = Some(self.parse_field_source()?);
            } else {
                return Err(Diagnostic::new(
                    "E1119",
                    "expected field modifier: required, immutable, default, or from",
                    Some(self.current_span()),
                ));
            }
        }

        self.expect(TokenKind::Semi)?;

        Ok(FieldDecl {
            name,
            required,
            immutable,
            default,
            source,
        })
    }

    fn parse_field_source(&mut self) -> Result<FieldSource, Diagnostic> {
        if self.is_kw("clock") {
            self.bump();
            return Ok(FieldSource::Clock);
        }
        if self.is_kw("peer") {
            self.bump();
            return Ok(FieldSource::Peer);
        }
        if self.is_kw("mode") {
            self.bump();
            return Ok(FieldSource::Mode);
        }
        if self.is_kw("self") {
            self.bump();
            return Ok(FieldSource::SelfRef);
        }
        Err(Diagnostic::new(
            "E1120",
            "expected field source: clock, peer, mode, or self",
            Some(self.current_span()),
        ))
    }

    fn parse_literal(&mut self) -> Result<Literal, Diagnostic> {
        let span = self.current_span();
        if let TokenKind::StringLit(s) = &self.current().kind {
            let out = s.clone();
            self.bump();
            return Ok(Literal::String(out));
        }
        if let TokenKind::IntLit(n) = self.current().kind {
            self.bump();
            return Ok(Literal::Int(n));
        }
        if self.is_kw("true") {
            self.bump();
            return Ok(Literal::Bool(true));
        }
        if self.is_kw("false") {
            self.bump();
            return Ok(Literal::Bool(false));
        }
        if self.is_kw("null") {
            self.bump();
            return Ok(Literal::Null);
        }
        Err(Diagnostic::new(
            "E1121",
            "expected literal value: string, integer, true, false, or null",
            Some(span),
        ))
    }

    // ── Transitions ──────────────────────────────────────────────────────

    fn parse_transition_block(&mut self) -> Result<TransitionBlock, Diagnostic> {
        self.expect_kw("transitions")?;
        let field = self.expect_ident()?;
        self.expect(TokenKind::LBrace)?;

        let mut rules = Vec::new();
        while !self.is(TokenKind::RBrace) {
            rules.push(self.parse_transition_rule()?);
        }

        self.expect(TokenKind::RBrace)?;

        Ok(TransitionBlock { field, rules })
    }

    fn parse_transition_rule(&mut self) -> Result<TransitionRule, Diagnostic> {
        let from_state = self.expect_string()?;
        self.expect_kw("to")?;
        let to_state = self.expect_string()?;
        self.expect_kw("by")?;

        let mut allowed_roles = vec![self.expect_ident()?];
        while self.is(TokenKind::Comma) {
            self.bump();
            allowed_roles.push(self.expect_ident()?);
        }

        let predicate = if self.is_kw("where") {
            Some(self.parse_predicate_expr()?)
        } else {
            None
        };

        self.expect(TokenKind::Semi)?;

        Ok(TransitionRule {
            from_state,
            to_state,
            allowed_roles,
            predicate,
        })
    }

    // ── Entity Rules ─────────────────────────────────────────────────────

    fn parse_entity_rule(&mut self) -> Result<EntityRule, Diagnostic> {
        self.expect_kw("on")?;

        if self.is_kw("update") {
            self.bump();
            self.expect_kw("set")?;
            let field = self.expect_ident()?;
            let source = self.parse_entity_action_source()?;
            self.expect(TokenKind::Semi)?;
            Ok(EntityRule::OnUpdateSet { field, source })
        } else if self.is_kw("delete") {
            self.bump();
            self.expect_kw("reject")?;
            self.expect(TokenKind::Semi)?;
            Ok(EntityRule::OnDeleteReject)
        } else {
            Err(Diagnostic::new(
                "E1122",
                "expected entity event: update or delete",
                Some(self.current_span()),
            ))
        }
    }

    fn parse_entity_action_source(&mut self) -> Result<EntityActionSource, Diagnostic> {
        if self.is_kw("from") {
            self.bump();
            if self.is_kw("clock") {
                self.bump();
                return Ok(EntityActionSource::FromClock);
            }
            if self.is_kw("peer") {
                self.bump();
                return Ok(EntityActionSource::FromPeer);
            }
            if self.is_kw("mode") {
                self.bump();
                return Ok(EntityActionSource::FromMode);
            }
            return Err(Diagnostic::new(
                "E1123",
                "expected source after 'from': clock, peer, or mode",
                Some(self.current_span()),
            ));
        }
        if self.is_kw("to") {
            self.bump();
            let lit = self.parse_literal()?;
            return Ok(EntityActionSource::ToLiteral(lit));
        }
        Err(Diagnostic::new(
            "E1124",
            "expected entity action source: from <source> or to <literal>",
            Some(self.current_span()),
        ))
    }

    // ── Predicate Expressions ────────────────────────────────────────────

    fn parse_predicate_expr(&mut self) -> Result<PredicateExpr, Diagnostic> {
        self.expect_kw("where")?;

        let mut predicates = vec![self.parse_predicate()?];
        while self.is_kw("and") {
            self.bump();
            predicates.push(self.parse_predicate()?);
        }

        Ok(PredicateExpr { predicates })
    }

    fn parse_predicate(&mut self) -> Result<Predicate, Diagnostic> {
        let field = self.expect_ident()?;

        if self.is_kw("is") {
            self.bump();
            if self.is_kw("not") {
                self.bump();
                let value = self.parse_value_ref()?;
                return Ok(Predicate::IsNot { field, value });
            }
            let value = self.parse_value_ref()?;
            return Ok(Predicate::Is { field, value });
        }
        // `in` is a keyword
        if self.is_kw("in") {
            self.bump();
            self.expect(TokenKind::LParen)?;
            let mut values = vec![self.parse_value_ref()?];
            while self.is(TokenKind::Comma) {
                self.bump();
                values.push(self.parse_value_ref()?);
            }
            self.expect(TokenKind::RParen)?;
            return Ok(Predicate::In { field, values });
        }
        if self.is_kw("above") {
            self.bump();
            let value = self.parse_value_ref()?;
            return Ok(Predicate::Above { field, value });
        }
        if self.is_kw("below") {
            self.bump();
            let value = self.parse_value_ref()?;
            return Ok(Predicate::Below { field, value });
        }

        Err(Diagnostic::new(
            "E1125",
            "expected predicate operator: is, is not, in, above, or below",
            Some(self.current_span()),
        ))
    }

    fn parse_value_ref(&mut self) -> Result<ValueRef, Diagnostic> {
        let span = self.current_span();
        if self.is_kw("self") {
            self.bump();
            return Ok(ValueRef::SelfRef);
        }
        if self.is_kw("peer") {
            self.bump();
            return Ok(ValueRef::Peer);
        }
        if self.is_kw("user") {
            self.bump();
            return Ok(ValueRef::User);
        }
        if self.is_kw("node") {
            self.bump();
            return Ok(ValueRef::Node);
        }
        if self.is_kw("true") {
            self.bump();
            return Ok(ValueRef::BoolTrue);
        }
        if self.is_kw("false") {
            self.bump();
            return Ok(ValueRef::BoolFalse);
        }
        if let TokenKind::StringLit(s) = &self.current().kind {
            let out = s.clone();
            self.bump();
            return Ok(ValueRef::StringLit(out));
        }
        if let TokenKind::IntLit(n) = self.current().kind {
            self.bump();
            return Ok(ValueRef::IntLit(n));
        }
        Err(Diagnostic::new(
            "E1126",
            "expected value reference: self, peer, user, node, string, integer, true, or false",
            Some(span),
        ))
    }

    // ── Layer Declaration ────────────────────────────────────────────────

    fn parse_layer_decl(&mut self) -> Result<LayerDecl, Diagnostic> {
        self.expect_kw("layer")?;
        let name = self.expect_ident()?;
        self.expect_kw("as")?;
        let kind = self.parse_layer_kind()?;

        // Optional: `for EntityName` (v2 entity binding)
        let entity_binding = if self.is_kw("for") {
            self.bump();
            Some(self.expect_ident()?)
        } else {
            None
        };

        self.expect(TokenKind::LBrace)?;

        let mut path = None;
        let mut namespace = None;
        let mut grant = GrantMode::Open;
        let mut time = TimeModel::Unsharded;
        let mut cache = Vec::new();
        let mut access = Vec::new();
        let mut dynamic_by = None;
        let mut create_allow = None;
        let mut discover = None;
        let mut retain = None;
        let mut broadcasts = Vec::new();
        let mut order = None;

        while !self.is(TokenKind::RBrace) {
            if self.is_kw("path") {
                self.bump();
                path = Some(self.parse_path()?);
                self.expect(TokenKind::Semi)?;
            } else if self.is_kw("namespace") {
                self.bump();
                namespace = Some(self.parse_namespace()?);
                self.expect(TokenKind::Semi)?;
            } else if self.is_kw("grant") {
                self.bump();
                grant = self.parse_grant_mode()?;
                self.expect(TokenKind::Semi)?;
            } else if self.is_kw("shard") {
                self.bump();
                self.expect_kw("by")?;
                let resolution = self.parse_resolution()?;
                time = TimeModel::TimeSharded {
                    resolution,
                    period_var: "period".to_string(),
                };
                self.expect(TokenKind::Semi)?;
            } else if self.is_kw("cache") {
                self.bump();
                cache = self.parse_cache_spec()?;
                self.expect(TokenKind::Semi)?;
            } else if self.is_kw("allow") {
                access.push(self.parse_access_decl()?);
            } else if self.is_kw("dynamic") {
                self.bump();
                self.expect_kw("by")?;
                let mut fields = vec![self.expect_ident()?];
                while self.is(TokenKind::Comma) {
                    self.bump();
                    fields.push(self.expect_ident()?);
                }
                dynamic_by = Some(fields);
                self.expect(TokenKind::Semi)?;
            } else if self.is_kw("create") {
                self.bump();
                self.expect_kw("allow")?;
                let mut roles = vec![self.expect_ident()?];
                while self.is(TokenKind::Comma) {
                    self.bump();
                    roles.push(self.expect_ident()?);
                }
                create_allow = Some(roles);
                self.expect(TokenKind::Semi)?;
            } else if self.is_kw("discover") {
                self.bump();
                self.expect_kw("on")?;
                discover = Some(self.parse_discover_mode()?);
                self.expect(TokenKind::Semi)?;
            } else if self.is_kw("retain") {
                self.bump();
                retain = Some(self.expect_int()?);
                self.expect(TokenKind::Semi)?;
            } else if self.is_kw("broadcast") {
                broadcasts.push(self.parse_broadcast_stmt()?);
            } else if self.is_kw("order") {
                self.bump();
                self.expect_kw("by")?;
                let field = self.expect_ident()?;
                let direction = self.parse_sort_direction()?;
                order = Some(OrderStmt { field, direction });
                self.expect(TokenKind::Semi)?;
            } else {
                return Err(Diagnostic::new(
                    "E1102",
                    "expected layer field: path, namespace, grant, shard, cache, allow, dynamic, create, discover, retain, broadcast, or order",
                    Some(self.current_span()),
                ));
            }
        }

        self.expect(TokenKind::RBrace)?;

        let path = path.ok_or_else(|| {
            Diagnostic::new(
                "E1103",
                "layer missing required field 'path'",
                Some(self.current_span()),
            )
        })?;
        let namespace = namespace.ok_or_else(|| {
            Diagnostic::new(
                "E1104",
                "layer missing required field 'namespace'",
                Some(self.current_span()),
            )
        })?;

        Ok(LayerDecl {
            name,
            kind,
            entity_binding,
            path,
            namespace,
            grant,
            time,
            cache,
            access,
            dynamic_by,
            create_allow,
            discover,
            retain,
            broadcasts,
            order,
        })
    }

    /// Parse a v2 structural path: `ident_or_placeholder { "/" ident_or_placeholder }`.
    fn parse_path(&mut self) -> Result<String, Diagnostic> {
        self.parse_structural_path()
    }

    /// Parse v2 structural path: `ident_or_placeholder { "/" ident_or_placeholder }`.
    /// Normalizes to a string like "orders/{customer}/entries".
    fn parse_structural_path(&mut self) -> Result<String, Diagnostic> {
        let mut segments = vec![self.parse_path_segment()?];
        while self.is(TokenKind::Slash) {
            self.bump();
            segments.push(self.parse_path_segment()?);
        }
        Ok(segments.join("/"))
    }

    /// Parse a single path segment: `ident` or `"{" ident "}"`.
    fn parse_path_segment(&mut self) -> Result<String, Diagnostic> {
        if self.is(TokenKind::LBrace) {
            self.bump();
            let name = self.expect_ident()?;
            self.expect(TokenKind::RBrace)?;
            Ok(format!("{{{}}}", name))
        } else {
            self.expect_ident()
        }
    }

    fn parse_access_decl(&mut self) -> Result<AccessRule, Diagnostic> {
        self.expect_kw("allow")?;

        let mut roles = vec![self.expect_ident()?];
        while self.is(TokenKind::Comma) {
            self.bump();
            // Stop consuming if next token is a keyword that starts an action
            // (handles `allow owner, node to ...`)
            roles.push(self.expect_ident()?);
        }

        self.expect_kw("to")?;
        let mut actions = vec![self.parse_action()?];
        while self.is(TokenKind::Comma) {
            self.bump();
            actions.push(self.parse_action()?);
        }

        let predicate = if self.is_kw("where") {
            Some(self.parse_predicate_expr()?)
        } else {
            None
        };

        self.expect(TokenKind::Semi)?;
        Ok(AccessRule {
            roles,
            actions,
            predicate,
        })
    }

    // ── Broadcast ────────────────────────────────────────────────────────

    fn parse_broadcast_stmt(&mut self) -> Result<BroadcastStmt, Diagnostic> {
        self.expect_kw("broadcast")?;

        let target = if self.is_kw("all") {
            self.bump();
            BroadcastTarget::All
        } else if self.is_kw("granted") {
            self.bump();
            BroadcastTarget::Granted
        } else if self.is_kw("to") {
            self.bump();
            let mut roles = vec![self.expect_ident()?];
            while self.is(TokenKind::Comma) {
                self.bump();
                roles.push(self.expect_ident()?);
            }
            BroadcastTarget::ToRoles(roles)
        } else {
            return Err(Diagnostic::new(
                "E1127",
                "expected broadcast target: all, granted, or to <roles>",
                Some(self.current_span()),
            ));
        };

        let debounce = if self.is_kw("debounce") {
            self.bump();
            Some(self.expect_int()?)
        } else {
            None
        };

        self.expect(TokenKind::Semi)?;

        Ok(BroadcastStmt { target, debounce })
    }

    fn parse_sort_direction(&mut self) -> Result<SortDirection, Diagnostic> {
        if self.is_kw("ascending") {
            self.bump();
            return Ok(SortDirection::Ascending);
        }
        if self.is_kw("descending") {
            self.bump();
            return Ok(SortDirection::Descending);
        }
        Err(Diagnostic::new(
            "E1128",
            "expected sort direction: ascending or descending",
            Some(self.current_span()),
        ))
    }

    fn parse_discover_mode(&mut self) -> Result<DiscoverMode, Diagnostic> {
        if self.is_kw("sync") {
            self.bump();
            return Ok(DiscoverMode::Sync);
        }
        if self.is_kw("grant") {
            self.bump();
            return Ok(DiscoverMode::Grant);
        }
        Err(Diagnostic::new(
            "E1129",
            "expected discover mode: sync or grant",
            Some(self.current_span()),
        ))
    }

    // ── Derive Declaration ───────────────────────────────────────────────

    fn parse_derive_decl(&mut self) -> Result<DeriveDecl, Diagnostic> {
        self.expect_kw("derive")?;
        let target = self.expect_ident()?;
        self.expect_kw("as")?;
        let kind = self.parse_layer_kind()?;
        self.expect(TokenKind::LBrace)?;

        let mut source = None;
        let mut cache = Vec::new();
        let mut using = None;

        while !self.is(TokenKind::RBrace) {
            if self.is_kw("from") {
                self.bump();
                source = Some(self.expect_ident()?);
                self.expect(TokenKind::Semi)?;
            } else if self.is_kw("cache") {
                self.bump();
                cache = self.parse_cache_spec()?;
                self.expect(TokenKind::Semi)?;
            } else if self.is_kw("using") {
                self.bump();
                using = Some(self.expect_string()?);
                self.expect(TokenKind::Semi)?;
            } else {
                return Err(Diagnostic::new(
                    "E1105",
                    "expected derive field: from, cache, or using",
                    Some(self.current_span()),
                ));
            }
        }

        self.expect(TokenKind::RBrace)?;

        Ok(DeriveDecl {
            target,
            kind,
            source: source.ok_or_else(|| {
                Diagnostic::new(
                    "E1106",
                    "derive missing required field 'from'",
                    Some(self.current_span()),
                )
            })?,
            cache,
            using,
        })
    }

    // ── Relay Declaration ────────────────────────────────────────────────

    fn parse_relay_decl(&mut self) -> Result<RelayDecl, Diagnostic> {
        self.expect_kw("relay")?;
        self.expect_kw("on")?;
        let layer = self.expect_ident()?;
        let event = self.parse_relay_event()?;
        self.expect_kw("using")?;
        let handler = self.expect_string()?;
        self.expect(TokenKind::Semi)?;

        Ok(RelayDecl {
            layer,
            event,
            handler,
        })
    }

    fn parse_relay_event(&mut self) -> Result<RelayEvent, Diagnostic> {
        if self.is_kw("insert") {
            self.bump();
            return Ok(RelayEvent::Insert);
        }
        if self.is_kw("update") {
            self.bump();
            return Ok(RelayEvent::Update);
        }
        if self.is_kw("delete") {
            self.bump();
            return Ok(RelayEvent::Delete);
        }
        Err(Diagnostic::new(
            "E1130",
            "expected relay event: insert, update, or delete",
            Some(self.current_span()),
        ))
    }

    // ── Validate Declaration ─────────────────────────────────────────────

    fn parse_validate_decl(&mut self) -> Result<ValidateDecl, Diagnostic> {
        self.expect_kw("validate")?;
        let layer = self.expect_ident()?;
        self.expect_kw("using")?;
        let handler = self.expect_string()?;
        self.expect(TokenKind::Semi)?;

        Ok(ValidateDecl { layer, handler })
    }

    // ── Permit Declaration ───────────────────────────────────────────────

    fn parse_permit_decl(&mut self) -> Result<PermitDecl, Diagnostic> {
        self.expect_kw("permit")?;
        let name = self.expect_ident()?;
        self.expect_kw("for")?;

        let mut roles = vec![self.expect_ident()?];
        while self.is(TokenKind::Comma) {
            self.bump();
            roles.push(self.expect_ident()?);
        }

        self.expect(TokenKind::LBrace)?;

        let mut statements = Vec::new();
        while !self.is(TokenKind::RBrace) {
            if self.is_kw("allow") {
                statements.push(PermitStmt::Allow(self.parse_permit_allow()?));
            } else if self.is_kw("issue") {
                statements.push(PermitStmt::Issue(self.parse_permit_issue()?));
            } else {
                return Err(Diagnostic::new(
                    "E1131",
                    "expected permit statement: allow or issue",
                    Some(self.current_span()),
                ));
            }
        }

        self.expect(TokenKind::RBrace)?;

        Ok(PermitDecl {
            name,
            roles,
            statements,
        })
    }

    /// Permit allow: `allow <actions> on <layer> [where <pred>];`
    /// Note: different from layer access — no roles, actions come first.
    fn parse_permit_allow(&mut self) -> Result<PermitAllow, Diagnostic> {
        self.expect_kw("allow")?;

        let mut actions = vec![self.parse_action()?];
        while self.is(TokenKind::Comma) {
            self.bump();
            actions.push(self.parse_action()?);
        }

        self.expect_kw("on")?;
        let layer = self.expect_ident()?;

        let predicate = if self.is_kw("where") {
            Some(self.parse_predicate_expr()?)
        } else {
            None
        };

        self.expect(TokenKind::Semi)?;

        Ok(PermitAllow {
            actions,
            layer,
            predicate,
        })
    }

    /// Permit issue: `issue for <roles>;`
    fn parse_permit_issue(&mut self) -> Result<PermitIssue, Diagnostic> {
        self.expect_kw("issue")?;
        self.expect_kw("for")?;

        let mut roles = vec![self.expect_ident()?];
        while self.is(TokenKind::Comma) {
            self.bump();
            roles.push(self.expect_ident()?);
        }

        self.expect(TokenKind::Semi)?;

        Ok(PermitIssue { roles })
    }

    // ── UI App Declaration ───────────────────────────────────────────────

    fn parse_ui_app_decl(&mut self) -> Result<UiAppDecl, Diagnostic> {
        self.expect_kw("ui_app")?;
        let name = self.expect_string()?;
        self.expect(TokenKind::LBrace)?;

        let mut allowed_roles = None;

        while !self.is(TokenKind::RBrace) {
            if self.is_kw("allow") {
                self.bump();
                let mut roles = vec![self.expect_ident()?];
                while self.is(TokenKind::Comma) {
                    self.bump();
                    roles.push(self.expect_ident()?);
                }
                allowed_roles = Some(roles);
                self.expect(TokenKind::Semi)?;
            } else {
                return Err(Diagnostic::new(
                    "E1107",
                    "expected ui_app field: allow",
                    Some(self.current_span()),
                ));
            }
        }

        self.expect(TokenKind::RBrace)?;

        Ok(UiAppDecl {
            name,
            allowed_roles: allowed_roles.ok_or_else(|| {
                Diagnostic::new(
                    "E1109",
                    "ui_app missing required field 'allow'",
                    Some(self.current_span()),
                )
            })?,
        })
    }

    // ── Shared Parsers ───────────────────────────────────────────────────

    fn parse_cache_spec(&mut self) -> Result<Vec<CachePolicy>, Diagnostic> {
        let mut out = vec![self.parse_cache_item()?];
        while self.is(TokenKind::Comma) {
            self.bump();
            out.push(self.parse_cache_item()?);
        }
        Ok(out)
    }

    fn parse_cache_item(&mut self) -> Result<CachePolicy, Diagnostic> {
        let span = self.current_span();
        if self.is_kw("ui_window") {
            self.bump();
            self.expect(TokenKind::LParen)?;
            let max_items = self.expect_int()?;
            self.expect(TokenKind::RParen)?;
            return Ok(CachePolicy::UiWindow { max_items });
        }
        if self.is_kw("memory_lru") {
            self.bump();
            self.expect(TokenKind::LParen)?;
            let max_layers = self.expect_int()?;
            self.expect(TokenKind::RParen)?;
            return Ok(CachePolicy::MemoryLru { max_layers });
        }
        if self.is_kw("sync_mode") {
            self.bump();
            self.expect(TokenKind::LParen)?;
            let mode = if self.is_kw("full_snapshot") {
                self.bump();
                SyncMode::FullSnapshot
            } else if self.is_kw("incremental") {
                self.bump();
                SyncMode::Incremental
            } else {
                return Err(Diagnostic::new(
                    "E1110",
                    "sync_mode requires full_snapshot or incremental",
                    Some(self.current_span()),
                ));
            };
            self.expect(TokenKind::RParen)?;
            return Ok(CachePolicy::SyncMode(mode));
        }
        if self.is_kw("retention_days") {
            self.bump();
            self.expect(TokenKind::LParen)?;
            let days = self.expect_int()?;
            self.expect(TokenKind::RParen)?;
            return Ok(CachePolicy::RetentionDays(days));
        }

        Err(Diagnostic::new("E1111", "unknown cache policy", Some(span)))
    }

    fn parse_capability(&mut self) -> Result<PeerCapability, Diagnostic> {
        if self.is_kw("share") {
            self.bump();
            return Ok(PeerCapability::Share);
        }
        if self.is_kw("delegate") {
            self.bump();
            return Ok(PeerCapability::Delegate);
        }
        if self.is_kw("relay") {
            self.bump();
            return Ok(PeerCapability::Relay);
        }
        if self.is_kw("accept_publish") {
            self.bump();
            return Ok(PeerCapability::AcceptPublish);
        }
        if self.is_kw("manage_access") {
            self.bump();
            return Ok(PeerCapability::ManageAccess);
        }
        Err(Diagnostic::new(
            "E1112",
            "unknown role capability",
            Some(self.current_span()),
        ))
    }

    fn parse_layer_kind(&mut self) -> Result<LayerKind, Diagnostic> {
        if self.is_kw("map") {
            self.bump();
            return Ok(LayerKind::Map);
        }
        if self.is_kw("list") {
            self.bump();
            return Ok(LayerKind::List);
        }
        if self.is_kw("text") {
            self.bump();
            return Ok(LayerKind::Text);
        }
        if self.is_kw("counter") {
            self.bump();
            return Ok(LayerKind::Counter);
        }
        if self.is_kw("blob") {
            self.bump();
            return Ok(LayerKind::Blob);
        }
        Err(Diagnostic::new(
            "E1113",
            "unknown layer kind",
            Some(self.current_span()),
        ))
    }

    fn parse_namespace(&mut self) -> Result<LayerNamespace, Diagnostic> {
        if self.is_kw("shared") {
            self.bump();
            return Ok(LayerNamespace::Shared);
        }
        if self.is_kw("page") {
            self.bump();
            return Ok(LayerNamespace::Page);
        }
        Err(Diagnostic::new(
            "E1114",
            "namespace must be shared or page",
            Some(self.current_span()),
        ))
    }

    fn parse_grant_mode(&mut self) -> Result<GrantMode, Diagnostic> {
        if self.is_kw("open") {
            self.bump();
            return Ok(GrantMode::Open);
        }
        if self.is_kw("explicit") {
            self.bump();
            return Ok(GrantMode::Explicit);
        }
        if self.is_kw("role_scoped") {
            self.bump();
            return Ok(GrantMode::RoleScoped);
        }
        Err(Diagnostic::new(
            "E1115",
            "unknown grant mode",
            Some(self.current_span()),
        ))
    }

    fn parse_resolution(&mut self) -> Result<Resolution, Diagnostic> {
        if self.is_kw("minute") {
            self.bump();
            return Ok(Resolution::Minute);
        }
        if self.is_kw("hour") {
            self.bump();
            return Ok(Resolution::Hour);
        }
        if self.is_kw("day") {
            self.bump();
            return Ok(Resolution::Day);
        }
        if self.is_kw("week") {
            self.bump();
            return Ok(Resolution::Week);
        }
        if self.is_kw("month") {
            self.bump();
            return Ok(Resolution::Month);
        }
        Err(Diagnostic::new(
            "E1116",
            "unknown shard resolution",
            Some(self.current_span()),
        ))
    }

    fn parse_action(&mut self) -> Result<Action, Diagnostic> {
        if self.is_kw("create") {
            self.bump();
            return Ok(Action::Create);
        }
        if self.is_kw("read") {
            self.bump();
            return Ok(Action::Read);
        }
        if self.is_kw("write") {
            self.bump();
            return Ok(Action::Write);
        }
        if self.is_kw("sync") {
            self.bump();
            return Ok(Action::Sync);
        }
        if self.is_kw("grant") {
            self.bump();
            return Ok(Action::Grant);
        }
        if self.is_kw("revoke") {
            self.bump();
            return Ok(Action::Revoke);
        }
        Err(Diagnostic::new(
            "E1117",
            "unknown action",
            Some(self.current_span()),
        ))
    }

    // ── Token Utilities ──────────────────────────────────────────────────

    fn expect_kw(&mut self, kw: &'static str) -> Result<(), Diagnostic> {
        if self.is_kw(kw) {
            self.bump();
            Ok(())
        } else {
            Err(Diagnostic::new(
                "E1190",
                format!("expected keyword '{}'", kw),
                Some(self.current_span()),
            ))
        }
    }

    /// Accept identifiers AND keywords as names.
    ///
    /// v2 adds many keywords (node, mode, peer, etc.) that are also
    /// valid role/layer/field names. The parser dispatches on specific
    /// keywords first, so accepting keywords here is unambiguous.
    fn expect_ident(&mut self) -> Result<String, Diagnostic> {
        let span = self.current_span();
        match &self.current().kind {
            TokenKind::Ident(v) => {
                let out = v.clone();
                self.bump();
                Ok(out)
            }
            TokenKind::Keyword(v) => {
                let out = v.clone();
                self.bump();
                Ok(out)
            }
            _ => Err(Diagnostic::new("E1191", "expected identifier", Some(span))),
        }
    }

    fn expect_string(&mut self) -> Result<String, Diagnostic> {
        let span = self.current_span();
        match &self.current().kind {
            TokenKind::StringLit(v) => {
                let out = v.clone();
                self.bump();
                Ok(out)
            }
            _ => Err(Diagnostic::new(
                "E1192",
                "expected string literal",
                Some(span),
            )),
        }
    }

    fn expect_int(&mut self) -> Result<u32, Diagnostic> {
        let span = self.current_span();
        match self.current().kind {
            TokenKind::IntLit(v) => {
                self.bump();
                Ok(v)
            }
            _ => Err(Diagnostic::new(
                "E1193",
                "expected integer literal",
                Some(span),
            )),
        }
    }

    fn expect(&mut self, kind: TokenKind) -> Result<(), Diagnostic> {
        if self.is(kind.clone()) {
            self.bump();
            Ok(())
        } else {
            Err(Diagnostic::new(
                "E1194",
                format!("expected token {:?}", kind),
                Some(self.current_span()),
            ))
        }
    }

    fn is_kw(&self, kw: &str) -> bool {
        matches!(&self.current().kind, TokenKind::Keyword(v) if v == kw)
    }

    fn is(&self, kind: TokenKind) -> bool {
        self.current().kind == kind
    }

    fn current(&self) -> &Token {
        &self.tokens[self.idx]
    }

    fn current_span(&self) -> Span {
        self.current().span
    }

    fn bump(&mut self) {
        if self.idx + 1 < self.tokens.len() {
            self.idx += 1;
        }
    }
}
