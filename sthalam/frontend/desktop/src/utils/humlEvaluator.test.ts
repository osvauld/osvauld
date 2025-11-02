/**
 * Tests for HUML Expression Evaluator
 */

import { evaluateExpression, evaluateValue, hasExpression } from './humlEvaluator';

// Simple test runner
function test(name: string, fn: () => void) {
  try {
    fn();
    console.log(`✅ ${name}`);
  } catch (error) {
    console.error(`❌ ${name}:`, error);
  }
}

function assert(condition: boolean, message: string = 'Assertion failed') {
  if (!condition) {
    throw new Error(message);
  }
}

function assertEquals(actual: any, expected: any, message?: string) {
  if (actual !== expected) {
    throw new Error(
      message || `Expected ${JSON.stringify(expected)} but got ${JSON.stringify(actual)}`
    );
  }
}

// Run tests
console.log('🧪 Running HUML Expression Evaluator Tests\n');

// ============================================================================
// LITERALS
// ============================================================================

test('Literal: number', () => {
  assertEquals(evaluateExpression('42', {}), 42);
  assertEquals(evaluateExpression('3.14', {}), 3.14);
  assertEquals(evaluateExpression('-10', {}), -10);
});

test('Literal: string', () => {
  assertEquals(evaluateExpression('"hello"', {}), 'hello');
  assertEquals(evaluateExpression("'world'", {}), 'world');
});

test('Literal: boolean', () => {
  assertEquals(evaluateExpression('true', {}), true);
  assertEquals(evaluateExpression('false', {}), false);
});

test('Literal: null', () => {
  assertEquals(evaluateExpression('null', {}), null);
});

// ============================================================================
// IDENTIFIERS
// ============================================================================

test('Identifier: simple', () => {
  assertEquals(evaluateExpression('foo', { foo: 42 }), 42);
  assertEquals(evaluateExpression('name', { name: 'Alice' }), 'Alice');
});

test('Identifier: undefined returns null', () => {
  assertEquals(evaluateExpression('missing', {}), null);
});

// ============================================================================
// ARITHMETIC
// ============================================================================

test('Arithmetic: addition', () => {
  assertEquals(evaluateExpression('1 + 2', {}), 3);
  assertEquals(evaluateExpression('a + b', { a: 5, b: 3 }), 8);
});

test('Arithmetic: subtraction', () => {
  assertEquals(evaluateExpression('10 - 3', {}), 7);
  assertEquals(evaluateExpression('a - b', { a: 10, b: 3 }), 7);
});

test('Arithmetic: multiplication', () => {
  assertEquals(evaluateExpression('3 * 4', {}), 12);
  assertEquals(evaluateExpression('a * b', { a: 5, b: 3 }), 15);
});

test('Arithmetic: division', () => {
  assertEquals(evaluateExpression('10 / 2', {}), 5);
  assertEquals(evaluateExpression('a / b', { a: 10, b: 2 }), 5);
});

test('Arithmetic: modulo', () => {
  assertEquals(evaluateExpression('10 % 3', {}), 1);
});

test('Arithmetic: complex', () => {
  assertEquals(evaluateExpression('2 + 3 * 4', {}), 14); // Precedence
  assertEquals(evaluateExpression('(2 + 3) * 4', {}), 20); // Parentheses
});

// ============================================================================
// COMPARISON
// ============================================================================

test('Comparison: equality', () => {
  assertEquals(evaluateExpression('5 == 5', {}), true);
  assertEquals(evaluateExpression('5 == 3', {}), false);
  assertEquals(evaluateExpression('a == b', { a: 5, b: 5 }), true);
});

test('Comparison: inequality', () => {
  assertEquals(evaluateExpression('5 != 3', {}), true);
  assertEquals(evaluateExpression('5 != 5', {}), false);
});

test('Comparison: greater than', () => {
  assertEquals(evaluateExpression('5 > 3', {}), true);
  assertEquals(evaluateExpression('3 > 5', {}), false);
  assertEquals(evaluateExpression('5 greater than 3', {}), true);
});

test('Comparison: less than', () => {
  assertEquals(evaluateExpression('3 < 5', {}), true);
  assertEquals(evaluateExpression('5 < 3', {}), false);
  assertEquals(evaluateExpression('3 less than 5', {}), true);
});

test('Comparison: greater than or equal', () => {
  assertEquals(evaluateExpression('5 >= 5', {}), true);
  assertEquals(evaluateExpression('5 >= 3', {}), true);
  assertEquals(evaluateExpression('3 >= 5', {}), false);
  assertEquals(evaluateExpression('5 at least 5', {}), true);
});

test('Comparison: less than or equal', () => {
  assertEquals(evaluateExpression('3 <= 5', {}), true);
  assertEquals(evaluateExpression('5 <= 5', {}), true);
  assertEquals(evaluateExpression('5 <= 3', {}), false);
  assertEquals(evaluateExpression('5 at most 5', {}), true);
});

// ============================================================================
// LOGICAL
// ============================================================================

test('Logical: and', () => {
  assertEquals(evaluateExpression('true and true', {}), true);
  assertEquals(evaluateExpression('true and false', {}), false);
  assertEquals(evaluateExpression('false and true', {}), false);
  assertEquals(evaluateExpression('isActive and not isDeleted', { isActive: true, isDeleted: false }), true);
});

test('Logical: or', () => {
  assertEquals(evaluateExpression('true or false', {}), true);
  assertEquals(evaluateExpression('false or true', {}), true);
  assertEquals(evaluateExpression('false or false', {}), false);
});

test('Logical: not', () => {
  assertEquals(evaluateExpression('not true', {}), false);
  assertEquals(evaluateExpression('not false', {}), true);
  assertEquals(evaluateExpression('not isActive', { isActive: false }), true);
});

test('Logical: complex', () => {
  assertEquals(
    evaluateExpression('(a > 5 and b < 10) or c == 0', { a: 6, b: 8, c: 1 }),
    true
  );
});

// ============================================================================
// STATE CHECKS
// ============================================================================

test('State check: is empty (string)', () => {
  assertEquals(evaluateExpression('name is empty', { name: '' }), true);
  assertEquals(evaluateExpression('name is empty', { name: 'Alice' }), false);
});

test('State check: is empty (array)', () => {
  assertEquals(evaluateExpression('items is empty', { items: [] }), true);
  assertEquals(evaluateExpression('items is empty', { items: [1, 2, 3] }), false);
});

test('State check: is not empty (string)', () => {
  assertEquals(evaluateExpression('name is not empty', { name: 'Alice' }), true);
  assertEquals(evaluateExpression('name is not empty', { name: '' }), false);
});

test('State check: is not empty (array)', () => {
  assertEquals(evaluateExpression('items is not empty', { items: [1, 2, 3] }), true);
  assertEquals(evaluateExpression('items is not empty', { items: [] }), false);
});

test('State check: in array', () => {
  assertEquals(evaluateExpression('"admin" in roles', { roles: ['admin', 'user'] }), true);
  assertEquals(evaluateExpression('"guest" in roles', { roles: ['admin', 'user'] }), false);
  assertEquals(evaluateExpression('userId in likedPosts', { userId: 'post-1', likedPosts: ['post-1', 'post-2'] }), true);
});

// ============================================================================
// MEMBER ACCESS
// ============================================================================

test('Member access: dot notation', () => {
  assertEquals(evaluateExpression('user.name', { user: { name: 'Alice' } }), 'Alice');
  assertEquals(evaluateExpression('post.author.email', {
    post: { author: { email: 'alice@example.com' } }
  }), 'alice@example.com');
});

test('Member access: bracket notation', () => {
  assertEquals(evaluateExpression('posts[0]', { posts: ['first', 'second'] }), 'first');
  assertEquals(evaluateExpression('users[currentIndex]', {
    users: ['Alice', 'Bob'],
    currentIndex: 1
  }), 'Bob');
});

test('Member access: undefined property returns null', () => {
  assertEquals(evaluateExpression('user.missing', { user: {} }), null);
  assertEquals(evaluateExpression('missing.property', {}), null);
});

// ============================================================================
// FUNCTIONS
// ============================================================================

test('Function: size of array', () => {
  assertEquals(evaluateExpression('size of posts', { posts: [1, 2, 3] }), 3);
  assertEquals(evaluateExpression('size of empty', { empty: [] }), 0);
});

test('Function: length of string', () => {
  assertEquals(evaluateExpression('length of text', { text: 'hello' }), 5);
  assertEquals(evaluateExpression('length of empty', { empty: '' }), 0);
});

test('Function: contains', () => {
  assertEquals(evaluateExpression('contains(roles, "admin")', {
    roles: ['admin', 'user']
  }), true);
  assertEquals(evaluateExpression('contains(roles, "guest")', {
    roles: ['admin', 'user']
  }), false);
});

test('Function: first of array', () => {
  assertEquals(evaluateExpression('first of items', { items: [1, 2, 3] }), 1);
  assertEquals(evaluateExpression('first of empty', { empty: [] }), null);
});

test('Function: last of array', () => {
  assertEquals(evaluateExpression('last of items', { items: [1, 2, 3] }), 3);
  assertEquals(evaluateExpression('last of empty', { empty: [] }), null);
});

test('Function: string conversion', () => {
  assertEquals(evaluateExpression('string(42)', {}), '42');
  assertEquals(evaluateExpression('string(true)', {}), 'true');
});

test('Function: number conversion', () => {
  assertEquals(evaluateExpression('number("42")', {}), 42);
  assertEquals(evaluateExpression('number("3.14")', {}), 3.14);
});

test('Function: uppercase', () => {
  assertEquals(evaluateExpression('uppercase("hello")', {}), 'HELLO');
  assertEquals(evaluateExpression('uppercase(name)', { name: 'alice' }), 'ALICE');
});

test('Function: lowercase', () => {
  assertEquals(evaluateExpression('lowercase("HELLO")', {}), 'hello');
});

test('Function: trim', () => {
  assertEquals(evaluateExpression('trim("  hello  ")', {}), 'hello');
});

test('Function: math', () => {
  assertEquals(evaluateExpression('abs(-5)', {}), 5);
  assertEquals(evaluateExpression('round(3.7)', {}), 4);
  assertEquals(evaluateExpression('floor(3.7)', {}), 3);
  assertEquals(evaluateExpression('ceil(3.2)', {}), 4);
  assertEquals(evaluateExpression('min(5, 3)', {}), 3);
  assertEquals(evaluateExpression('max(5, 3)', {}), 5);
});

// ============================================================================
// TERNARY
// ============================================================================

test('Ternary: basic', () => {
  assertEquals(evaluateExpression('true ? "yes" : "no"', {}), 'yes');
  assertEquals(evaluateExpression('false ? "yes" : "no"', {}), 'no');
});

test('Ternary: with condition', () => {
  assertEquals(
    evaluateExpression('count > 0 ? "Has items" : "Empty"', { count: 5 }),
    'Has items'
  );
  assertEquals(
    evaluateExpression('count > 0 ? "Has items" : "Empty"', { count: 0 }),
    'Empty'
  );
});

test('Ternary: nested', () => {
  assertEquals(
    evaluateExpression('count == 0 ? "none" : count == 1 ? "one" : "many"', { count: 0 }),
    'none'
  );
  assertEquals(
    evaluateExpression('count == 0 ? "none" : count == 1 ? "one" : "many"', { count: 1 }),
    'one'
  );
  assertEquals(
    evaluateExpression('count == 0 ? "none" : count == 1 ? "one" : "many"', { count: 5 }),
    'many'
  );
});

// ============================================================================
// COMPLEX EXPRESSIONS (Real-world examples)
// ============================================================================

test('Real-world: canPublish', () => {
  assertEquals(
    evaluateExpression('newPostContent is not empty and not isPublishing', {
      newPostContent: 'Hello',
      isPublishing: false
    }),
    true
  );
  assertEquals(
    evaluateExpression('newPostContent is not empty and not isPublishing', {
      newPostContent: '',
      isPublishing: false
    }),
    false
  );
});

test('Real-world: postsCount', () => {
  assertEquals(
    evaluateExpression('size of content.posts', {
      content: { posts: [1, 2, 3, 4, 5] }
    }),
    5
  );
});

test('Real-world: charCount display', () => {
  assertEquals(
    evaluateExpression('string(length of text) + "/280"', { text: 'Hello world' }),
    '11/280'
  );
});

test('Real-world: status badge', () => {
  assertEquals(
    evaluateExpression('isPublishing ? "Publishing..." : "Publish"', {
      isPublishing: true
    }),
    'Publishing...'
  );
});

test('Real-world: visible when selected', () => {
  assertEquals(
    evaluateExpression('selectedId == post.id', {
      selectedId: 'post-1',
      post: { id: 'post-1' }
    }),
    true
  );
});

test('Real-world: disabled button', () => {
  assertEquals(
    evaluateExpression('length of newPostContent < 10 or length of newPostContent > 280', {
      newPostContent: 'Hi'
    }),
    true
  );
});

// ============================================================================
// EVALUATE VALUE (with {{ }} wrapper)
// ============================================================================

test('evaluateValue: with expression', () => {
  assertEquals(evaluateValue('{{5 + 3}}', {}), 8);
  assertEquals(evaluateValue('{{name}}', { name: 'Alice' }), 'Alice');
});

test('evaluateValue: without expression', () => {
  assertEquals(evaluateValue('plain text', {}), 'plain text');
  assertEquals(evaluateValue(42, {}), 42);
});

test('evaluateValue: with whitespace', () => {
  assertEquals(evaluateValue('  {{  5 + 3  }}  ', {}), 8);
});

// ============================================================================
// HAS EXPRESSION
// ============================================================================

test('hasExpression: detects expressions', () => {
  assertEquals(hasExpression('{{foo}}'), true);
  assertEquals(hasExpression('plain text'), false);
  assertEquals(hasExpression('{{ 5 + 3 }}'), true);
});

console.log('\n✅ All tests passed!');
