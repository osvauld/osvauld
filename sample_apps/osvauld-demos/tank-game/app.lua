-- Tank Brick Game App Logic
-- Classic tank battle with enemies, bosses, and synced high scores

-- Grid settings
local GRID_WIDTH = 20
local GRID_HEIGHT = 15
local CELL_SIZE = 24

-- Timing (frames at ~60fps)
local PLAYER_MOVE_FRAMES = 8     -- Player moves every N frames
local ENEMY_MOVE_FRAMES = 12     -- Base enemy speed
local BULLET_MOVE_FRAMES = 2     -- Bullets move fast
local SHOOT_COOLDOWN_FRAMES = 15 -- Min frames between player shots
local ENEMY_SHOOT_CHANCE = 2     -- % chance enemy shoots when facing player

-- Game settings
local ENEMIES_PER_STAGE = 5      -- Base enemies per stage
local BOSS_HP = 5                -- Boss hit points
local ENEMY_HP = 1               -- Normal enemy HP

-- Game state
local game = {
    tank = { x = 10, y = 13, direction = "up", alive = true },
    enemies = {},           -- id -> { x, y, direction, hp, is_boss }
    bullets = {},           -- id -> { x, y, direction, owner }
    obstacles = {},         -- Array of { x, y, type }
    stage = 1,
    score = 0,
    high_score = 0,
    lives = 3,
    game_over = false,
    paused = true,
    enemies_to_spawn = 0,
    boss_spawned = false,
    boss_defeated = false,
    frame_count = 0,
    player_move_counter = 0,
    enemy_move_counter = 0,
    bullet_move_counter = 0,
    shoot_cooldown = 0,
    next_enemy_id = 1,
    next_bullet_id = 1,
    fps_frames = 0,
    fps_last_time = 0,
    fps = 0,
}

-- Loro layers
local page_id = nil
local scores_layer = nil

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

-- Initialize the app
function on_init()
    page_id = permit:page_id()
    scores_layer = loro:get_or_create_layer(page_id .. "/scores", "list")

    -- Set grid settings for UI
    ui:set("grid_width", GRID_WIDTH)
    ui:set("grid_height", GRID_HEIGHT)
    ui:set("cell_size", CELL_SIZE)

    reset_game()
    refresh_leaderboard()
end

-- Reset game to initial state
function reset_game()
    -- Reset player tank
    game.tank = {
        x = math.floor(GRID_WIDTH / 2),
        y = GRID_HEIGHT - 2,
        direction = "up",
        alive = true,
    }

    game.enemies = {}
    game.bullets = {}
    game.stage = 1
    game.score = 0
    game.lives = 3
    game.game_over = false
    game.paused = true
    game.enemies_to_spawn = ENEMIES_PER_STAGE
    game.boss_spawned = false
    game.boss_defeated = false
    game.frame_count = 0
    game.player_move_counter = 0
    game.enemy_move_counter = 0
    game.bullet_move_counter = 0
    game.shoot_cooldown = 0
    game.next_enemy_id = 1
    game.next_bullet_id = 1

    -- Generate obstacles for stage 1
    generate_obstacles()

    -- Spawn initial enemies
    for i = 1, 3 do
        spawn_enemy()
    end

    sync_game_ui()
end

-- Generate obstacles for the current stage
function generate_obstacles()
    game.obstacles = {}

    -- Create some brick walls (pattern based on stage)
    local patterns = {
        -- Stage 1: Simple horizontal walls
        function()
            -- Top horizontal wall with gaps
            for x = 3, GRID_WIDTH - 4 do
                if x ~= 10 then
                    table.insert(game.obstacles, { x = x, y = 4, kind = "brick" })
                end
            end
            -- Middle barriers
            for y = 7, 9 do
                table.insert(game.obstacles, { x = 4, y = y, kind = "brick" })
                table.insert(game.obstacles, { x = 15, y = y, kind = "brick" })
            end
            -- Steel walls (indestructible) near spawn
            table.insert(game.obstacles, { x = 9, y = 12, kind = "steel" })
            table.insert(game.obstacles, { x = 10, y = 12, kind = "steel" })
            table.insert(game.obstacles, { x = 11, y = 12, kind = "steel" })
        end,
        -- Stage 2: Cross pattern
        function()
            -- Vertical center
            for y = 3, 11 do
                if y ~= 7 then
                    table.insert(game.obstacles, { x = 10, y = y, kind = "brick" })
                end
            end
            -- Horizontal center
            for x = 4, 16 do
                if x ~= 10 then
                    table.insert(game.obstacles, { x = x, y = 7, kind = "brick" })
                end
            end
            -- Steel corners
            table.insert(game.obstacles, { x = 2, y = 2, kind = "steel" })
            table.insert(game.obstacles, { x = 17, y = 2, kind = "steel" })
        end,
        -- Stage 3+: Random maze-like
        function()
            for _ = 1, 25 + game.stage * 3 do
                local x = math.random(2, GRID_WIDTH - 3)
                local y = math.random(3, GRID_HEIGHT - 4)
                -- Don't place near player spawn
                if not (y >= GRID_HEIGHT - 3 and x >= 8 and x <= 12) then
                    local obstacle_kind = math.random() < 0.15 and "steel" or "brick"
                    table.insert(game.obstacles, { x = x, y = y, kind = obstacle_kind })
                end
            end
        end,
    }

    local pattern_idx = math.min(game.stage, #patterns)
    patterns[pattern_idx]()
end

-- Check if a cell is blocked
function is_blocked(x, y, ignore_entity_type)
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

    -- Player tank check (if not ignoring)
    if ignore_entity_type ~= "player" and game.tank.alive then
        if game.tank.x == x and game.tank.y == y then
            return true
        end
    end

    -- Enemy check (if not ignoring)
    if ignore_entity_type ~= "enemy" then
        for _, enemy in pairs(game.enemies) do
            if enemy.x == x and enemy.y == y then
                return true
            end
        end
    end

    return false
end

-- Spawn an enemy at a random edge position
function spawn_enemy()
    if game.enemies_to_spawn <= 0 then
        return false
    end

    -- Try to find valid spawn position (top row, away from obstacles)
    local spawn_positions = { 1, 5, 10, 14, 18 }
    for _, try_x in ipairs(spawn_positions) do
        if not is_blocked(try_x, 1, "enemy") then
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
                        table.remove(game.obstacles, i)
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
                if bullet.owner == "player" then
                    for enemy_id, enemy in pairs(game.enemies) do
                        if bullet.x == enemy.x and bullet.y == enemy.y then
                            enemy.hp = enemy.hp - 1
                            table.insert(bullets_to_remove, id)

                            if enemy.hp <= 0 then
                                -- Enemy destroyed
                                local points = enemy.is_boss and 100 or 10
                                game.score = game.score + points
                                game.enemies[enemy_id] = nil

                                if enemy.is_boss then
                                    game.boss_defeated = true
                                end
                            end
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

-- Move enemies (simple AI)
function move_enemies()
    for id, enemy in pairs(game.enemies) do
        -- Decide action: move or change direction
        local vec = DIR_VECTORS[enemy.direction]
        local new_x = enemy.x + vec.dx
        local new_y = enemy.y + vec.dy

        -- Check if can move forward
        local can_move = not is_blocked(new_x, new_y, "enemy")

        if can_move then
            enemy.x = new_x
            enemy.y = new_y
        else
            -- Pick a new random direction
            local directions = { "up", "down", "left", "right" }
            -- Shuffle
            for i = #directions, 2, -1 do
                local j = math.random(i)
                directions[i], directions[j] = directions[j], directions[i]
            end

            for _, dir in ipairs(directions) do
                local v = DIR_VECTORS[dir]
                if not is_blocked(enemy.x + v.dx, enemy.y + v.dy, "enemy") then
                    enemy.direction = dir
                    break
                end
            end
        end

        -- Shooting: check if facing player
        local shoot_chance = enemy.is_boss and (ENEMY_SHOOT_CHANCE * 3) or ENEMY_SHOOT_CHANCE
        if math.random(100) <= shoot_chance then
            local facing_player = false
            local vec = DIR_VECTORS[enemy.direction]
            local check_x, check_y = enemy.x, enemy.y

            for _ = 1, math.max(GRID_WIDTH, GRID_HEIGHT) do
                check_x = check_x + vec.dx
                check_y = check_y + vec.dy

                if check_x < 0 or check_x >= GRID_WIDTH or check_y < 0 or check_y >= GRID_HEIGHT then
                    break
                end

                if game.tank.alive and check_x == game.tank.x and check_y == game.tank.y then
                    facing_player = true
                    break
                end

                -- Stop at obstacles
                local blocked = false
                for _, obs in ipairs(game.obstacles) do
                    if obs.x == check_x and obs.y == check_y then
                        blocked = true
                        break
                    end
                end
                if blocked then break end
            end

            if facing_player or math.random(100) <= 10 then
                fire_bullet(id, enemy.x, enemy.y, enemy.direction)
            end
        end
    end
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
    game.bullets = {}
    game.enemies = {}

    -- Reset player position
    game.tank.x = math.floor(GRID_WIDTH / 2)
    game.tank.y = GRID_HEIGHT - 2
    game.tank.direction = "up"

    -- Generate new obstacles
    generate_obstacles()

    -- Spawn initial enemies
    for i = 1, 3 do
        spawn_enemy()
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
    local my_did = permit:my_did()
    local short_did = my_did:sub(-8)

    local entry = {
        player = short_did,
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
    end

    if game.paused or game.game_over then
        return
    end

    game.frame_count = game.frame_count + 1

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

    -- Move enemies
    local enemy_speed = math.max(4, ENEMY_MOVE_FRAMES - game.stage + 1)
    game.enemy_move_counter = game.enemy_move_counter + 1
    if game.enemy_move_counter >= enemy_speed then
        game.enemy_move_counter = 0
        move_enemies()
    end

    -- Check stage completion
    check_stage_complete()

    -- Update UI
    sync_game_ui()
end

-- Sync game state to UI
function sync_game_ui()
    -- Convert tank to UI format
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
        reset_game()
    end
end

-- Handle key events
function on_key_pressed(key)
    if game.game_over then
        return
    end

    -- Movement (only when game is running)
    if not game.paused then
        game.player_move_counter = game.player_move_counter + 1

        local moved = false
        if key == "up" or key == "down" or key == "left" or key == "right" then
            -- Change direction
            game.tank.direction = key

            -- Move if enough frames passed
            if game.player_move_counter >= PLAYER_MOVE_FRAMES then
                game.player_move_counter = 0
                local vec = DIR_VECTORS[key]
                local new_x = game.tank.x + vec.dx
                local new_y = game.tank.y + vec.dy

                if not is_blocked(new_x, new_y, "player") then
                    game.tank.x = new_x
                    game.tank.y = new_y
                    moved = true
                end
            end
        end

        -- Shooting
        if key == "shoot" or key == "space" then
            if game.shoot_cooldown == 0 then
                fire_bullet("player", game.tank.x, game.tank.y, game.tank.direction)
                game.shoot_cooldown = SHOOT_COOLDOWN_FRAMES
                sync_game_ui()
            end
        end

        if moved then
            sync_game_ui()
        end
    end
end

-- Handle Loro changes
function on_loro_change(layer_name, change_type)
    if layer_name:match("/scores$") then
        refresh_leaderboard()
    end
end

-- API exports for testing
api.export("reset_game", reset_game)
api.export("get_score", function() return game.score end)
api.export("get_stage", function() return game.stage end)
api.export("get_lives", function() return game.lives end)
