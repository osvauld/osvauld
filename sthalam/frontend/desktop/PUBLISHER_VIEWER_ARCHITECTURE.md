# Publisher/Viewer Architecture Documentation

## Overview
This document tracks the implementation of the Publisher/Viewer architecture for the HUML template system.

## Key Architecture Decisions

### 1. Mode Separation
- **Publisher** and **Viewer** are completely separate modes
- No cross-reactivity between Publisher and Viewer components
- When switching modes, we load the full state fresh
- Each mode operates independently with its own UI state

### 2. Document Structure

```
templateDoc/
├── metadata (Map)
│   ├── hasPublisher: boolean
│   └── hasViewer: boolean
├── publisherState (Map) - Initial state for publisher
├── publisherComputed (Map) - Computed expressions for publisher
├── publisherScreens (Tree) - UI structure for publisher
├── viewerState (Map) - Initial state for viewer
├── viewerComputed (Map) - Computed expressions for viewer
└── viewerScreens (Tree) - UI structure for viewer

contentDoc (Map) - Shared persistent content
├── posts: []
├── title: string
└── [other shared data]

userContentDoc (Map) - Per-user persistent state
└── [user-specific data]

uiStateDoc (Tree) - Session-only UI state
└── [temporary UI state]

collaborativeDoc (Map of Trees) - Comments/threads
└── [thread-id]: Tree
```

### 3. Data Access Patterns

#### Publisher Mode:
- **Read/Write**: `contentDoc` (can modify shared content)
- **Read**: `publisherState`, `publisherComputed`, `publisherScreens`
- **Session State**: Own UI state (not persisted)
- **Cannot**: Access viewer comments/threads directly

#### Viewer Mode:
- **Read-only**: `contentDoc` (cannot modify shared content)
- **Read**: `viewerState`, `viewerComputed`, `viewerScreens`
- **Read/Write**: `collaborativeDoc` (can add comments)
- **Read/Write**: `userContentDoc` (personal data like likes)
- **Session State**: Own UI state (not persisted)

## Implementation Status

### ✅ Completed

1. **Template Structure** (`social_feed_v2.huml`)
   - Separate `publisher::` and `viewer::` sections
   - Each with own state, computed, screens

2. **Template Importer** (`templateImporter.ts`)
   - `importPublisherTemplate()` - Loads publisher sections
   - `importViewerTemplate()` - Loads viewer sections
   - Stores in separate maps/trees in templateDoc

3. **Content Store** (`contentStore.ts`)
   - Wraps `contentDoc` for content management
   - Publisher methods: `publishPost()`, `updatePost()`, `deletePost()`
   - Viewer methods: `incrementLikes()` (limited write)
   - Read methods: `getPosts()`, `getPost()`, `getContent()`

4. **Publisher App** (`PublisherApp.svelte`)
   - Loads from `publisherScreens` tree
   - Initializes state from `publisherState` map
   - Evaluates `publisherComputed` expressions
   - Handles actions: publishPost, updatePost, deletePost, setState
   - No cross-reactivity with Viewer

5. **HUML Syntax Highlighting** (`huml.ts`)
   - Highlights `publisher::` and `viewer::` as keywords
   - Visual distinction in code editor

### 🚧 In Progress

#### Publisher UI Implementation
- [ ] Form handling with state binding
- [ ] Expression evaluation in templates
- [ ] Navigation between publisher screens
- [ ] Real-time preview of changes

### 📋 TODO

#### Viewer App Implementation
- [ ] Create `ViewerApp.svelte` component
- [ ] Load from `viewerScreens` tree
- [ ] Read-only content access
- [ ] Thread/comment functionality
- [ ] User-specific state (likes, preferences)

#### Mode Switching
- [ ] Create mode switcher component
- [ ] Clean unmount of current mode
- [ ] Fresh mount of new mode
- [ ] No state carryover between modes

#### Integration
- [ ] Update `WebsiteBuilder.svelte` to use PublisherApp/ViewerApp
- [ ] Mode toggle in UI
- [ ] Test with social_feed_v2.huml template

## Architecture Notes

### Why No Cross-Reactivity?
1. **Simplicity**: Each mode is self-contained
2. **Performance**: No unnecessary subscriptions
3. **Clear Boundaries**: Publisher can't accidentally affect Viewer UI
4. **Fresh State**: Mode switch = fresh start, no stale state

### Mode Switching Strategy
```javascript
// When switching from Publisher to Viewer:
1. Unmount PublisherApp completely
2. Save any pending changes to contentDoc
3. Mount ViewerApp fresh
4. ViewerApp loads all data from Loro documents
5. No state transfer between modes
```

### Expression Context Hierarchy
```javascript
// Publisher context:
{
  ...publisherUIState,    // Session state
  content: {...},         // From contentDoc
  ...publisherComputed,   // Evaluated expressions
  mode: 'publisher'       // Mode indicator
}

// Viewer context:
{
  ...viewerUIState,       // Session state
  content: {...},         // From contentDoc (read-only)
  userContent: {...},     // From userContentDoc
  threads: {...},         // From collaborativeDoc
  ...viewerComputed,      // Evaluated expressions
  mode: 'viewer'          // Mode indicator
}
```

## Action Handlers

### Publisher Actions
- `publishPost` - Creates new content entry
- `updatePost` - Modifies existing content
- `deletePost` - Removes content
- `setState` - Updates session UI state
- `navigate` - Switch between publisher screens

### Viewer Actions (TODO)
- `addComment` - Add to thread (collaborativeDoc)
- `likePost` - Update user preference (userContentDoc)
- `setState` - Updates session UI state
- `navigate` - Switch between viewer screens

## Testing Strategy

1. **Publisher Mode Tests**:
   - Create post → Verify in contentDoc
   - Update post → Verify changes persist
   - Delete post → Verify removal
   - UI state changes → Verify don't persist on reload

2. **Viewer Mode Tests**:
   - View content → Verify read-only
   - Add comment → Verify in collaborativeDoc
   - Like post → Verify in userContentDoc
   - Cannot modify contentDoc

3. **Mode Switching Tests**:
   - Publisher → Viewer → No state carryover
   - Viewer → Publisher → Fresh publisher state
   - Content persists across mode switches

## Future Enhancements

1. **Analytics Dashboard** (Publisher)
   - View metrics on post engagement
   - Comment statistics
   - User interaction patterns

2. **Moderation Tools** (Publisher)
   - Hide/show comments
   - Ban users
   - Content filtering

3. **User Preferences** (Viewer)
   - Theme selection
   - Notification settings
   - Content filtering preferences

4. **Real-time Collaboration**
   - Live content updates
   - Presence indicators
   - Collaborative editing (publisher team)

## References

- [HUML Template Guide V3](./HUML_TEMPLATE_GUIDE_V3.md)
- [Social Feed Template](./social_feed_v2.huml)
- [Loro Documentation](https://loro.dev)