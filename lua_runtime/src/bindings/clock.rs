//! Clock bindings for Lua
//!
//! Provides time-based functions for period-based layer naming and time queries.

use std::sync::Arc;

use domains::{format_period, ClockSource};
use mlua::{UserData, UserDataMethods};

/// Clock bindings for Lua
pub struct ClockBindings {
    clock: Arc<dyn ClockSource>,
}

impl ClockBindings {
    pub fn new(clock: Arc<dyn ClockSource>) -> Self {
        Self { clock }
    }
}

impl UserData for ClockBindings {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // clock.count([offset_seconds]) -> unix_timestamp
        methods.add_method("count", |_lua, this, offset: Option<i64>| {
            let unix = this.clock.now_unix();
            let result = unix + offset.unwrap_or(0);
            Ok(result)
        });

        // clock.minute([offset]) -> "2026-02-20T14:35"
        methods.add_method("minute", |_lua, this, offset: Option<i32>| {
            let unix = this.clock.now_unix();
            format_period("minute", unix, offset.unwrap_or(0)).map_err(mlua::Error::external)
        });

        // clock.hour([offset]) -> "2026-02-20T14"
        methods.add_method("hour", |_lua, this, offset: Option<i32>| {
            let unix = this.clock.now_unix();
            format_period("hour", unix, offset.unwrap_or(0)).map_err(mlua::Error::external)
        });

        // clock.day([offset]) -> "2026-02-20"
        methods.add_method("day", |_lua, this, offset: Option<i32>| {
            let unix = this.clock.now_unix();
            format_period("day", unix, offset.unwrap_or(0)).map_err(mlua::Error::external)
        });

        // clock.week([offset]) -> "2026-W08"
        methods.add_method("week", |_lua, this, offset: Option<i32>| {
            let unix = this.clock.now_unix();
            format_period("week", unix, offset.unwrap_or(0)).map_err(mlua::Error::external)
        });

        // clock.month([offset]) -> "2026-02"
        methods.add_method("month", |_lua, this, offset: Option<i32>| {
            let unix = this.clock.now_unix();
            format_period("month", unix, offset.unwrap_or(0)).map_err(mlua::Error::external)
        });
    }
}
