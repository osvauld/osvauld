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
    fn parse_file(&mut self) -> Result<AppSpec, Diagnostic> {
        self.expect_kw("app")?;
        let name = self.expect_string()?;
        self.expect_kw("version")?;
        let version = self.expect_string()?;
        self.expect(TokenKind::LBrace)?;

        let mut roles = Vec::new();
        let mut layers = Vec::new();
        let mut derives = Vec::new();
        let mut ui_apps = Vec::new();

        while !self.is(TokenKind::RBrace) {
            if self.is_kw("role") {
                roles.push(self.parse_role_decl()?);
            } else if self.is_kw("layer") {
                layers.push(self.parse_layer_decl()?);
            } else if self.is_kw("derive") {
                derives.push(self.parse_derive_decl()?);
            } else if self.is_kw("ui_app") {
                ui_apps.push(self.parse_ui_app_decl()?);
            } else {
                return Err(Diagnostic::new(
                    "E1101",
                    "expected top-level declaration: role, layer, derive, or ui_app",
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
            layers,
            derives,
            ui_apps,
        })
    }

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

    fn parse_layer_decl(&mut self) -> Result<LayerDecl, Diagnostic> {
        self.expect_kw("layer")?;
        let name = self.expect_ident()?;
        self.expect_kw("as")?;
        let kind = self.parse_layer_kind()?;
        self.expect(TokenKind::LBrace)?;

        let mut path = None;
        let mut namespace = None;
        let mut grant = GrantMode::Open;
        let mut time = TimeModel::Unsharded;
        let mut cache = Vec::new();
        let mut access = Vec::new();

        while !self.is(TokenKind::RBrace) {
            if self.is_kw("path") {
                self.bump();
                path = Some(self.expect_string()?);
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
            } else {
                return Err(Diagnostic::new(
                    "E1102",
                    "expected layer field: path, namespace, grant, shard, cache, or allow",
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
            path,
            namespace,
            grant,
            time,
            cache,
            access,
        })
    }

    fn parse_access_decl(&mut self) -> Result<AccessRule, Diagnostic> {
        self.expect_kw("allow")?;

        let mut roles = vec![self.expect_ident()?];
        while self.is(TokenKind::Comma) {
            self.bump();
            roles.push(self.expect_ident()?);
        }

        self.expect_kw("to")?;
        let mut actions = vec![self.parse_action()?];
        while self.is(TokenKind::Comma) {
            self.bump();
            actions.push(self.parse_action()?);
        }

        let scope = if self.is_kw("on") {
            self.bump();
            if self.is(TokenKind::Star) {
                self.bump();
                ScopeExpr::All
            } else {
                ScopeExpr::LayerRef(self.expect_ident()?)
            }
        } else {
            ScopeExpr::All
        };

        self.expect(TokenKind::Semi)?;
        Ok(AccessRule {
            roles,
            actions,
            scope,
        })
    }

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

    fn parse_ui_app_decl(&mut self) -> Result<UiAppDecl, Diagnostic> {
        self.expect_kw("ui_app")?;
        let name = self.expect_string()?;
        self.expect(TokenKind::LBrace)?;

        let mut entry = None;
        let mut allowed_roles = None;

        while !self.is(TokenKind::RBrace) {
            if self.is_kw("entry") {
                self.bump();
                entry = Some(self.expect_string()?);
                self.expect(TokenKind::Semi)?;
            } else if self.is_kw("allow") {
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
                    "expected ui_app field: entry or allow",
                    Some(self.current_span()),
                ));
            }
        }

        self.expect(TokenKind::RBrace)?;

        Ok(UiAppDecl {
            name,
            entry: entry.ok_or_else(|| {
                Diagnostic::new(
                    "E1108",
                    "ui_app missing required field 'entry'",
                    Some(self.current_span()),
                )
            })?,
            allowed_roles: allowed_roles.ok_or_else(|| {
                Diagnostic::new(
                    "E1109",
                    "ui_app missing required field 'allow'",
                    Some(self.current_span()),
                )
            })?,
        })
    }

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
        if self.is_kw("creator") {
            self.bump();
            return Ok(LayerNamespace::Creator);
        }
        Err(Diagnostic::new(
            "E1114",
            "namespace must be shared or creator",
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

    fn expect_ident(&mut self) -> Result<String, Diagnostic> {
        let span = self.current_span();
        match &self.current().kind {
            TokenKind::Ident(v) => {
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

    fn is_kw(&self, kw: &'static str) -> bool {
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
