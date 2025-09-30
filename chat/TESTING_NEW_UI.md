# Testing the New WhatsApp-Style UI

## Quick Start

### 1. Build and Run the Application

```bash
cd /home/abe/osvauld/chat/src-tauri
cargo build --release
```

Then run the application:
```bash
cargo run --release
```

Alternatively, if you have the frontend dev server setup:
```bash
cd /home/abe/osvauld/chat/frontend/desktop
pnpm dev
```

### 2. What to Test

#### Initial Load
- [ ] App loads with split-view layout
- [ ] Chat list appears on the left (400px width)
- [ ] Empty state shows on the right with chat icon and message
- [ ] "Chats" header visible with search box and + button

#### Chat List Functionality
- [ ] All existing chats appear in the left panel
- [ ] Each chat shows:
  - Online/offline indicator (green or gray dot)
  - Participant name
  - Last message preview
  - Timestamp (formatted as time or date)
  - Unread count (if any)
- [ ] Hover over a chat shows visual feedback
- [ ] Search bar is functional
- [ ] "+ New Chat" button works

#### Selecting a Chat
- [ ] Click on a chat in the list
- [ ] Selected chat gets lavender border highlight
- [ ] Right panel immediately shows the chat workspace
- [ ] Chat header displays:
  - Avatar circle with first letter of name
  - Participant name
  - Online/offline status
- [ ] Previous messages load and display correctly
- [ ] Messages auto-scroll to the bottom

#### Message Display
- [ ] Your messages appear on the right (lavender background)
- [ ] Other person's messages appear on the left (dark frame)
- [ ] Message bubbles have directional tails
- [ ] No author names shown (position indicates sender)
- [ ] Timestamps visible on each message
- [ ] Messages are readable with good contrast

#### Sending Messages
- [ ] Message input field at bottom is visible
- [ ] Type a message in the input
- [ ] Press Enter to send (without Shift)
- [ ] Message appears immediately in the chat
- [ ] Input clears after sending
- [ ] Chat auto-scrolls to show new message
- [ ] Send button (circular with icon) works on click
- [ ] Send button is disabled when input is empty

#### Switching Between Chats
- [ ] Click different chat in the list
- [ ] Right panel updates to show selected chat
- [ ] Previous chat deselects (border returns to normal)
- [ ] New chat gets lavender border
- [ ] Messages load for new chat
- [ ] Auto-scrolls to bottom of new chat
- [ ] Left panel remains visible throughout
- [ ] No "Back" button needed

#### Receiving Messages
- [ ] New messages appear in real-time
- [ ] Chat auto-scrolls to show new messages
- [ ] Unread count updates in chat list (if implemented)
- [ ] Last message preview updates in chat list
- [ ] Timestamp updates

#### Edge Cases
- [ ] What happens when no chats exist?
- [ ] Does empty state show correctly?
- [ ] Can you create a new chat?
- [ ] What happens with very long messages?
- [ ] What happens with many messages (scrolling)?
- [ ] What happens with long participant names?
- [ ] Does the layout work on different window sizes?

## Expected Behavior

### Layout
```
┌────────────────────────────────────────────┐
│              Header Bar                    │
├──────────┬─────────────────────────────────┤
│ Chats    │  Selected Chat / Empty State    │
│ [Search] │                                 │
│          │                                 │
│ Chat 1 ✓ │        Chat messages or         │
│ Chat 2   │        empty state here         │
│ Chat 3   │                                 │
│          │                                 │
└──────────┴─────────────────────────────────┘
```

### Visual Indicators
- **Selected chat**: Lavender border on left side
- **Online**: Green dot next to name
- **Offline**: Gray dot next to name
- **Your messages**: Right-aligned, lavender background
- **Their messages**: Left-aligned, dark background with border

### Interactions
1. **Click chat** → Opens on right, highlights on left
2. **Type + Enter** → Sends message, clears input, scrolls to bottom
3. **Click Send button** → Same as pressing Enter
4. **Switch chats** → Immediate transition, no view change
5. **Receive message** → Appears in chat, auto-scrolls

## Common Issues to Check

### If chat list doesn't show
- Check if data is loading (`isDataLoading` state)
- Verify `fetchChats()` is being called
- Check console for errors

### If messages don't appear
- Verify chat coordinator is initialized
- Check if `currentChatMessages` is populated
- Look for loading errors in console

### If auto-scroll doesn't work
- Check if `messagesContainer` ref is bound correctly
- Verify `$effect` is triggering
- Check console for errors in `scrollToBottom()`

### If styling looks wrong
- Verify Tailwind CSS is loading
- Check if custom colors are defined
- Inspect elements to see applied styles

## Performance Checklist

- [ ] Chat list scrolls smoothly with many chats
- [ ] Message list scrolls smoothly with many messages
- [ ] Switching chats is instant
- [ ] Sending messages is responsive
- [ ] No lag when typing in input
- [ ] No memory leaks after extended use

## Accessibility Checklist

- [ ] Can navigate with keyboard
- [ ] Tab order makes sense
- [ ] Focus indicators are visible
- [ ] Button hover states work
- [ ] Text is readable (contrast)
- [ ] Interactive elements have appropriate cursor

## Browser/Platform Testing

- [ ] Works on Linux (your platform)
- [ ] Works at different window sizes
- [ ] Responsive to window resizing
- [ ] Works with dark mode (if applicable)

## Known Limitations

1. **Fixed width**: Chat list is fixed at 400px (no resize drag)
2. **No mobile view**: Layout is desktop-optimized
3. **No breakpoints**: Doesn't collapse on small screens
4. **Single chat only**: No multi-select or bulk actions

## Next Steps After Testing

If everything works well:
1. Consider adding resize handle for chat list width
2. Add responsive breakpoints for smaller screens
3. Implement typing indicators
4. Add message reactions
5. Implement read receipts
6. Add chat settings/actions menu

## Reporting Issues

If you find issues, please note:
- What you were doing
- What you expected to happen
- What actually happened
- Browser/OS version
- Console errors (if any)
- Screenshots (if visual issue)
