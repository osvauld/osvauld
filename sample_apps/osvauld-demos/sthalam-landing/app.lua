-- Sthalam Landing Page
-- Read-only showcase — no scribe layers needed

function on_init()
    -- Landing page is static, nothing to initialize
end

function on_click(target)
    if target == "cta:Read the Docs" then
        page:open_app("Protocol Docs")
    end
end
