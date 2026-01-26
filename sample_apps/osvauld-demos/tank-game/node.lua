-- Tank Game Node Logic
-- Runs on node: enemy AI, obstacle generation, game state management
-- Players send positions via ephemeral, node broadcasts enemy state

-- Grid settings (must match client)
local GRID_WIDTH = 36
local GRID_HEIGHT = 28

-- Timing (frames at tick rate)
local ENEMY_MOVE_FRAMES = 12
local ENEMY_SHOOT_CHANCE = 5  -- 5% chance per move to shoot

-- Game settings
local ENEMIES_PER_STAGE = 5
local BOSS_HP = 5
local ENEMY_HP = 1

-- Game state
local game = {
    stage = 1,
    enemies = {},           -- id -> { x, y, direction, hp, is_boss }
    enemies_to_spawn = 0,
    boss_spawned = false,
    boss_defeated = false,
    next_enemy_id = 1,
    enemy_move_counter = 0,
    frame_count = 0,
}

-- Player tracking (from ephemeral)
local players = {}  -- did -> { x, y, direction, alive, last_seen }

-- Loro layers
local page_id = nil
local obstacles_layer = nil

-- Direction vectors
local DIR_VECTORS = {
    up = { dx = 0, dy = -1 },
    down = { dx = 0, dy = 1 },
    left = { dx = -1, dy = 0 },
    right = { dx = 1, dy = 0 },
}

-- Initialize the node game logic
function on_init()
    page_id = permit:page_id()

    -- Get or create obstacles layer
    obstacles_layer = loro:get_or_create_layer(page_id .. "/obstacles", "map")

    -- Check if obstacles exist, if not generate them
    local existing_keys = obstacles_layer:keys()
    if #existing_keys == 0 then
        generate_obstacles()
    end

    -- Initialize game state
    reset_game()

    print("[Node] Tank game initialized for page: " .. page_id)
end

-- Reset game state
function reset_game()
    game.stage = 1
    game.enemies = {}
    game.enemies_to_spawn = ENEMIES_PER_STAGE
    game.boss_spawned = false
    game.boss_defeated = false
    game.next_enemy_id = 1
    game.enemy_move_counter = 0
    game.frame_count = 0

    -- Spawn initial enemies
    for i = 1, 3 do
        spawn_enemy()
    end

    print("[Node] Game reset, spawned initial enemies")
end

-- Generate obstacles and write to CRDT layer
function generate_obstacles()
    -- Clear existing obstacles
    for _, key in ipairs(obstacles_layer:keys()) do
        obstacles_layer:set(key, nil)
    end

    -- Stage 1: Walls adapted to larger grid
    -- Top horizontal wall with gaps
    local gap1 = math.floor(GRID_WIDTH / 3)
    local gap2 = math.floor(GRID_WIDTH * 2 / 3)
    for x = 4, GRID_WIDTH - 5 do
        if x ~= gap1 and x ~= gap2 then
            local key = x .. "_5"
            obstacles_layer:set(key, { x = x, y = 5, kind = "brick" })
        end
    end

    -- Middle horizontal wall
    for x = 4, GRID_WIDTH - 5 do
        if x ~= math.floor(GRID_WIDTH / 2) then
            local key = x .. "_" .. math.floor(GRID_HEIGHT / 2)
            obstacles_layer:set(key, { x = x, y = math.floor(GRID_HEIGHT / 2), kind = "brick" })
        end
    end

    -- Left and right barriers
    local barrier_y_start = math.floor(GRID_HEIGHT / 3)
    local barrier_y_end = math.floor(GRID_HEIGHT * 2 / 3)
    for y = barrier_y_start, barrier_y_end do
        obstacles_layer:set("5_" .. y, { x = 5, y = y, kind = "brick" })
        obstacles_layer:set((GRID_WIDTH - 6) .. "_" .. y, { x = GRID_WIDTH - 6, y = y, kind = "brick" })
    end

    -- Steel walls near player spawn (bottom center)
    local spawn_x = math.floor(GRID_WIDTH / 2)
    local spawn_y = GRID_HEIGHT - 4
    obstacles_layer:set((spawn_x - 1) .. "_" .. spawn_y, { x = spawn_x - 1, y = spawn_y, kind = "steel" })
    obstacles_layer:set(spawn_x .. "_" .. spawn_y, { x = spawn_x, y = spawn_y, kind = "steel" })
    obstacles_layer:set((spawn_x + 1) .. "_" .. spawn_y, { x = spawn_x + 1, y = spawn_y, kind = "steel" })

    print("[Node] Generated obstacles for stage " .. game.stage)
end

-- Load obstacles from CRDT into local cache
function load_obstacles()
    local obstacles = {}
    for _, key in ipairs(obstacles_layer:keys()) do
        local obs = obstacles_layer:get(key)
        if obs then
            table.insert(obstacles, obs)
        end
    end
    return obstacles
end

-- Check if a cell is blocked
function is_blocked(x, y)
    -- Boundary check
    if x < 0 or x >= GRID_WIDTH or y < 0 or y >= GRID_HEIGHT then
        return true
    end

    -- Obstacle check
    local key = x .. "_" .. y
    local obs = obstacles_layer:get(key)
    if obs then
        return true
    end

    -- Player check
    for _, player in pairs(players) do
        if player.alive and player.x == x and player.y == y then
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

-- Spawn an enemy at a random edge position
function spawn_enemy()
    if game.enemies_to_spawn <= 0 then
        return false
    end

    -- Spawn positions adapted to grid width
    local spawn_positions = {}
    for i = 2, GRID_WIDTH - 3, 4 do
        table.insert(spawn_positions, i)
    end

    for _, try_x in ipairs(spawn_positions) do
        if not is_blocked(try_x, 1) then
            local enemy = {
                x = try_x,
                y = 1,
                direction = "down",
                hp = ENEMY_HP,
                is_boss = false,
            }
            game.enemies[game.next_enemy_id] = enemy
            game.next_enemy_id = game.next_enemy_id + 1
            game.enemies_to_spawn = game.enemies_to_spawn - 1
            return true
        end
    end

    return false
end

-- Spawn the boss
function spawn_boss()
    if game.boss_spawned then
        return false
    end

    local boss = {
        x = math.floor(GRID_WIDTH / 2),
        y = 1,
        direction = "down",
        hp = BOSS_HP,
        is_boss = true,
    }
    game.enemies[game.next_enemy_id] = boss
    game.next_enemy_id = game.next_enemy_id + 1
    game.boss_spawned = true
    return true
end

-- Find nearest player to an enemy
function find_nearest_player(enemy)
    local nearest = nil
    local nearest_dist = math.huge

    for did, player in pairs(players) do
        if player.alive then
            local dist = math.abs(player.x - enemy.x) + math.abs(player.y - enemy.y)
            if dist < nearest_dist then
                nearest = player
                nearest_dist = dist
            end
        end
    end

    return nearest
end

-- Move enemies (AI targeting players)
function move_enemies()
    for id, enemy in pairs(game.enemies) do
        local nearest = find_nearest_player(enemy)

        if nearest then
            -- Try to move towards nearest player
            local dx = 0
            local dy = 0

            if nearest.x < enemy.x then dx = -1
            elseif nearest.x > enemy.x then dx = 1
            end

            if nearest.y < enemy.y then dy = -1
            elseif nearest.y > enemy.y then dy = 1
            end

            -- Prefer horizontal or vertical based on distance
            local try_directions = {}
            if math.abs(nearest.x - enemy.x) > math.abs(nearest.y - enemy.y) then
                if dx ~= 0 then table.insert(try_directions, dx > 0 and "right" or "left") end
                if dy ~= 0 then table.insert(try_directions, dy > 0 and "down" or "up") end
            else
                if dy ~= 0 then table.insert(try_directions, dy > 0 and "down" or "up") end
                if dx ~= 0 then table.insert(try_directions, dx > 0 and "right" or "left") end
            end

            -- Add remaining directions
            for _, dir in ipairs({"up", "down", "left", "right"}) do
                local found = false
                for _, d in ipairs(try_directions) do
                    if d == dir then found = true break end
                end
                if not found then table.insert(try_directions, dir) end
            end

            -- Try to move in preferred directions
            local moved = false
            for _, dir in ipairs(try_directions) do
                local vec = DIR_VECTORS[dir]
                local new_x = enemy.x + vec.dx
                local new_y = enemy.y + vec.dy

                if not is_blocked(new_x, new_y) then
                    enemy.x = new_x
                    enemy.y = new_y
                    enemy.direction = dir
                    moved = true
                    break
                end
            end

            -- Shooting: enemies shoot in their facing direction periodically
            -- Boss has higher chance, regular enemies have base chance
            local shoot_chance = enemy.is_boss and (ENEMY_SHOOT_CHANCE * 3) or ENEMY_SHOOT_CHANCE
            if math.random(100) <= shoot_chance then
                -- Fire bullet in current direction (bullet will travel and hit whatever is in path)
                broadcast_bullet(enemy.x, enemy.y, enemy.direction, "enemy")
            end
        else
            -- No players, random movement
            local directions = {"up", "down", "left", "right"}
            local dir = directions[math.random(#directions)]
            local vec = DIR_VECTORS[dir]
            local new_x = enemy.x + vec.dx
            local new_y = enemy.y + vec.dy

            if not is_blocked(new_x, new_y) then
                enemy.x = new_x
                enemy.y = new_y
                enemy.direction = dir
            end
        end
    end
end

-- Broadcast bullet via ephemeral
function broadcast_bullet(x, y, direction, owner)
    -- Skip if no active players (avoid ephemeral spam when users are in other apps)
    if not has_active_players() then
        return
    end

    local vec = DIR_VECTORS[direction]
    local bullet_x = x + vec.dx
    local bullet_y = y + vec.dy

    local payload = string.format(
        '{"type":"bullet","x":%d,"y":%d,"dir":"%s","owner":"%s"}',
        bullet_x, bullet_y, direction, owner
    )
    butler:send_ephemeral(payload)
end

-- Check if there are active players (users who sent positions recently)
function has_active_players()
    local now = os.time()
    for did, player in pairs(players) do
        -- Player is active if they sent position in last 10 seconds
        if now - player.last_seen < 10 then
            return true
        end
    end
    return false
end

-- Broadcast enemies state via ephemeral
function broadcast_enemies()
    -- Skip if no active players (avoid ephemeral spam when users are in other apps)
    if not has_active_players() then
        return
    end

    local enemies_data = {}
    for id, enemy in pairs(game.enemies) do
        table.insert(enemies_data, string.format(
            '{"id":%d,"x":%d,"y":%d,"dir":"%s","hp":%d,"boss":%s}',
            id, enemy.x, enemy.y, enemy.direction, enemy.hp,
            enemy.is_boss and "true" or "false"
        ))
    end

    local payload = '{"type":"enemies","data":[' .. table.concat(enemies_data, ",") .. ']}'
    butler:send_ephemeral(payload)
end

-- Broadcast score update to player who destroyed enemy
function broadcast_score(points, player_did)
    -- Score updates are always sent when there are active players
    local payload = string.format(
        '{"type":"score","points":%d,"player":"%s"}',
        points, player_did
    )
    butler:send_ephemeral(payload)
end

-- Broadcast stage change to all players
function broadcast_stage()
    -- Stage changes are always sent when there are active players
    local payload = string.format('{"type":"stage","stage":%d}', game.stage)
    butler:send_ephemeral(payload)
end

-- Check for stage completion
function check_stage_complete()
    if game.boss_defeated then
        next_stage()
        return true
    end

    -- Count remaining enemies
    local enemy_count = 0
    for _ in pairs(game.enemies) do
        enemy_count = enemy_count + 1
    end

    -- Spawn more enemies if needed
    if enemy_count < 3 and game.enemies_to_spawn > 0 then
        spawn_enemy()
    end

    -- Spawn boss when all regular enemies defeated
    if enemy_count == 0 and game.enemies_to_spawn == 0 and not game.boss_spawned then
        spawn_boss()
    end

    return false
end

-- Advance to next stage
function next_stage()
    game.stage = game.stage + 1
    game.enemies_to_spawn = ENEMIES_PER_STAGE + game.stage - 1
    game.boss_spawned = false
    game.boss_defeated = false
    game.enemies = {}

    -- Generate new obstacles
    generate_obstacles()

    -- Spawn initial enemies
    for i = 1, 3 do
        spawn_enemy()
    end

    -- Notify all clients of stage change
    broadcast_stage()

    print("[Node] Advanced to stage " .. game.stage)
end

-- Handle incoming ephemeral from players
function on_ephemeral(user_did, payload)
    local msg_type = payload:match('"type":"([^"]+)"')

    if msg_type == "player" then
        -- Update player position
        local x = tonumber(payload:match('"x":(-?%d+)'))
        local y = tonumber(payload:match('"y":(-?%d+)'))
        local dir = payload:match('"dir":"([^"]+)"')
        local alive = payload:match('"alive":true') ~= nil

        if x and y then
            players[user_did] = {
                x = x,
                y = y,
                direction = dir or "up",
                alive = alive,
                last_seen = os.time(),
            }
        end

    elseif msg_type == "bullet" then
        -- Player bullet - check for enemy hits
        local x = tonumber(payload:match('"x":(-?%d+)'))
        local y = tonumber(payload:match('"y":(-?%d+)'))
        local owner = payload:match('"owner":"([^"]+)"')

        if x and y and owner ~= "enemy" then
            -- Check if bullet hits any enemy
            for enemy_id, enemy in pairs(game.enemies) do
                if enemy.x == x and enemy.y == y then
                    enemy.hp = enemy.hp - 1
                    if enemy.hp <= 0 then
                        -- Award points based on enemy type
                        local points = enemy.is_boss and 100 or 10
                        broadcast_score(points, owner)

                        game.enemies[enemy_id] = nil
                        if enemy.is_boss then
                            game.boss_defeated = true
                        end
                        print("[Node] Enemy " .. enemy_id .. " destroyed by " .. owner)
                    end
                    break
                end
            end
        end

    elseif msg_type == "restart_request" then
        -- Player requested restart
        reset_game()
        generate_obstacles()
        -- Notify all clients of stage reset
        broadcast_stage()
        print("[Node] Game restarted by player: " .. user_did)
    end
end

-- Cleanup stale players
function cleanup_stale_players()
    local now = os.time()
    local stale = {}
    for did, player in pairs(players) do
        if now - player.last_seen > 5 then
            table.insert(stale, did)
        end
    end
    for _, did in ipairs(stale) do
        players[did] = nil
    end
end

-- Game tick - called by node runtime
function tick()
    game.frame_count = game.frame_count + 1

    -- Cleanup stale players periodically
    if game.frame_count % 60 == 0 then
        cleanup_stale_players()
    end

    -- Move enemies
    local enemy_speed = math.max(4, ENEMY_MOVE_FRAMES - game.stage + 1)
    game.enemy_move_counter = game.enemy_move_counter + 1
    if game.enemy_move_counter >= enemy_speed then
        game.enemy_move_counter = 0
        move_enemies()
    end

    -- Check stage completion
    check_stage_complete()

    -- Broadcast enemy positions every few frames
    if game.frame_count % 3 == 0 then
        broadcast_enemies()
    end
end

-- Handle CRDT changes (obstacles destroyed by bullets)
function on_loro_change(layer_name, data)
    if layer_name:match("/obstacles$") then
        -- Obstacles changed, clients will pick this up via CRDT
        print("[Node] Obstacles layer changed")
    end
end
