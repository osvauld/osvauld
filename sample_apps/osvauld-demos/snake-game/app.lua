-- Snake Game App Logic
-- Classic snake with Loro-synced high scores
-- Uses scribe:bind() for declarative layer-UI sync

-- Grid settings
local GRID_SIZE = 20
local CELL_SIZE = 20
local MOVE_EVERY_N_FRAMES = 5

-- Game state
local game = {
    snake = {},
    food = {x = 10, y = 10},
    direction = "right",
    next_direction = "right",
    score = 0,
    high_score = 0,
    game_over = false,
    paused = true,
    frame_count = 0,
    fps_frames = 0,
    fps_last_time = 0,
    fps = 0,
}

local page_id = nil
local scores_layer = nil

function on_init()
    page_id = scribe:page_id()
    scores_layer = scribe:list(page_id .. "/scores")

    -- Declarative binding: leaderboard auto-syncs to UI
    -- NOTE: Sorting by score is done in Slint UI (enables surgical updates)
    scribe:bind("leaderboard", "scores", {
        key = "player",  -- Use player as stable identity
        transform = function(entry)
            return {
                player = entry.player or "???",
                score = entry.score or 0,
                date = entry.date or ""
            }
        end
    })

    reset_game()
    ui:set("grid_size", GRID_SIZE)
    ui:set("cell_size", CELL_SIZE)
    timer.setInterval(16, game_tick)
end

function reset_game()
    local center = math.floor(GRID_SIZE / 2)
    game.snake = {
        {x = center, y = center},
        {x = center - 1, y = center},
        {x = center - 2, y = center},
    }
    game.direction = "right"
    game.next_direction = "right"
    game.score = 0
    game.game_over = false
    game.paused = true
    spawn_food()
    sync_game_ui()
end

function spawn_food()
    local valid = false
    local x, y
    while not valid do
        x = math.random(0, GRID_SIZE - 1)
        y = math.random(0, GRID_SIZE - 1)
        valid = true
        for _, segment in ipairs(game.snake) do
            if segment.x == x and segment.y == y then
                valid = false
                break
            end
        end
    end
    game.food = {x = x, y = y}
    ui:set("food_x", x)
    ui:set("food_y", y)
end

function game_tick()
    game.fps_frames = game.fps_frames + 1
    local now = os.time()
    if now ~= game.fps_last_time then
        game.fps = game.fps_frames
        game.fps_frames = 0
        game.fps_last_time = now
        ui:set("fps", game.fps)
    end

    if game.paused or game.game_over then return end

    game.frame_count = game.frame_count + 1
    if game.frame_count < MOVE_EVERY_N_FRAMES then return end
    game.frame_count = 0

    game.direction = game.next_direction

    local head = game.snake[1]
    local new_head = {x = head.x, y = head.y}

    if game.direction == "up" then new_head.y = new_head.y - 1
    elseif game.direction == "down" then new_head.y = new_head.y + 1
    elseif game.direction == "left" then new_head.x = new_head.x - 1
    elseif game.direction == "right" then new_head.x = new_head.x + 1 end

    if new_head.x < 0 then new_head.x = GRID_SIZE - 1
    elseif new_head.x >= GRID_SIZE then new_head.x = 0 end
    if new_head.y < 0 then new_head.y = GRID_SIZE - 1
    elseif new_head.y >= GRID_SIZE then new_head.y = 0 end

    for _, segment in ipairs(game.snake) do
        if segment.x == new_head.x and segment.y == new_head.y then
            end_game()
            return
        end
    end

    table.insert(game.snake, 1, new_head)

    if new_head.x == game.food.x and new_head.y == game.food.y then
        game.score = game.score + 10
        spawn_food()
    else
        table.remove(game.snake)
    end

    sync_game_ui()
end

function end_game()
    game.game_over = true
    game.paused = true
    if game.score > game.high_score then game.high_score = game.score end
    if game.score > 0 then save_score() end
    sync_game_ui()
end

function save_score()
    local my_did = scribe:my_did()
    local entry = {
        player = my_did:sub(-8),
        score = game.score,
        timestamp = os.time(),
        date = os.date("%Y-%m-%d")
    }
    scores_layer:push(entry)
end

function sync_game_ui()
    ui:set("snake", game.snake)
    ui:set("score", game.score)
    ui:set("high_score", game.high_score)
    ui:set("game_over", game.game_over)
    ui:set("game_paused", game.paused)
end

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

function on_key_pressed(key)
    if key == "up" and game.direction ~= "down" then game.next_direction = "up"
    elseif key == "down" and game.direction ~= "up" then game.next_direction = "down"
    elseif key == "left" and game.direction ~= "right" then game.next_direction = "left"
    elseif key == "right" and game.direction ~= "left" then game.next_direction = "right" end
end

api.export("reset_game", reset_game)
api.export("get_score", function() return game.score end)
api.export("get_snake_length", function() return #game.snake end)
