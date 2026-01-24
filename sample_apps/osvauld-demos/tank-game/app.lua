-- Tank Brick Game App Logic
-- Classic tank battle with enemies, bosses, and synced high scores

-- Grid settings (Slint handles cell_size responsively)
local GRID_WIDTH = 36
local GRID_HEIGHT = 28

-- Timing (frames at ~60fps)
local PLAYER_MOVE_FRAMES = 8     -- Player moves every N frames
local ENEMY_MOVE_FRAMES = 12     -- Base enemy speed
local BULLET_MOVE_FRAMES = 2     -- Bullets move fast
local SHOOT_COOLDOWN_FRAMES = 15 -- Min frames between player shots
local ENEMY_SHOOT_CHANCE = 2     -- % chance enemy shoots when facing player

-- Game state
-- Note: Enemies and stage progression are managed by the node
local game = {
    tank = { x = 10, y = 13, direction = "up", alive = true },
    enemies = {},           -- id -> { x, y, direction, hp, is_boss } (received from node)
    bullets = {},           -- id -> { x, y, direction, owner }
    obstacles = {},         -- Array of { x, y, type } (loaded from CRDT)
    stage = 1,
    score = 0,
    high_score = 0,
    lives = 3,
    game_over = false,
    paused = true,
    frame_count = 0,
    bullet_move_counter = 0,
    shoot_cooldown = 0,
    next_bullet_id = 1,
    fps_frames = 0,
    fps_last_time = 0,
    fps = 0,
}

-- Loro layers
local page_id = nil
local scores_layer = nil
local obstacles_layer = nil  -- LoroMap with "x_y" keys for shared obstacles

-- Multiplayer state
local my_did = nil
local my_name = nil
local remote_players = {}  -- did -> { x, y, direction, alive, color, name, last_seen }
local last_position_broadcast = 0

-- Player colors for multiplayer (assigned based on DID hash)
local PLAYER_COLORS = {"#0f380f", "#0000ff", "#ff00ff", "#00ffff", "#ff8800", "#00ff00"}

-- Direction vectors
local DIR_VECTORS = {
    up = { dx = 0, dy = -1 },
    down = { dx = 0, dy = 1 },
    left = { dx = -1, dy = 0 },
    right = { dx = 1, dy = 0 },
}

-- Opposite directions
local OPPOSITE = {
    up = "down",
    down = "up",
    left = "right",
    right = "left",
}

-- Assign consistent color index based on DID (like canvas app)
function color_index_for_user(did)
    local hash = 0
    for i = 1, #did do
        hash = (hash * 31 + string.byte(did, i)) % #PLAYER_COLORS
    end
    return hash  -- 0-indexed for Slint
end

-- Initialize the app
function on_init()
    page_id = permit:page_id()
    my_did = permit:my_did()
    my_name = permit:my_name() or my_did:sub(-8)

    -- Set up Loro layers
    scores_layer = loro:get_or_create_layer(page_id .. "/scores", "list")
    obstacles_layer = loro:get_or_create_layer(page_id .. "/obstacles", "map")

    -- Set grid settings for UI (cell_size is computed by Slint from container size)
    ui:set("grid_width", GRID_WIDTH)
    ui:set("grid_height", GRID_HEIGHT)

    -- Initialize client state (node manages enemies/obstacles)
    init_client()
    refresh_leaderboard()
end

-- Initialize client state (called on first load)
function init_client()
    -- Reset player tank to spawn position
    game.tank = {
        x = math.floor(GRID_WIDTH / 2),
        y = GRID_HEIGHT - 2,
        direction = "up",
        alive = true,
    }

    game.enemies = {}  -- Will be populated by node broadcasts
    game.bullets = {}
    game.stage = 1
    game.score = 0
    game.lives = 3
    game.game_over = false
    game.paused = true
    game.frame_count = 0
    game.bullet_move_counter = 0
    game.shoot_cooldown = 0
    game.next_bullet_id = 1

    -- Load obstacles from CRDT (node generates them)
    load_obstacles_from_layer()

    sync_game_ui()
end

-- Request game restart (sends to node)
function request_restart()
    local payload = '{"type":"restart_request"}'
    butler:send_ephemeral(payload)

    -- Reset local client state
    init_client()
end

-- Load obstacles from CRDT layer into local game state
function load_obstacles_from_layer()
    game.obstacles = {}
    for _, key in ipairs(obstacles_layer:keys()) do
        local obs = obstacles_layer:get(key)
        if obs then
            table.insert(game.obstacles, obs)
        end
    end
end

-- Destroy a brick at position (remove from CRDT)
function destroy_brick(x, y)
    local key = x .. "_" .. y
    obstacles_layer:set(key, nil)
end

-- Check if a cell is blocked (for player movement)
function is_blocked(x, y)
    -- Boundary check
    if x < 0 or x >= GRID_WIDTH or y < 0 or y >= GRID_HEIGHT then
        return true
    end

    -- Obstacle check
    for _, obs in ipairs(game.obstacles) do
        if obs.x == x and obs.y == y then
            return true
        end
    end

    -- Enemy check
    for _, enemy in pairs(game.enemies) do
        if enemy.x == x and enemy.y == y then
            return true
        end
    end

    return false
end

-- Fire a bullet
function fire_bullet(owner, x, y, direction)
    local vec = DIR_VECTORS[direction]
    local bullet = {
        x = x + vec.dx,
        y = y + vec.dy,
        direction = direction,
        owner = owner, -- "player" or enemy id
    }

    -- Only create if starting position is valid
    if bullet.x >= 0 and bullet.x < GRID_WIDTH and bullet.y >= 0 and bullet.y < GRID_HEIGHT then
        game.bullets[game.next_bullet_id] = bullet
        game.next_bullet_id = game.next_bullet_id + 1
    end
end

-- Move all bullets
function move_bullets()
    local bullets_to_remove = {}

    for id, bullet in pairs(game.bullets) do
        local vec = DIR_VECTORS[bullet.direction]
        bullet.x = bullet.x + vec.dx
        bullet.y = bullet.y + vec.dy

        -- Check boundary
        if bullet.x < 0 or bullet.x >= GRID_WIDTH or bullet.y < 0 or bullet.y >= GRID_HEIGHT then
            table.insert(bullets_to_remove, id)
        else
            -- Check obstacle collision
            local hit_obstacle = false
            for i, obs in ipairs(game.obstacles) do
                if obs.x == bullet.x and obs.y == bullet.y then
                    hit_obstacle = true
                    if obs.kind == "brick" then
                        -- Remove brick from CRDT layer (syncs to all players)
                        destroy_brick(obs.x, obs.y)
                        game.score = game.score + 1
                    end
                    break
                end
            end

            if hit_obstacle then
                table.insert(bullets_to_remove, id)
            else
                -- Check player hit (by enemy bullets)
                if bullet.owner ~= "player" and game.tank.alive then
                    if bullet.x == game.tank.x and bullet.y == game.tank.y then
                        player_hit()
                        table.insert(bullets_to_remove, id)
                    end
                end

                -- Check enemy hit (by player bullets)
                -- Node handles enemy damage via ephemeral, but we remove bullet locally
                if bullet.owner == "player" then
                    for enemy_id, enemy in pairs(game.enemies) do
                        if bullet.x == enemy.x and bullet.y == enemy.y then
                            table.insert(bullets_to_remove, id)
                            -- Score is updated when node confirms enemy destroyed
                            break
                        end
                    end
                end

                -- Check bullet collision with other bullets
                for other_id, other in pairs(game.bullets) do
                    if id ~= other_id and bullet.x == other.x and bullet.y == other.y then
                        table.insert(bullets_to_remove, id)
                        table.insert(bullets_to_remove, other_id)
                        break
                    end
                end
            end
        end
    end

    -- Remove destroyed bullets
    for _, id in ipairs(bullets_to_remove) do
        game.bullets[id] = nil
    end
end

-- Player gets hit
function player_hit()
    game.lives = game.lives - 1

    if game.lives <= 0 then
        game.tank.alive = false
        end_game()
    else
        -- Respawn player
        game.tank.x = math.floor(GRID_WIDTH / 2)
        game.tank.y = GRID_HEIGHT - 2
        game.tank.direction = "up"
    end
end

-- End the game
function end_game()
    game.game_over = true
    game.paused = true

    if game.score > game.high_score then
        game.high_score = game.score
    end

    if game.score > 0 then
        save_score()
    end

    sync_game_ui()
end

-- Save score to Loro
function save_score()
    local entry = {
        player = my_name or my_did:sub(-8),
        score = game.score,
        stage = game.stage,
        timestamp = os.time(),
        date = os.date("%Y-%m-%d"),
    }

    scores_layer:push(entry)
    refresh_leaderboard()
end

-- Refresh leaderboard from Loro
function refresh_leaderboard()
    local all_scores = {}
    local len = scores_layer:length()

    for i = 0, len - 1 do
        local entry = scores_layer:get(i)
        if entry then
            table.insert(all_scores, entry)
        end
    end

    -- Sort by score descending
    table.sort(all_scores, function(a, b)
        return (a.score or 0) > (b.score or 0)
    end)

    -- Take top 10
    local leaderboard = {}
    for i = 1, math.min(10, #all_scores) do
        local entry = all_scores[i]
        table.insert(leaderboard, {
            player = entry.player or "???",
            score = entry.score or 0,
            stage = entry.stage or 1,
            date = entry.date or "",
        })

        if i == 1 and entry.score > game.high_score then
            game.high_score = entry.score
        end
    end

    ui:set("leaderboard", leaderboard)
    ui:set("high_score", game.high_score)
end

-- Broadcast player position via ephemeral
function broadcast_position()
    if not game.tank.alive then return end

    local payload = string.format(
        '{"type":"player","x":%d,"y":%d,"dir":"%s","alive":%s,"name":"%s"}',
        game.tank.x, game.tank.y, game.tank.direction,
        game.tank.alive and "true" or "false",
        my_name or "Player"
    )
    butler:send_ephemeral(payload)
end

-- Broadcast bullet via ephemeral
function broadcast_bullet(x, y, direction, owner)
    local payload = string.format(
        '{"type":"bullet","x":%d,"y":%d,"dir":"%s","owner":"%s"}',
        x, y, direction, owner
    )
    butler:send_ephemeral(payload)
end

-- Handle incoming ephemeral messages
function on_ephemeral(user_did, payload)
    if user_did == my_did then return end

    local msg_type = payload:match('"type":"([^"]+)"')

    if msg_type == "player" then
        -- Parse player position
        local x = tonumber(payload:match('"x":(-?%d+)'))
        local y = tonumber(payload:match('"y":(-?%d+)'))
        local dir = payload:match('"dir":"([^"]+)"')
        local alive = payload:match('"alive":true') ~= nil
        local name = payload:match('"name":"([^"]+)"')

        -- Only add remote player if we have valid coordinates (not at 0,0 default)
        if x and y and (x > 0 or y > 0) then
            remote_players[user_did] = {
                x = x,
                y = y,
                direction = dir or "up",
                alive = alive,
                color_index = color_index_for_user(user_did),
                name = name or user_did:sub(-8),
                last_seen = os.time(),
            }

            sync_game_ui()
        end

    elseif msg_type == "enemies" then
        -- Receive enemy positions from node
        local enemies_str = payload:match('"data":%[(.-)%]')
        if enemies_str then
            game.enemies = {}
            for enemy_data in enemies_str:gmatch('{[^}]+}') do
                local id = tonumber(enemy_data:match('"id":(%d+)'))
                local x = tonumber(enemy_data:match('"x":(-?%d+)'))
                local y = tonumber(enemy_data:match('"y":(-?%d+)'))
                local dir = enemy_data:match('"dir":"([^"]+)"')
                local hp = tonumber(enemy_data:match('"hp":(%d+)'))
                local is_boss = enemy_data:match('"boss":true') ~= nil

                if id and x and y then
                    game.enemies[id] = {
                        x = x,
                        y = y,
                        direction = dir or "down",
                        hp = hp or 1,
                        is_boss = is_boss,
                    }
                end
            end
        end

    elseif msg_type == "stage" then
        -- Node notifies stage change
        local new_stage = tonumber(payload:match('"stage":(%d+)'))
        if new_stage then
            game.stage = new_stage
            -- Reset player position on stage change
            game.tank.x = math.floor(GRID_WIDTH / 2)
            game.tank.y = GRID_HEIGHT - 2
            game.tank.direction = "up"
            game.bullets = {}
        end

    elseif msg_type == "score" then
        -- Node confirms score update (enemy destroyed)
        local points = tonumber(payload:match('"points":(%d+)'))
        if points then
            game.score = game.score + points
        end

    elseif msg_type == "bullet" then
        local x = tonumber(payload:match('"x":(-?%d+)'))
        local y = tonumber(payload:match('"y":(-?%d+)'))
        local dir = payload:match('"dir":"([^"]+)"')
        local owner = payload:match('"owner":"([^"]+)"')

        -- Add bullet to local game state so it renders
        if x and y and dir then
            local bullet = {
                x = x,
                y = y,
                direction = dir,
                owner = owner or "remote",
            }
            game.bullets[game.next_bullet_id] = bullet
            game.next_bullet_id = game.next_bullet_id + 1
        end

        -- Check collision with local player (enemy bullets)
        if owner == "enemy" and game.tank.alive then
            if x == game.tank.x and y == game.tank.y then
                player_hit()
            end
        end

        -- Check collision with bricks (all players can destroy)
        for i, obs in ipairs(game.obstacles) do
            if obs.x == x and obs.y == y and obs.kind == "brick" then
                destroy_brick(x, y)
                break
            end
        end
    end
end

-- Cleanup stale remote players
function cleanup_stale_players()
    local now = os.time()
    local stale = {}
    for did, player in pairs(remote_players) do
        if now - player.last_seen > 3 then
            table.insert(stale, did)
        end
    end
    for _, did in ipairs(stale) do
        remote_players[did] = nil
    end
end

-- Handle peer joining
function on_peer_joined(user_did)
    -- Broadcast our position immediately so new peer sees us
    broadcast_position()
end

-- Handle peer leaving
function on_peer_left(user_did)
    remote_players[user_did] = nil
    sync_game_ui()
end

-- Game tick - called by runtime at ~60fps
function tick()
    -- Update FPS counter
    game.fps_frames = game.fps_frames + 1
    local now = os.time()
    if now ~= game.fps_last_time then
        game.fps = game.fps_frames
        game.fps_frames = 0
        game.fps_last_time = now
        ui:set("fps", game.fps)

        -- Cleanup stale remote players (once per second)
        cleanup_stale_players()
    end

    -- Broadcast our position periodically (every few frames)
    game.frame_count = game.frame_count + 1
    if game.frame_count % 6 == 0 then
        broadcast_position()
    end

    if game.paused or game.game_over then
        sync_game_ui()  -- Still sync UI to show remote players
        return
    end

    -- Update shoot cooldown
    if game.shoot_cooldown > 0 then
        game.shoot_cooldown = game.shoot_cooldown - 1
    end

    -- Move bullets (fast)
    game.bullet_move_counter = game.bullet_move_counter + 1
    if game.bullet_move_counter >= BULLET_MOVE_FRAMES then
        game.bullet_move_counter = 0
        move_bullets()
    end

    -- Note: Enemy AI and stage progression are handled by the node
    -- Clients receive enemy positions via ephemeral broadcasts

    -- Update UI
    sync_game_ui()
end

-- Sync game state to UI
function sync_game_ui()
    -- Build players array (local player + remote players)
    local players_array = {}

    -- Add local player first
    table.insert(players_array, {
        x = game.tank.x,
        y = game.tank.y,
        direction = game.tank.direction,
        alive = game.tank.alive,
        is_me = true,
        color_index = color_index_for_user(my_did or ""),
        name = my_name or "You",
    })

    -- Add remote players
    for did, player in pairs(remote_players) do
        table.insert(players_array, {
            x = player.x,
            y = player.y,
            direction = player.direction,
            alive = player.alive,
            is_me = false,
            color_index = player.color_index,
            name = player.name,
        })
    end
    ui:set("players", players_array)

    -- Also set single tank for backwards compatibility
    ui:set("tank", {
        x = game.tank.x,
        y = game.tank.y,
        direction = game.tank.direction,
        alive = game.tank.alive,
    })

    -- Convert enemies to array for UI
    local enemies_array = {}
    for _, enemy in pairs(game.enemies) do
        table.insert(enemies_array, {
            x = enemy.x,
            y = enemy.y,
            direction = enemy.direction,
            hp = enemy.hp,
            is_boss = enemy.is_boss,
        })
    end
    ui:set("enemies", enemies_array)

    -- Convert bullets to array for UI
    local bullets_array = {}
    for _, bullet in pairs(game.bullets) do
        table.insert(bullets_array, {
            x = bullet.x,
            y = bullet.y,
            direction = bullet.direction,
        })
    end
    ui:set("bullets", bullets_array)

    -- Obstacles
    ui:set("obstacles", game.obstacles)

    -- Game stats
    ui:set("score", game.score)
    ui:set("lives", game.lives)
    ui:set("stage", game.stage)
    ui:set("game_over", game.game_over)
    ui:set("game_paused", game.paused)
end

-- Handle click events
function on_click(target)
    if target == "toggle_pause" then
        if not game.game_over then
            game.paused = not game.paused
            sync_game_ui()
        end
    elseif target == "restart" then
        request_restart()
    end
end

-- Handle key events (immediate movement for responsive controls)
function on_key_pressed(key)
    if game.game_over then
        return
    end

    -- Movement (only when game is running)
    if not game.paused then
        if key == "up" or key == "down" or key == "left" or key == "right" then
            -- Change direction
            game.tank.direction = key

            -- Move immediately
            local vec = DIR_VECTORS[key]
            local new_x = game.tank.x + vec.dx
            local new_y = game.tank.y + vec.dy

            if not is_blocked(new_x, new_y) then
                game.tank.x = new_x
                game.tank.y = new_y
            end

            sync_game_ui()
            broadcast_position()  -- Sync to other players
        end

        -- Shooting
        if key == "shoot" or key == "space" then
            if game.shoot_cooldown == 0 then
                fire_bullet("player", game.tank.x, game.tank.y, game.tank.direction)
                -- Broadcast bullet position with offset (same as fire_bullet creates)
                local vec = DIR_VECTORS[game.tank.direction]
                local bullet_x = game.tank.x + vec.dx
                local bullet_y = game.tank.y + vec.dy
                broadcast_bullet(bullet_x, bullet_y, game.tank.direction, my_did)
                game.shoot_cooldown = SHOOT_COOLDOWN_FRAMES
                sync_game_ui()
            end
        end
    end
end

-- Handle Loro changes
function on_loro_change(layer_name, change_type)
    if layer_name:match("/scores$") then
        refresh_leaderboard()
    elseif layer_name:match("/obstacles$") then
        -- Reload obstacles from CRDT layer when changed
        load_obstacles_from_layer()
        sync_game_ui()
    end
end

-- API exports for testing
api.export("request_restart", request_restart)
api.export("get_score", function() return game.score end)
api.export("get_stage", function() return game.stage end)
api.export("get_lives", function() return game.lives end)
