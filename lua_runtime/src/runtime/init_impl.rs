use super::*;

fn set_global<T: mlua::IntoLua>(lua: &Lua, name: &str, value: T) -> Result<(), String> {
    lua.globals()
        .set(name, value)
        .map_err(|e| format!("Failed to set {} global: {}", name, e))
}

fn load_module(
    lua: &Lua,
    global_name: &str,
    source: &str,
    module_name: &str,
) -> Result<(), String> {
    let module: Value = lua
        .load(source)
        .eval()
        .map_err(|e| format!("Failed to load {} module: {}", module_name, e))?;
    set_global(lua, global_name, module)
}

impl LuaRuntime {
    /// Create runtime (internal, called on the OS thread)
    pub(super) fn new_internal(
        config: LuaRuntimeConfig,
        cmd_rx: mpsc::Receiver<LuaCommand>,
    ) -> Result<Self, String> {
        let lua = Lua::new();
        let scheduler = Arc::new(Mutex::new(Scheduler::new(config.clock.clone())));
        let ui_shared = Arc::new(Mutex::new(UiSharedState::new()));

        let timers_table: Table = lua
            .create_table()
            .map_err(|e| format!("Failed to create _timers table: {}", e))?;
        set_global(&lua, "_timers", timers_table)?;

        let timer_table = super::register_timer_functions(&lua, scheduler.clone())
            .map_err(|e| format!("Failed to create timer table: {}", e))?;
        set_global(&lua, "timer", timer_table)?;

        let our_name = Some(config.user_name.clone());
        let scribe = if config.ui_enabled {
            if let Some(ref ui_tx) = config.ui_tx {
                ScribeBindings::with_ui(
                    config.scribe.clone(),
                    config.page_id.clone(),
                    config.user_did.clone(),
                    our_name,
                    ui_tx.clone(),
                )
            } else {
                ScribeBindings::new(
                    config.scribe.clone(),
                    config.page_id.clone(),
                    config.user_did.clone(),
                    our_name,
                )
            }
        } else {
            ScribeBindings::new(
                config.scribe.clone(),
                config.page_id.clone(),
                config.user_did.clone(),
                our_name,
            )
        };
        let binding_manager = scribe.binding_manager();
        set_global(&lua, "scribe", scribe)?;

        let derivation = DerivationBindings::new(config.scribe.clone());
        set_global(&lua, "derivation", derivation)?;

        let permit = PermitBindings::new(
            config.page_id.clone(),
            config.user_did.clone(),
            config.user_name.clone(),
            config.user_role.clone(),
        );
        set_global(&lua, "permit", permit)?;

        let peers = PeersBindings::new(config.scribe.clone());
        set_global(&lua, "peers", peers)?;

        let page = PageBindings {
            navigate_tx: config.navigate_tx,
        };
        set_global(&lua, "page", page)?;

        let clock = ClockBindings::new(config.clock.clone());
        set_global(&lua, "clock", clock)?;

        if config.ui_enabled {
            if let (Some(_ui_tx), Some(query_tx)) = (config.ui_tx.clone(), config.query_tx.clone())
            {
                let ui = UiBindings {
                    app_id: config.page_id.clone(),
                    query_tx,
                    shared: ui_shared.clone(),
                };
                set_global(&lua, "ui", ui)?;

                let layout = LayoutBindings::new();
                set_global(&lua, "layout", layout)?;

                let emoji = EmojiBindings;
                set_global(&lua, "emoji", emoji)?;
            } else {
                return Err("ui_enabled=true requires ui_tx and query_tx".into());
            }
        } else {
            let ui = super::BufferedUiBindings::new();
            set_global(&lua, "ui", ui)?;
        }

        load_module(&lua, "api", LUA_API_MODULE, "api")?;
        load_module(&lua, "datetime", LUA_DATE_MODULE, "datetime")?;
        load_module(&lua, "presence_lib", LUA_PRESENCE_MODULE, "presence")?;
        load_module(&lua, "binding", LUA_BINDING_MODULE, "binding")?;

        lua.load(&config.lua_code)
            .exec()
            .map_err(|e| format!("Failed to load Lua code: {}", e))?;

        info!(
            page_id = %config.page_id,
            app_name = %config.app_name,
            ui_enabled = config.ui_enabled,
            "LuaRuntime created"
        );

        let handler_cache = HandlerCache::populate(&lua);

        Ok(Self {
            lua,
            page_id: config.page_id,
            scheduler,
            cmd_rx,
            ui_tx: config.ui_tx,
            ui_shared,
            binding_manager,
            scribe: config.scribe,
            ui_enabled: config.ui_enabled,
            handler_cache,
            clock: config.clock,
        })
    }
}
