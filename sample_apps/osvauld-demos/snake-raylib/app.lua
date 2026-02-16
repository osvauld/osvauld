-- Snake Game - Raylib Version
-- Classic snake with immediate-mode rendering

-- Grid settings
local GRID_SIZE = 20
local CELL_SIZE = 20
local MOVE_INTERVAL = 0.1  -- Move every 100ms (10 moves/sec)

-- Colors (hex strings)
local COLORS = {
    bg = "#0f172a",
    grid = "#1e293b",
    grid_line = "#475569",
    snake_head = "#22c55e",
    snake_body = "#16a34a",
    food = "#ef4444",
    text = "#f1f5f9",
    text_dim = "#94a3b8",
    overlay = "#00000080",
}

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
    move_timer = 0,
}

-- Initialize the game
function on_init()
    reset_game()
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
    game.move_timer = 0

    spawn_food()
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
end

-- Update game state (called every frame)
function update(dt)
    -- Handle input
    handle_input()

    if game.paused or game.game_over then
        return
    end

    -- Accumulate time
    game.move_timer = game.move_timer + dt

    -- Only move when interval elapsed
    if game.move_timer < MOVE_INTERVAL then
        return
    end
    game.move_timer = game.move_timer - MOVE_INTERVAL

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

    -- Wrap around walls
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
end

-- Handle keyboard input
function handle_input()
    -- Direction keys (prevent 180-degree turns)
    if input:key_down("up") and game.direction ~= "down" then
        game.next_direction = "up"
    elseif input:key_down("down") and game.direction ~= "up" then
        game.next_direction = "down"
    elseif input:key_down("left") and game.direction ~= "right" then
        game.next_direction = "left"
    elseif input:key_down("right") and game.direction ~= "left" then
        game.next_direction = "right"
    end

    -- Space to toggle pause
    if input:key_down("space") then
        if not game.space_was_down then
            game.space_was_down = true
            if game.game_over then
                reset_game()
            else
                game.paused = not game.paused
            end
        end
    else
        game.space_was_down = false
    end

    -- R to restart
    if input:key_down("r") then
        if not game.r_was_down then
            game.r_was_down = true
            reset_game()
        end
    else
        game.r_was_down = false
    end
end

-- End the game
function end_game()
    game.game_over = true
    game.paused = true

    if game.score > game.high_score then
        game.high_score = game.score
    end
end

-- Draw frame (called every frame)
function draw()
    -- Clear background
    canvas:clear(COLORS.bg)

    -- Calculate offsets for centering
    local grid_width = GRID_SIZE * CELL_SIZE
    local grid_height = GRID_SIZE * CELL_SIZE
    local offset_x = 10
    local offset_y = 40

    -- Draw header
    canvas:text(10, 10, "Snake Game", 20, COLORS.text)
    canvas:text(200, 12, "Score: " .. game.score, 16, COLORS.snake_head)
    canvas:text(350, 12, "High: " .. game.high_score, 14, COLORS.text_dim)

    -- Draw grid background
    canvas:rect(offset_x, offset_y, grid_width, grid_height, COLORS.grid)

    -- Draw grid lines (subtle)
    for i = 0, GRID_SIZE do
        local x = offset_x + i * CELL_SIZE
        local y = offset_y + i * CELL_SIZE
        -- Vertical line
        canvas:line(x, offset_y, x, offset_y + grid_height, COLORS.grid_line)
        -- Horizontal line
        canvas:line(offset_x, y, offset_x + grid_width, y, COLORS.grid_line)
    end

    -- Draw food
    local food_x = offset_x + game.food.x * CELL_SIZE + CELL_SIZE / 2
    local food_y = offset_y + game.food.y * CELL_SIZE + CELL_SIZE / 2
    canvas:circle(food_x, food_y, CELL_SIZE / 2 - 2, COLORS.food)

    -- Draw snake
    for i, segment in ipairs(game.snake) do
        local x = offset_x + segment.x * CELL_SIZE + 1
        local y = offset_y + segment.y * CELL_SIZE + 1
        local color = (i == 1) and COLORS.snake_head or COLORS.snake_body
        canvas:rect(x, y, CELL_SIZE - 2, CELL_SIZE - 2, color)
    end

    -- Draw overlays
    if game.game_over then
        -- Game over overlay
        canvas:rect(offset_x, offset_y, grid_width, grid_height, COLORS.overlay)
        canvas:text(offset_x + grid_width/2 - 70, offset_y + grid_height/2 - 30, "Game Over", 28, COLORS.food)
        canvas:text(offset_x + grid_width/2 - 50, offset_y + grid_height/2 + 10, "Score: " .. game.score, 20, COLORS.text)
        canvas:text(offset_x + grid_width/2 - 80, offset_y + grid_height/2 + 50, "Press SPACE to restart", 14, COLORS.text_dim)
    elseif game.paused then
        -- Paused overlay
        canvas:rect(offset_x, offset_y, grid_width, grid_height, COLORS.overlay)
        canvas:text(offset_x + grid_width/2 - 45, offset_y + grid_height/2 - 20, "Paused", 24, COLORS.text)
        canvas:text(offset_x + grid_width/2 - 80, offset_y + grid_height/2 + 20, "Press SPACE to start", 14, COLORS.text_dim)
        canvas:text(offset_x + grid_width/2 - 60, offset_y + grid_height/2 + 45, "WASD to move", 12, COLORS.text_dim)
    end
end
