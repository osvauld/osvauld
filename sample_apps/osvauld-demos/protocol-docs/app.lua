-- Osvauld Protocol Documentation Browser
-- Read-only docs viewer — no scribe layers needed

-- Navigation structure: sections and their subsections
local NAV = {
    { name = "Overview",       subs = { "Architecture", "Crate Map" } },
    { name = "Protocol",       subs = { "Wire Protocol", "Message Types", "Handshake", "Sync Protocol" } },
    { name = "Data Model",     subs = { "Hierarchy", "Loro CRDTs", "Encryption", "Derivation" } },
    { name = "Building Apps",  subs = { "Getting Started", "Lua API", "Scribe API", "Permits", "Renderers", "Validation" } },
    { name = "AI Development", subs = { "api.export", "Control Server", "Python Client", "Debugging" } },
    { name = "Testing",        subs = { "Script Automation", "Integration Tests", "Performance" } },
    { name = "Deployment",     subs = { "Running Nodes", "Relay Setup", "IoT & Future" } },
}

local section = 0
local subsection = 0

-- Compute and set prev/next labels for the current position
local function update_nav_labels()
    local prev_label = ""
    local next_label = ""

    -- Previous: go to prior subsection, or last subsection of prior section
    if subsection > 0 then
        prev_label = NAV[section + 1].subs[subsection] -- current sub - 1 (Lua 1-indexed)
    elseif section > 0 then
        local prev_sec = NAV[section] -- prior section (Lua 1-indexed)
        prev_label = prev_sec.subs[#prev_sec.subs]
    end

    -- Next: go to next subsection, or first subsection of next section
    local subs = NAV[section + 1].subs
    if subsection < #subs - 1 then
        next_label = subs[subsection + 2] -- current sub + 1 (Lua 1-indexed)
    elseif section < #NAV - 1 then
        next_label = NAV[section + 2].subs[1]
    end

    ui:set("prev_label", prev_label)
    ui:set("next_label", next_label)
end

local function navigate_to(sec, sub)
    section = sec
    subsection = sub
    ui:set("active_section", section)
    ui:set("active_subsection", subsection)
    update_nav_labels()
end

function on_init()
    navigate_to(0, 0)
end

function on_click(target)
    -- Section navigation: "section:N"
    if target:match("^section:") then
        local sec = tonumber(target:sub(9))
        if sec then
            navigate_to(sec, 0)
        end
        return
    end

    -- Subsection navigation: "sub:N"
    if target:match("^sub:") then
        local sub = tonumber(target:sub(5))
        if sub then
            navigate_to(section, sub)
        end
        return
    end

    -- Nav prev
    if target == "nav:prev" then
        if subsection > 0 then
            navigate_to(section, subsection - 1)
        elseif section > 0 then
            local prev_subs = NAV[section] -- prior section (Lua 1-indexed)
            navigate_to(section - 1, #prev_subs.subs - 1)
        end
        return
    end

    -- Nav next
    if target == "nav:next" then
        local subs = NAV[section + 1].subs
        if subsection < #subs - 1 then
            navigate_to(section, subsection + 1)
        elseif section < #NAV - 1 then
            navigate_to(section + 1, 0)
        end
        return
    end
end
