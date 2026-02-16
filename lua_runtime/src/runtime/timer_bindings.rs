use std::sync::Arc;

use mlua::{Function, Lua, Table, Value};
use parking_lot::Mutex;
use tracing::trace;

use crate::scheduler::Scheduler;

/// Register timer functions on a Lua table.
///
/// Creates timer.setTimeout, timer.setInterval, timer.clear as regular functions
/// that can be called with dot syntax (timer.setInterval(ms, callback)).
pub(super) fn register_timer_functions(
    lua: &Lua,
    scheduler: Arc<Mutex<Scheduler>>,
) -> mlua::Result<Table> {
    let timer_table = lua.create_table()?;

    // timer.setTimeout(ms, callback) -> id
    let sched = scheduler.clone();
    timer_table.set(
        "setTimeout",
        lua.create_function(move |lua, (ms, callback): (u64, Function)| {
            let id = {
                let mut scheduler = sched.lock();
                scheduler.register_timer(ms, false)
            };
            let timers: Table = lua.globals().get("_timers")?;
            timers.set(id, callback)?;
            trace!(timer_id = id, ms, "setTimeout registered");
            Ok(id)
        })?,
    )?;

    // timer.setInterval(ms, callback) -> id
    let sched = scheduler.clone();
    timer_table.set(
        "setInterval",
        lua.create_function(move |lua, (ms, callback): (u64, Function)| {
            let id = {
                let mut scheduler = sched.lock();
                scheduler.register_timer(ms, true)
            };
            let timers: Table = lua.globals().get("_timers")?;
            timers.set(id, callback)?;
            trace!(timer_id = id, ms, "setInterval registered");
            Ok(id)
        })?,
    )?;

    // timer.clear(id)
    let sched = scheduler;
    timer_table.set(
        "clear",
        lua.create_function(move |lua, id: u64| {
            {
                let mut scheduler = sched.lock();
                scheduler.clear_timer(id);
            }
            let timers: Table = lua.globals().get("_timers")?;
            timers.set(id, Value::Nil)?;
            trace!(timer_id = id, "Timer cleared");
            Ok(())
        })?,
    )?;

    Ok(timer_table)
}
