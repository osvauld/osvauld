-- Snake Game App Logic
-- Classic snake with Loro-synced high scores

-- Grid settings
local GRID_SIZE = 20
local CELL_SIZE = 20
local MOVE_EVERY_N_FRAMES = 5  -- Move snake every N frames (~12 moves/sec at 60fps)

-- Game state
local game = {
    snake = {},        -- Array of {x, y} segments, head is first
    food = {x = 10, y = 10},
    direction = "right",
    next_direction = "right",
    score = 0,
    high_score = 0,
    game_over = false,
    paused = true,
    frame_count = 0,   -- Frame counter for movement timing
    fps_frames = 0,    -- Frames in current second
    fps_last_time = 0, -- Last FPS calculation time
    fps = 0,           -- Current FPS
}

-- Loro layers
local page_id = nil
local scores_layer = nil

-- Initialize the app
function on_init()
    page_id = permit:page_id()

    -- Scores layer (synced across peers)
    scores_layer = loro:get_or_create_layer(page_id .. "/scores", "list")

    -- Initialize game
    reset_game()

    -- Load high scores
    refresh_leaderboard()

    -- Set grid settings
    ui:set("grid_size", GRID_SIZE)
    ui:set("cell_size", CELL_SIZE)
end

-- Reset game to initial state
function reset_game()
    -- Start snake in center
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
    game.last_tick = 0

    -- Spawn food
    spawn_food()

    -- Update UI
    sync_game_ui()
end

-- Spawn food at random location (not on snake)
function spawn_food()
    local valid = false
    local x, y

    while not valid do
        x = math.random(0, GRID_SIZE - 1)
        y = math.random(0, GRID_SIZE - 1)
        valid = true

        -- Check not on snake
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

    -- Only move snake every N frames
    game.frame_count = game.frame_count + 1
    if game.frame_count < MOVE_EVERY_N_FRAMES then
        return
    end
    game.frame_count = 0

    -- Apply direction change
    game.direction = game.next_direction

    -- Calculate new head position
    local head = game.snake[1]
    local new_head = {x = head.x, y = head.y}

    if game.direction == "up" then
        new_head.y = new_head.y - 1
    elseif game.direction == "down" then
        new_head.y = new_head.y + 1
    elseif game.direction == "left" then
        new_head.x = new_head.x - 1
    elseif game.direction == "right" then
        new_head.x = new_head.x + 1
    end

    -- Wrap around walls (come out the other side)
    if new_head.x < 0 then
        new_head.x = GRID_SIZE - 1
    elseif new_head.x >= GRID_SIZE then
        new_head.x = 0
    end
    if new_head.y < 0 then
        new_head.y = GRID_SIZE - 1
    elseif new_head.y >= GRID_SIZE then
        new_head.y = 0
    end

    -- Check self collision
    for _, segment in ipairs(game.snake) do
        if segment.x == new_head.x and segment.y == new_head.y then
            end_game()
            return
        end
    end

    -- Move snake
    table.insert(game.snake, 1, new_head)

    -- Check food collision
    if new_head.x == game.food.x and new_head.y == game.food.y then
        -- Eat food - don't remove tail (snake grows)
        game.score = game.score + 10
        spawn_food()
    else
        -- Remove tail
        table.remove(game.snake)
    end

    -- Update UI
    sync_game_ui()
end

-- End the game
function end_game()
    game.game_over = true
    game.paused = true

    -- Update high score if needed
    if game.score > game.high_score then
        game.high_score = game.score
    end

    -- Save score to Loro if > 0
    if game.score > 0 then
        save_score()
    end

    sync_game_ui()
end

-- Save score to Loro
function save_score()
    local my_did = permit:my_did()
    local short_did = my_did:sub(-8) -- Last 8 chars

    local entry = {
        player = short_did,
        score = game.score,
        timestamp = os.time(),
        date = os.date("%Y-%m-%d")
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
            date = entry.date or ""
        })

        -- Update high score
        if i == 1 and entry.score > game.high_score then
            game.high_score = entry.score
        end
    end

    ui:set("leaderboard", leaderboard)
    ui:set("high_score", game.high_score)
end

-- Sync game state to UI
function sync_game_ui()
    ui:set("snake", game.snake)
    ui:set("score", game.score)
    ui:set("high_score", game.high_score)
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
    -- Prevent 180-degree turns
    if key == "up" and game.direction ~= "down" then
        game.next_direction = "up"
    elseif key == "down" and game.direction ~= "up" then
        game.next_direction = "down"
    elseif key == "left" and game.direction ~= "right" then
        game.next_direction = "left"
    elseif key == "right" and game.direction ~= "left" then
        game.next_direction = "right"
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
api.export("get_snake_length", function() return #game.snake end)
