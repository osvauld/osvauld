-- Math Simulation App Logic
-- Particle physics with configurable parameters

-- Canvas settings
local CANVAS_WIDTH = 600
local CANVAS_HEIGHT = 400

-- Simulation state
local sim = {
    particles = {},
    running = false,
    gravity = 0.5,
    bounce = 0.8,
    friction = 0.99,
    particle_count = 100,

    -- FPS tracking
    fps = 0,
    frame_count = 0,
    last_fps_update = 0,
}

-- Loro layers
local page_id = nil
local config_layer = nil

-- Initialize the app
function on_init()
    page_id = permit:page_id()

    -- Config layer (synced)
    config_layer = loro:get_or_create_layer(page_id .. "/sim_config", "map")

    -- Load saved config
    local saved = config_layer:get("settings")
    if saved then
        sim.gravity = saved.gravity or 0.5
        sim.bounce = saved.bounce or 0.8
        sim.particle_count = saved.particle_count or 100
    end

    -- Set canvas size
    ui:set("canvas_width", CANVAS_WIDTH)
    ui:set("canvas_height", CANVAS_HEIGHT)

    -- Initialize particles
    reset_particles()

    -- Update UI
    sync_config_ui()
end

-- Create particles
function reset_particles()
    sim.particles = {}

    for i = 1, sim.particle_count do
        create_particle(
            math.random() * CANVAS_WIDTH,
            math.random() * CANVAS_HEIGHT * 0.5,  -- Start in top half
            (math.random() - 0.5) * 4,  -- Random vx
            (math.random() - 0.5) * 4   -- Random vy
        )
    end

    sync_particles_ui()
end

-- Create a single particle
function create_particle(x, y, vx, vy)
    local particle = {
        x = x,
        y = y,
        vx = vx or 0,
        vy = vy or 0,
        radius = 4 + math.random() * 4,
        color_index = math.random(0, 7),
    }
    table.insert(sim.particles, particle)
    return particle
end

-- Physics tick
function tick()
    -- FPS calculation
    sim.frame_count = sim.frame_count + 1
    local now = os.clock()
    if now - sim.last_fps_update >= 1.0 then
        sim.fps = sim.frame_count
        sim.frame_count = 0
        sim.last_fps_update = now
        ui:set("fps", sim.fps)
    end

    if not sim.running then
        return
    end

    -- Update each particle
    for _, p in ipairs(sim.particles) do
        -- Apply gravity
        p.vy = p.vy + sim.gravity

        -- Apply friction
        p.vx = p.vx * sim.friction
        p.vy = p.vy * sim.friction

        -- Update position
        p.x = p.x + p.vx
        p.y = p.y + p.vy

        -- Bounce off walls
        if p.x - p.radius < 0 then
            p.x = p.radius
            p.vx = -p.vx * sim.bounce
        elseif p.x + p.radius > CANVAS_WIDTH then
            p.x = CANVAS_WIDTH - p.radius
            p.vx = -p.vx * sim.bounce
        end

        -- Bounce off floor/ceiling
        if p.y - p.radius < 0 then
            p.y = p.radius
            p.vy = -p.vy * sim.bounce
        elseif p.y + p.radius > CANVAS_HEIGHT then
            p.y = CANVAS_HEIGHT - p.radius
            p.vy = -p.vy * sim.bounce
        end
    end

    sync_particles_ui()
end

-- Explode particles outward from center
function explode()
    local cx = CANVAS_WIDTH / 2
    local cy = CANVAS_HEIGHT / 2

    for _, p in ipairs(sim.particles) do
        local dx = p.x - cx
        local dy = p.y - cy
        local dist = math.sqrt(dx * dx + dy * dy)
        if dist > 0 then
            local force = 20
            p.vx = (dx / dist) * force
            p.vy = (dy / dist) * force
        end
    end
end

-- Sync particles to UI
function sync_particles_ui()
    local ui_particles = {}
    for _, p in ipairs(sim.particles) do
        table.insert(ui_particles, {
            x = p.x,
            y = p.y,
            radius = p.radius,
            color_index = p.color_index,
        })
    end
    ui:set("particles", ui_particles)
end

-- Sync config to UI
function sync_config_ui()
    ui:set("running", sim.running)
    ui:set("gravity", sim.gravity)
    ui:set("bounce", sim.bounce)
    ui:set("friction", sim.friction)
    ui:set("particle_count", sim.particle_count)
end

-- Save config to Loro
function save_config()
    config_layer:set("settings", {
        gravity = sim.gravity,
        bounce = sim.bounce,
        particle_count = sim.particle_count,
    })
end

-- Handle click events
function on_click(target)
    if target == "toggle" then
        sim.running = not sim.running
        ui:set("running", sim.running)

    elseif target == "reset" then
        reset_particles()

    elseif target == "explode" then
        explode()

    elseif target:match("^particles:") then
        local delta = tonumber(target:sub(11))
        if delta then
            sim.particle_count = math.max(10, math.min(500, sim.particle_count + delta))
            ui:set("particle_count", sim.particle_count)
            reset_particles()
            save_config()
        end

    elseif target:match("^gravity:") then
        local delta = tonumber(target:sub(9))
        if delta then
            sim.gravity = math.max(0, math.min(2, sim.gravity + delta))
            ui:set("gravity", sim.gravity)
            save_config()
        end

    elseif target:match("^bounce:") then
        local delta = tonumber(target:sub(8))
        if delta then
            sim.bounce = math.max(0, math.min(1, sim.bounce + delta))
            ui:set("bounce", sim.bounce)
            save_config()
        end

    elseif target:match("^preset:") then
        local preset = target:sub(8)
        apply_preset(preset)
    end
end

-- Apply preset configurations
function apply_preset(preset)
    if preset == "rain" then
        sim.gravity = 0.8
        sim.bounce = 0.3
        sim.particle_count = 200

    elseif preset == "bounce" then
        sim.gravity = 0.5
        sim.bounce = 0.95
        sim.particle_count = 50

    elseif preset == "float" then
        sim.gravity = 0.1
        sim.bounce = 0.9
        sim.particle_count = 100
    end

    sync_config_ui()
    reset_particles()
    save_config()
end

-- Handle Loro changes
function on_loro_change(layer_name, change_type)
    if layer_name:match("/sim_config$") then
        local saved = config_layer:get("settings")
        if saved then
            sim.gravity = saved.gravity or 0.5
            sim.bounce = saved.bounce or 0.8
            -- Don't auto-reset particles on remote config change
            sync_config_ui()
        end
    end
end

-- API exports for testing
api.export("get_particle_count", function() return #sim.particles end)
api.export("get_fps", function() return sim.fps end)
api.export("set_running", function(val) sim.running = val end)
