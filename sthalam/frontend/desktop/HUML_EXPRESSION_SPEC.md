# HUML Expression Language Specification v0.1.0

## Philosophy

Following HUML's core principles, the expression language prioritizes:

1. **Human-first readability** - Natural language keywords over cryptic symbols
2. **Visual clarity** - Clear operator precedence and structure
3. **Eliminated ambiguity** - Strict rules, one way to express each concept
4. **Singular representation** - Canonical syntax for each operation
5. **Inherent consistency** - Uniform patterns throughout

## Syntax Overview

Expressions are embedded in HUML templates using `{{...}}` delimiters:

```yaml
canPublish: "{{newPostContent is not empty and not isPublishing}}"
postsCount: "{{size of content.posts}}"
```

## Data Types

Expressions evaluate to these types:
- **String**: `"hello"`, `'world'`
- **Number**: `42`, `3.14`, `-10`
- **Boolean**: `true`, `false`
- **Null**: `null`
- **Array**: Accessed via dot notation or brackets
- **Object**: Accessed via dot notation

## Operators

### Comparison (return boolean)
- `==` or `equals` - Equality
- `!=` or `not equals` - Inequality
- `>` or `greater than`
- `<` or `less than`
- `>=` or `at least`
- `<=` or `at most`

**Examples:**
```
age == 18
count greater than 10
price at least 100
```

### Logical (return boolean)
- `and` - Logical AND (no `&&` allowed)
- `or` - Logical OR (no `||` allowed)
- `not` - Logical NOT (no `!` allowed)

**Examples:**
```
isActive and not isDeleted
age >= 18 or hasPermission
not isEmpty
```

### Arithmetic (return number)
- `+` - Addition
- `-` - Subtraction or negation
- `*` - Multiplication
- `/` - Division
- `%` - Modulo

**Examples:**
```
count + 1
total - discount
quantity * price
```

### String Operations
- `+` - Concatenation

**Examples:**
```
"Hello " + name
```

### Ternary Conditional
- `condition ? valueIfTrue : valueIfFalse`

**Examples:**
```
isActive ? "Active" : "Inactive"
count > 0 ? "Has items" : "Empty"
```

## State Checks

Special keyword checks for common patterns:

- `is empty` - Check if string is `""` or array is `[]`
- `is not empty` - Opposite of above
- `in array` - Check if value exists in array

**Examples:**
```
newPostContent is empty
username is not empty
"admin" in roles
postId in likedPosts
```

## Built-in Functions

Functions use natural language syntax: `function of argument`

### Collection Functions
- `size of array` - Returns array length
- `length of string` - Returns string length
- `contains(array, item)` - Check if array contains item
- `first of array` - Get first element
- `last of array` - Get last element

### Type Conversion
- `string(value)` - Convert to string
- `number(value)` - Convert to number
- `boolean(value)` - Convert to boolean

### String Functions
- `uppercase(string)` - Convert to uppercase
- `lowercase(string)` - Convert to lowercase
- `trim(string)` - Remove whitespace

### Math Functions
- `abs(number)` - Absolute value
- `round(number)` - Round to nearest integer
- `floor(number)` - Round down
- `ceil(number)` - Round up
- `min(a, b)` - Minimum value
- `max(a, b)` - Maximum value

**Examples:**
```
size of content.posts
length of newPostContent
contains(roles, "admin")
uppercase(username)
round(price)
```

## Property Access

### Dot Notation
Access object properties:
```
user.name
post.author.email
content.posts
```

### Bracket Notation
Access array elements or computed properties:
```
posts[0]
users[currentIndex]
state[dynamicKey]
```

## Operator Precedence

From highest to lowest:
1. Property access (`.`, `[]`)
2. Function calls
3. Unary operators (`not`, `-`)
4. Multiplicative (`*`, `/`, `%`)
5. Additive (`+`, `-`)
6. Comparison (`>`, `<`, `>=`, `<=`)
7. Equality (`==`, `!=`)
8. Logical AND (`and`)
9. Logical OR (`or`)
10. Ternary (`? :`)

Use parentheses `()` for explicit grouping when needed.

## Context Variables

Expressions have access to:
- **State variables**: `newPostContent`, `isPublishing`, etc.
- **Content data**: `content.posts`, `content.title`
- **Computed values**: Available in same scope
- **Loop variables**: `post`, `comment` (in forEach blocks)
- **User content**: `userContent.preferences`
- **Threads**: `threads['post-1']`

## Whitespace Rules

Following HUML's strict formatting:
- One space after keywords: `not empty`, `size of posts`
- No spaces around dots: `post.id` not `post . id`
- Single space around operators: `a + b` not `a+b`
- No trailing spaces

## Examples

### Simple Conditions
```yaml
# Boolean checks
visible: "{{isLoggedIn}}"
disabled: "{{not canSubmit}}"

# Comparisons
visible: "{{selectedId == post.id}}"
disabled: "{{count <= 0}}"

# Combined logic
canPublish: "{{newPostContent is not empty and not isPublishing}}"
showWarning: "{{age < 18 or not hasConsent}}"
```

### With Functions
```yaml
# Collection operations
postsCount: "{{size of content.posts}}"
hasComments: "{{size of threads[postId] > 0}}"

# String operations
charCount: "{{length of newPostContent}}/280"
displayName: "{{uppercase(username)}}"

# Type checks
isEmpty: "{{content.posts is empty}}"
hasRole: "{{contains(user.roles, 'admin')}}"
```

### Complex Expressions
```yaml
# Ternary
status: "{{isPublishing ? 'Publishing...' : 'Publish'}}"
badge: "{{count > 99 ? '99+' : string(count)}}"

# Nested access
authorName: "{{content.posts[0].author.name}}"
commentCount: "{{size of threads[post.id]}}"

# Arithmetic
total: "{{quantity * price * (1 - discount)}}"
average: "{{sum / count}}"

# Multiple conditions
canEdit: "{{isOwner and not isLocked and hasPermission}}"
showButton: "{{(isAdmin or isOwner) and not isDeleted}}"
```

## Error Handling

- **Undefined variables**: Return `null`
- **Type mismatches**: Coerce when possible, otherwise `null`
- **Division by zero**: Return `Infinity` or `-Infinity`
- **Invalid property access**: Return `null`
- **Function errors**: Log warning, return `null`

## Reserved Keywords

Cannot be used as variable names:
- `and`, `or`, `not`
- `is`, `in`
- `empty`
- `of`
- `true`, `false`, `null`
- `equals`, `greater`, `less`, `than`, `least`, `most`

## Design Rationale

### Why Keywords Over Symbols?
```yaml
# Readable (HUML way)
"{{isActive and not isDeleted}}"

# Cryptic (avoid)
"{{isActive && !isDeleted}}"
```

### Why "size of" Instead of "size()"?
```yaml
# Natural language (HUML way)
"{{size of posts}}"

# Function call syntax (less readable)
"{{size(posts)}}"
```

### Why Strict Spacing?
Following HUML's principle of eliminating ambiguity through consistent formatting.

### Why One Way?
HUML's "singular representation" principle - reduces cognitive load and prevents bikeshedding.

## Future Considerations

Potential additions for v0.2.0:
- Null coalescing: `value ?? default`
- Optional chaining: `user?.address?.city`
- Array methods: `filter`, `map`, `reduce`
- String interpolation: Template literals
- Date/time functions

## Implementation Notes

The evaluator is implemented in TypeScript with:
1. **Tokenizer**: Lexical analysis into tokens
2. **Parser**: Builds Abstract Syntax Tree (AST)
3. **Evaluator**: Executes AST with context

This ensures predictable behavior and helpful error messages.
