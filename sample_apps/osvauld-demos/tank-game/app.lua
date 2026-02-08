-- Tank Brick Game App Logic
-- Classic tank battle with enemies, bosses, and synced high scores
-- Uses scribe:bind() for declarative layer-UI sync

local GRID_WIDTH = 36
local GRID_HEIGHT = 28

local PLAYER_MOVE_FRAMES = 8
local ENEMY_MOVE_FRAMES = 12
local BULLET_MOVE_FRAMES = 2
local SHOOT_COOLDOWN_FRAMES = 15
local ENEMY_SHOOT_CHANCE = 2

local game = {
    tank = { x = 10, y = 13, direction = "up", alive = true },
    enemies = {},
    bullets = {},
    obstacles = {},
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

local page_id = nil
local scores_layer = nil
local obstacles_layer = nil

local my_did = nil
local my_name = nil
local remote_players = {}
local last_position_broadcast = 0

local PLAYER_COLORS = {"#0f380f", "#0000ff", "#ff00ff", "#00ffff", "#ff8800", "#00ff00"}

local DIR_VECTORS = {
    up = { dx = 0, dy = -1 },
    down = { dx = 0, dy = 1 },
    left = { dx = -1, dy = 0 },
    right = { dx = 1, dy = 0 },
}

local OPPOSITE = { up = "down", down = "up", left = "right", right = "left" }

function color_index_for_user(did)
    local hash = 0
    for i = 1, #did do
        hash = (hash * 31 + string.byte(did, i)) % #PLAYER_COLORS
    end
    return hash
end

function on_init()
    page_id = scribe:page_id()
    my_did = scribe:my_did()
    my_name = scribe:my_name() or my_did:sub(-8)

    scores_layer = scribe:list(page_id .. "/scores")
    obstacles_layer = scribe:map(page_id .. "/obstacles")

    -- Declarative binding: leaderboard auto-syncs to UI
    -- NOTE: Sorting by score is done in Slint UI (enables surgical updates)
    scribe:bind("leaderboard", "scores", {
        key = "player",  -- Use player as stable identity
        transform = function(entry)
            return {
                player = entry.player or "???",
                score = entry.score or 0,
                stage = entry.stage or 1,
                date = entry.date or "",
            }
        end
    })

    ui:set("grid_width", GRID_WIDTH)
    ui:set("grid_height", GRID_HEIGHT)

    init_client()
    timer.setInterval(16, game_tick)
end

function init_client()
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
    game.frame_count = 0
    game.bullet_move_counter = 0
    game.shoot_cooldown = 0
    game.next_bullet_id = 1

    load_obstacles_from_layer()
    sync_game_ui()
end

function get_remote_players_count()
    local count = 0
    for _ in pairs(remote_players) do count = count + 1 end
    return count
end

function get_remote_players_info()
    local info = {}
    for did, player in pairs(remote_players) do
        table.insert(info, { did = did:sub(-12), x = player.x, y = player.y, name = player.name })
    end
    return info
end

function get_game_state()
    return {
        tank_x = game.tank.x, tank_y = game.tank.y, tank_alive = game.tank.alive,
        paused = game.paused, game_over = game.game_over, remote_count = get_remote_players_count()
    }
end

function request_restart()
    scribe:send("restart_request", {})
    init_client()
end

function load_obstacles_from_layer()
    if not obstacles_layer then return end
    game.obstacles = {}
    for _, key in ipairs(obstacles_layer:keys()) do
        local obs = obstacles_layer:get(key)
        if obs then table.insert(game.obstacles, obs) end
    end
end

function destroy_brick(x, y)
    local key = x .. "_" .. y
    obstacles_layer:set(key, nil)
end

function is_blocked(x, y)
    if x < 0 or x >= GRID_WIDTH or y < 0 or y >= GRID_HEIGHT then return true end
    for _, obs in ipairs(game.obstacles) do
        if obs.x == x and obs.y == y then return true end
    end
    for _, enemy in pairs(game.enemies) do
        if enemy.x == x and enemy.y == y then return true end
    end
    return false
end

function fire_bullet(owner, x, y, direction)
    local vec = DIR_VECTORS[direction]
    local bullet = { x = x + vec.dx, y = y + vec.dy, direction = direction, owner = owner }
    if bullet.x >= 0 and bullet.x < GRID_WIDTH and bullet.y >= 0 and bullet.y < GRID_HEIGHT then
        game.bullets[game.next_bullet_id] = bullet
        game.next_bullet_id = game.next_bullet_id + 1
    end
end

function move_bullets()
    local bullets_to_remove = {}
    for id, bullet in pairs(game.bullets) do
        local vec = DIR_VECTORS[bullet.direction]
        bullet.x = bullet.x + vec.dx
        bullet.y = bullet.y + vec.dy

        if bullet.x < 0 or bullet.x >= GRID_WIDTH or bullet.y < 0 or bullet.y >= GRID_HEIGHT then
            table.insert(bullets_to_remove, id)
        else
            local hit_obstacle = false
            for _, obs in ipairs(game.obstacles) do
                if obs.x == bullet.x and obs.y == bullet.y then
                    hit_obstacle = true
                    if obs.kind == "brick" then
                        destroy_brick(obs.x, obs.y)
                        game.score = game.score + 1
                    end
                    break
                end
            end

            if hit_obstacle then
                table.insert(bullets_to_remove, id)
            else
                if bullet.owner ~= "player" and game.tank.alive then
                    if bullet.x == game.tank.x and bullet.y == game.tank.y then
                        player_hit()
                        table.insert(bullets_to_remove, id)
                    end
                end

                if bullet.owner == "player" then
                    for enemy_id, enemy in pairs(game.enemies) do
                        if bullet.x == enemy.x and bullet.y == enemy.y then
                            table.insert(bullets_to_remove, id)
                            break
                        end
                    end
                end

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

    for _, id in ipairs(bullets_to_remove) do game.bullets[id] = nil end
end

function player_hit()
    game.lives = game.lives - 1
    if game.lives <= 0 then
        game.tank.alive = false
        end_game()
    else
        game.tank.x = math.floor(GRID_WIDTH / 2)
        game.tank.y = GRID_HEIGHT - 2
        game.tank.direction = "up"
    end
end

function end_game()
    game.game_over = true
    game.paused = true
    if game.score > game.high_score then game.high_score = game.score end
    if game.score > 0 then save_score() end
    sync_game_ui()
end

function save_score()
    local entry = {
        player = my_name or my_did:sub(-8),
        score = game.score,
        stage = game.stage,
        timestamp = os.time(),
        date = os.date("%Y-%m-%d"),
    }
    scores_layer:push(entry)
end

function broadcast_position()
    if not game.tank.alive then return end
    scribe:send("player", {
        x = game.tank.x, y = game.tank.y, dir = game.tank.direction,
        alive = game.tank.alive, name = my_name or "Player"
    })
end

function broadcast_bullet(x, y, direction, owner)
    scribe:send("bullet", { x = x, y = y, dir = direction, owner = owner })
end

function on_ephemeral(from_did, func, args)
    if from_did == my_did then return end

    if func == "player" then
        if args.x and args.y and (args.x > 0 or args.y > 0) then
            remote_players[from_did] = {
                x = args.x, y = args.y, direction = args.dir or "up", alive = args.alive,
                color_index = color_index_for_user(from_did), name = args.name or from_did:sub(-8),
                last_seen = os.time(),
            }
            sync_game_ui()
        end
    elseif func == "enemies" then
        if args.data then
            game.enemies = {}
            for _, enemy in ipairs(args.data) do
                if enemy.id and enemy.x and enemy.y then
                    game.enemies[enemy.id] = {
                        x = enemy.x, y = enemy.y, direction = enemy.dir or "down",
                        hp = enemy.hp or 1, is_boss = enemy.boss or false,
                    }
                end
            end
        end
    elseif func == "stage" then
        if args.stage then
            game.stage = args.stage
            game.tank.x = math.floor(GRID_WIDTH / 2)
            game.tank.y = GRID_HEIGHT - 2
            game.tank.direction = "up"
            game.bullets = {}
        end
    elseif func == "score" then
        if args.points then game.score = game.score + args.points end
    elseif func == "bullet" then
        if args.x and args.y and args.dir then
            game.bullets[game.next_bullet_id] = {
                x = args.x, y = args.y, direction = args.dir, owner = args.owner or "remote"
            }
            game.next_bullet_id = game.next_bullet_id + 1
        end
        if args.owner == "enemy" and game.tank.alive then
            if args.x == game.tank.x and args.y == game.tank.y then player_hit() end
        end
        for _, obs in ipairs(game.obstacles) do
            if obs.x == args.x and obs.y == args.y and obs.kind == "brick" then
                destroy_brick(args.x, args.y)
                break
            end
        end
    elseif func == "obstacles" then
        load_obstacles_from_layer()
        sync_game_ui()
    end
end

function cleanup_stale_players()
    local now = os.time()
    local stale = {}
    for did, player in pairs(remote_players) do
        if now - player.last_seen > 3 then table.insert(stale, did) end
    end
    for _, did in ipairs(stale) do remote_players[did] = nil end
end

function on_peer_joined(user_did) broadcast_position() end
function on_peer_left(user_did) remote_players[user_did] = nil; sync_game_ui() end

function game_tick()
    game.fps_frames = game.fps_frames + 1
    local now = os.time()
    if now ~= game.fps_last_time then
        game.fps = game.fps_frames
        game.fps_frames = 0
        game.fps_last_time = now
        ui:set("fps", game.fps)
        cleanup_stale_players()
    end

    game.frame_count = game.frame_count + 1
    if game.frame_count % 6 == 0 then broadcast_position() end

    if game.paused or game.game_over then sync_game_ui(); return end

    if game.shoot_cooldown > 0 then game.shoot_cooldown = game.shoot_cooldown - 1 end

    game.bullet_move_counter = game.bullet_move_counter + 1
    if game.bullet_move_counter >= BULLET_MOVE_FRAMES then
        game.bullet_move_counter = 0
        move_bullets()
    end

    sync_game_ui()
end

function sync_game_ui()
    local players_array = {}
    table.insert(players_array, {
        x = game.tank.x, y = game.tank.y, direction = game.tank.direction,
        alive = game.tank.alive, is_me = true, color_index = color_index_for_user(my_did or ""),
        name = my_name or "You",
    })
    for did, player in pairs(remote_players) do
        table.insert(players_array, {
            x = player.x, y = player.y, direction = player.direction, alive = player.alive,
            is_me = false, color_index = player.color_index, name = player.name,
        })
    end
    ui:set("players", players_array)
    ui:set("tank", { x = game.tank.x, y = game.tank.y, direction = game.tank.direction, alive = game.tank.alive })

    local enemies_array = {}
    for _, enemy in pairs(game.enemies) do
        table.insert(enemies_array, {
            x = enemy.x, y = enemy.y, direction = enemy.direction, hp = enemy.hp, is_boss = enemy.is_boss,
        })
    end
    ui:set("enemies", enemies_array)

    local bullets_array = {}
    for _, bullet in pairs(game.bullets) do
        table.insert(bullets_array, { x = bullet.x, y = bullet.y, direction = bullet.direction })
    end
    ui:set("bullets", bullets_array)

    ui:set("obstacles", game.obstacles)
    ui:set("score", game.score)
    ui:set("lives", game.lives)
    ui:set("stage", game.stage)
    ui:set("game_over", game.game_over)
    ui:set("game_paused", game.paused)
end

function on_click(target)
    if target == "toggle_pause" then
        if not game.game_over then game.paused = not game.paused; sync_game_ui() end
    elseif target == "restart" then
        request_restart()
    end
end

function on_key_pressed(key)
    if game.game_over then return end
    if not game.paused then
        if key == "up" or key == "down" or key == "left" or key == "right" then
            game.tank.direction = key
            local vec = DIR_VECTORS[key]
            local new_x, new_y = game.tank.x + vec.dx, game.tank.y + vec.dy
            if not is_blocked(new_x, new_y) then game.tank.x, game.tank.y = new_x, new_y end
            sync_game_ui()
            broadcast_position()
        end
        if key == "shoot" or key == "space" then
            if game.shoot_cooldown == 0 then
                fire_bullet("player", game.tank.x, game.tank.y, game.tank.direction)
                local vec = DIR_VECTORS[game.tank.direction]
                broadcast_bullet(game.tank.x + vec.dx, game.tank.y + vec.dy, game.tank.direction, my_did)
                game.shoot_cooldown = SHOOT_COOLDOWN_FRAMES
                sync_game_ui()
            end
        end
    end
end

api.export("request_restart", request_restart)
api.export("get_score", function() return game.score end)
api.export("get_stage", function() return game.stage end)
api.export("get_lives", function() return game.lives end)
