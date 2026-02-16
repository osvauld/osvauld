-- Tank Raylib Game
-- Classic tank battle using Raylib immediate-mode rendering
-- Multiplayer via ephemeral broadcasts

-- Screen dimensions (from manifest)
local SCREEN_WIDTH = 800
local SCREEN_HEIGHT = 600

-- Grid settings
local CELL_SIZE = 20
local GRID_WIDTH = math.floor(SCREEN_WIDTH / CELL_SIZE)
local GRID_HEIGHT = math.floor(SCREEN_HEIGHT / CELL_SIZE)

-- Colors (hex strings)
local COLORS = {
    bg = "#1a1a2e",
    player = "#00ff00",
    enemy = "#ff0000",
    bullet = "#ffff00",
    brick = "#8b4513",
    steel = "#808080",
    text = "#ffffff",
    grid = "#333333",
}

-- Remote player colors (distinguishable from local green)
local REMOTE_COLORS = {"#00aaff", "#ff00ff", "#00ffff", "#ff8800", "#ffff00", "#aa00ff"}

-- Game state
local game = {
    tank = { x = 0, y = 0, direction = "up" },
    enemies = {},
    bullets = {},
    obstacles = {},
    score = 0,
    lives = 3,
    game_over = false,
    paused = false,
    move_timer = 0,
    shoot_cooldown = 0,
    frame_count = 0,
    broadcast_timer = 0,
}

-- Multiplayer state
local my_did = nil
local my_name = nil
local page_id = nil
local remote_players = {}  -- did -> { x, y, direction, alive, color, name, last_seen }

-- Timing
local MOVE_DELAY = 0.1  -- seconds between moves
local BULLET_SPEED = 0.03  -- seconds between bullet moves
local SHOOT_COOLDOWN = 0.3  -- seconds between shots
local ENEMY_MOVE_DELAY = 0.2  -- enemy move speed
local BROADCAST_INTERVAL = 0.1  -- broadcast position every 100ms (~6 frames at 60fps)
local STALE_TIMEOUT = 3  -- remove players not seen for 3 seconds

-- Direction vectors
local DIR_VECTORS = {
    up = { dx = 0, dy = -1 },
    down = { dx = 0, dy = 1 },
    left = { dx = -1, dy = 0 },
    right = { dx = 1, dy = 0 },
}

-- Assign consistent color based on DID
function color_for_user(did)
    local hash = 0
    for i = 1, #did do
        hash = (hash * 31 + string.byte(did, i)) % #REMOTE_COLORS
    end
    return REMOTE_COLORS[hash + 1]
end

-- Initialize game
function on_init()
    page_id = scribe:page_id()
    my_did = scribe:my_did()
    my_name = scribe:my_name() or (my_did and my_did:sub(-8)) or "Player"
    reset_game()
end

-- Reset game state
function reset_game()
    -- Place player at bottom center
    game.tank = {
        x = math.floor(GRID_WIDTH / 2),
        y = GRID_HEIGHT - 2,
        direction = "up",
    }

    -- Create some enemies
    game.enemies = {}
    for i = 1, 5 do
        table.insert(game.enemies, {
            x = math.random(2, GRID_WIDTH - 3),
            y = math.random(2, 5),
            direction = "down",
            move_timer = 0,
        })
    end

    -- Create obstacles (bricks and steel)
    game.obstacles = {}
    -- Add some brick walls
    for x = 5, GRID_WIDTH - 5, 6 do
        for y = 10, 15 do
            table.insert(game.obstacles, { x = x, y = y, kind = "brick" })
            table.insert(game.obstacles, { x = x + 1, y = y, kind = "brick" })
        end
    end
    -- Add some steel walls
    for x = 3, GRID_WIDTH - 3, 10 do
        table.insert(game.obstacles, { x = x, y = 8, kind = "steel" })
    end

    game.bullets = {}
    game.score = 0
    game.lives = 3
    game.game_over = false
    game.paused = false
    game.move_timer = 0
    game.shoot_cooldown = 0
    game.frame_count = 0
    game.broadcast_timer = 0
end

-- Broadcast player position via ephemeral
function broadcast_position()
    if not scribe then return end
    scribe:send("player", {
        x = game.tank.x,
        y = game.tank.y,
        dir = game.tank.direction,
        alive = not game.game_over,
        name = my_name or "Player",
    })
end

-- Handle incoming structured ephemeral messages
function on_ephemeral(from_did, func, args)
    if from_did == my_did then return end

    if func == "player" then
        local x = args.x
        local y = args.y
        if x and y then
            remote_players[from_did] = {
                x = x,
                y = y,
                direction = args.dir or "up",
                alive = args.alive,
                color = color_for_user(from_did),
                name = args.name or from_did:sub(-8),
                last_seen = os.time(),
            }
        end
    end
end

-- Handle peer joining
function on_peer_joined(user_did)
    broadcast_position()
end

-- Handle peer leaving
function on_peer_left(user_did)
    remote_players[user_did] = nil
end

-- Cleanup stale remote players
function cleanup_stale_players()
    local now = os.time()
    local stale = {}
    for did, player in pairs(remote_players) do
        if now - player.last_seen > STALE_TIMEOUT then
            table.insert(stale, did)
        end
    end
    for _, did in ipairs(stale) do
        remote_players[did] = nil
    end
end

-- Debug helpers
function get_remote_players_count()
    local count = 0
    for _ in pairs(remote_players) do count = count + 1 end
    return count
end

-- Check if position is blocked
function is_blocked(x, y, ignore_enemies)
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
    if not ignore_enemies then
        for _, enemy in ipairs(game.enemies) do
            if enemy.x == x and enemy.y == y then
                return true
            end
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
        owner = owner,
        move_timer = 0,
    }

    if not is_blocked(bullet.x, bullet.y, true) then
        table.insert(game.bullets, bullet)
    end
end

-- Update game state (called each frame)
function update(dt)
    -- Broadcast position periodically
    game.broadcast_timer = game.broadcast_timer + dt
    if game.broadcast_timer >= BROADCAST_INTERVAL then
        game.broadcast_timer = 0
        broadcast_position()
    end

    -- Cleanup stale remote players (every second via frame counting)
    game.frame_count = game.frame_count + 1
    if game.frame_count % 60 == 0 then
        cleanup_stale_players()
    end

    if game.game_over then
        -- Check for restart
        if input:key_down("enter") or input:key_down("space") then
            reset_game()
        end
        return
    end

    if game.paused then
        if input:key_down("escape") then
            game.paused = false
        end
        return
    end

    -- Handle pause
    if input:key_down("escape") then
        game.paused = true
        return
    end

    -- Update timers
    game.move_timer = game.move_timer + dt
    game.shoot_cooldown = math.max(0, game.shoot_cooldown - dt)

    -- Handle player input
    if game.move_timer >= MOVE_DELAY then
        local moved = false

        if input:key_down("up") or input:key_down("w") then
            game.tank.direction = "up"
            local new_y = game.tank.y - 1
            if not is_blocked(game.tank.x, new_y) then
                game.tank.y = new_y
                moved = true
            end
        elseif input:key_down("down") or input:key_down("s") then
            game.tank.direction = "down"
            local new_y = game.tank.y + 1
            if not is_blocked(game.tank.x, new_y) then
                game.tank.y = new_y
                moved = true
            end
        elseif input:key_down("left") or input:key_down("a") then
            game.tank.direction = "left"
            local new_x = game.tank.x - 1
            if not is_blocked(new_x, game.tank.y) then
                game.tank.x = new_x
                moved = true
            end
        elseif input:key_down("right") or input:key_down("d") then
            game.tank.direction = "right"
            local new_x = game.tank.x + 1
            if not is_blocked(new_x, game.tank.y) then
                game.tank.x = new_x
                moved = true
            end
        end

        if moved then
            game.move_timer = 0
            broadcast_position()
        end
    end

    -- Handle shooting
    if input:key_down("space") and game.shoot_cooldown <= 0 then
        fire_bullet("player", game.tank.x, game.tank.y, game.tank.direction)
        game.shoot_cooldown = SHOOT_COOLDOWN
    end

    -- Update bullets
    local bullets_to_remove = {}
    for i, bullet in ipairs(game.bullets) do
        bullet.move_timer = bullet.move_timer + dt

        if bullet.move_timer >= BULLET_SPEED then
            bullet.move_timer = 0
            local vec = DIR_VECTORS[bullet.direction]
            bullet.x = bullet.x + vec.dx
            bullet.y = bullet.y + vec.dy

            -- Check boundary
            if bullet.x < 0 or bullet.x >= GRID_WIDTH or bullet.y < 0 or bullet.y >= GRID_HEIGHT then
                table.insert(bullets_to_remove, i)
            else
                -- Check obstacle collision
                for j, obs in ipairs(game.obstacles) do
                    if obs.x == bullet.x and obs.y == bullet.y then
                        if obs.kind == "brick" then
                            table.remove(game.obstacles, j)
                            game.score = game.score + 1
                        end
                        table.insert(bullets_to_remove, i)
                        break
                    end
                end

                -- Check enemy collision (player bullets)
                if bullet.owner == "player" then
                    for j, enemy in ipairs(game.enemies) do
                        if enemy.x == bullet.x and enemy.y == bullet.y then
                            table.remove(game.enemies, j)
                            game.score = game.score + 10
                            table.insert(bullets_to_remove, i)
                            break
                        end
                    end
                end

                -- Check player collision (enemy bullets)
                if bullet.owner == "enemy" then
                    if bullet.x == game.tank.x and bullet.y == game.tank.y then
                        game.lives = game.lives - 1
                        if game.lives <= 0 then
                            game.game_over = true
                        else
                            -- Respawn player
                            game.tank.x = math.floor(GRID_WIDTH / 2)
                            game.tank.y = GRID_HEIGHT - 2
                        end
                        table.insert(bullets_to_remove, i)
                    end
                end
            end
        end
    end

    -- Remove destroyed bullets (in reverse order)
    table.sort(bullets_to_remove, function(a, b) return a > b end)
    for _, i in ipairs(bullets_to_remove) do
        table.remove(game.bullets, i)
    end

    -- Update enemies
    for _, enemy in ipairs(game.enemies) do
        enemy.move_timer = enemy.move_timer + dt

        if enemy.move_timer >= ENEMY_MOVE_DELAY then
            enemy.move_timer = 0

            -- Simple AI: move toward player or random
            local dx = game.tank.x - enemy.x
            local dy = game.tank.y - enemy.y

            local new_dir
            if math.abs(dx) > math.abs(dy) then
                new_dir = dx > 0 and "right" or "left"
            else
                new_dir = dy > 0 and "down" or "up"
            end

            -- Random chance to change direction
            if math.random() < 0.3 then
                local dirs = {"up", "down", "left", "right"}
                new_dir = dirs[math.random(#dirs)]
            end

            enemy.direction = new_dir
            local vec = DIR_VECTORS[new_dir]
            local new_x = enemy.x + vec.dx
            local new_y = enemy.y + vec.dy

            if not is_blocked(new_x, new_y) then
                enemy.x = new_x
                enemy.y = new_y
            end

            -- Random chance to shoot
            if math.random() < 0.1 then
                fire_bullet("enemy", enemy.x, enemy.y, enemy.direction)
            end
        end
    end

    -- Check win condition
    if #game.enemies == 0 then
        -- Spawn more enemies
        for i = 1, 5 do
            table.insert(game.enemies, {
                x = math.random(2, GRID_WIDTH - 3),
                y = math.random(2, 5),
                direction = "down",
                move_timer = 0,
            })
        end
    end
end

-- Draw game (called each frame)
function draw()
    -- Clear background
    canvas:clear(COLORS.bg)

    -- Draw obstacles
    for _, obs in ipairs(game.obstacles) do
        local color = obs.kind == "steel" and COLORS.steel or COLORS.brick
        canvas:rect(
            obs.x * CELL_SIZE,
            obs.y * CELL_SIZE,
            CELL_SIZE - 1,
            CELL_SIZE - 1,
            color
        )
    end

    -- Draw enemies
    for _, enemy in ipairs(game.enemies) do
        draw_tank(enemy.x, enemy.y, enemy.direction, COLORS.enemy)
    end

    -- Draw remote players
    for did, player in pairs(remote_players) do
        if player.alive ~= false then
            draw_tank(player.x, player.y, player.direction, player.color)
            -- Draw name label above remote tank
            canvas:text(
                player.x * CELL_SIZE - 5,
                player.y * CELL_SIZE - 14,
                player.name,
                10,
                player.color
            )
        end
    end

    -- Draw local player
    draw_tank(game.tank.x, game.tank.y, game.tank.direction, COLORS.player)
    -- Draw own name label
    canvas:text(
        game.tank.x * CELL_SIZE - 5,
        game.tank.y * CELL_SIZE - 14,
        my_name or "You",
        10,
        COLORS.player
    )

    -- Draw bullets
    for _, bullet in ipairs(game.bullets) do
        canvas:circle(
            bullet.x * CELL_SIZE + CELL_SIZE / 2,
            bullet.y * CELL_SIZE + CELL_SIZE / 2,
            4,
            COLORS.bullet
        )
    end

    -- Draw HUD
    canvas:text(10, 10, "Score: " .. game.score, 20, COLORS.text)
    canvas:text(10, 35, "Lives: " .. game.lives, 20, COLORS.text)

    -- Draw multiplayer info
    local remote_count = get_remote_players_count()
    canvas:text(SCREEN_WIDTH - 180, 10, "Players: " .. (1 + remote_count), 16, COLORS.text)

    -- Draw remote player names in HUD
    local y_offset = 30
    for did, player in pairs(remote_players) do
        canvas:text(SCREEN_WIDTH - 180, y_offset, player.name, 12, player.color)
        y_offset = y_offset + 15
    end

    -- Draw game over or pause screen
    if game.game_over then
        canvas:text(
            SCREEN_WIDTH / 2 - 80,
            SCREEN_HEIGHT / 2 - 20,
            "GAME OVER",
            32,
            COLORS.text
        )
        canvas:text(
            SCREEN_WIDTH / 2 - 100,
            SCREEN_HEIGHT / 2 + 20,
            "Press ENTER to restart",
            16,
            COLORS.text
        )
    elseif game.paused then
        canvas:text(
            SCREEN_WIDTH / 2 - 50,
            SCREEN_HEIGHT / 2,
            "PAUSED",
            32,
            COLORS.text
        )
    end
end

-- Draw a tank at grid position
function draw_tank(grid_x, grid_y, direction, color)
    local x = grid_x * CELL_SIZE
    local y = grid_y * CELL_SIZE
    local size = CELL_SIZE - 2

    -- Tank body
    canvas:rect(x + 1, y + 1, size, size, color)

    -- Tank barrel (direction indicator)
    local barrel_len = size / 2
    local cx = x + CELL_SIZE / 2
    local cy = y + CELL_SIZE / 2

    local bx, by
    if direction == "up" then
        bx, by = cx, cy - barrel_len
    elseif direction == "down" then
        bx, by = cx, cy + barrel_len
    elseif direction == "left" then
        bx, by = cx - barrel_len, cy
    else -- right
        bx, by = cx + barrel_len, cy
    end

    canvas:line(cx, cy, bx, by, "#ffffff")
end
