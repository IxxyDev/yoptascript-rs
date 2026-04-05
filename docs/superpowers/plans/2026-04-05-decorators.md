# TC39 Stage 3 Decorators Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement TC39 Stage 3 decorators for classes and class members in YoptaScript-rs.

**Architecture:** Add `@` token to lexer, `decorators: Vec<Expr>` to AST nodes (ClassDecl + ClassMember variants except Constructor), and decorator application logic in the interpreter following TC39 execution order. Key insight: ALL decorator expressions are evaluated first (top-to-bottom), then applied in category order: method/getter/setter → field → class (bottom-to-top per element). Each decorator receives `(value, context)` where context is `{вид, имя, статичное, приватное, добавитьИнициализатор}`. `addInitializer` callbacks stored on `ClassDef` — static ones run after class creation, instance ones run on each `new`.

**Tech Stack:** Rust, no new dependencies.

**Test patterns in this codebase:**
- Parser tests: `SourceFile::new("test.yop".to_string(), src.to_string())` → `Parser::new(&tokens, &source)`
- Interpreter tests: `run_code(src) -> Interpreter` → `interp.get("var")` to check variables. No output capture exists — test results via stored variables, not `сказать` output.
- `тырыпыры` is the internal binding name for `this` in the interpreter.

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/yps-lexer/src/token.rs:84-100` | Modify | Add `At` to `PunctuationKind` |
| `crates/yps-lexer/src/lexer.rs:393` | Modify | Handle `@` in `read_operator_or_punctuation()` |
| `crates/yps-parser/src/ast/stmt.rs:5-11,97-102` | Modify | Add `decorators: Vec<Expr>` to ClassDecl and ClassMember variants |
| `crates/yps-parser/src/parser/mod.rs:526-551,1670-1704,1706-1825` | Modify | Parse `@expr` before class and class members |
| `crates/yps-interpreter/src/value.rs:12-25` | Modify | Add `instance_initializers` and `static_initializers` to ClassDef |
| `crates/yps-interpreter/src/interpreter.rs:254,1350-1441,1443-1523` | Modify | Apply decorators during class creation; run initializers |

---

### Task 1: Lexer — Add `@` token

**Files:**
- Modify: `crates/yps-lexer/src/token.rs:84-100`
- Modify: `crates/yps-lexer/src/lexer.rs:393`
- Test: `crates/yps-lexer/src/lexer.rs` (inline tests)

- [ ] **Step 1: Write the failing test**

In `crates/yps-lexer/src/lexer.rs`, add to the inline test module:

```rust
#[test]
fn test_at_token() {
    let source = SourceFile::new("test.yop".to_string(), "@декоратор".to_string());
    let (tokens, diags) = Lexer::new(&source).tokenize();
    assert!(diags.is_empty(), "Unexpected diagnostics: {diags:?}");
    assert_eq!(tokens[0].kind, TokenKind::Punctuation(PunctuationKind::At));
    assert_eq!(tokens[1].kind, TokenKind::Identifier);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p yps-lexer test_at_token`
Expected: compilation error — `PunctuationKind::At` doesn't exist.

- [ ] **Step 3: Add `At` variant to `PunctuationKind`**

In `crates/yps-lexer/src/token.rs`, add `At` to `PunctuationKind` enum (after `Arrow`):

```rust
pub enum PunctuationKind {
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Semicolon,
    Comma,
    Colon,
    Dot,
    Spread,
    Question,
    OptionalChain,
    Arrow,
    At,
}
```

- [ ] **Step 4: Handle `@` in lexer**

In `crates/yps-lexer/src/lexer.rs`, in `read_operator_or_punctuation()`, add a match arm **before** the `_ =>` fallback (line 393):

```rust
'@' => TokenKind::Punctuation(PunctuationKind::At),
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p yps-lexer test_at_token`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/yps-lexer/src/token.rs crates/yps-lexer/src/lexer.rs
git commit -m "feat(lexer): add @ token for decorators"
```

---

### Task 2: AST — Add `decorators` field to ClassDecl and ClassMember

**Files:**
- Modify: `crates/yps-parser/src/ast/stmt.rs:5-11,97-102`

- [ ] **Step 1: Add `decorators` to ClassMember variants**

In `crates/yps-parser/src/ast/stmt.rs`, modify `ClassMember` — add `decorators: Vec<Expr>` to Method, Field, Getter, Setter (NOT Constructor per TC39 spec):

```rust
#[derive(Debug, Clone)]
pub enum ClassMember {
    Constructor { params: Vec<Param>, body: Block, span: Span },
    Method { name: Identifier, params: Vec<Param>, body: Block, is_static: bool, is_private: bool, decorators: Vec<Expr>, span: Span },
    Field { name: Identifier, init: Option<Expr>, is_static: bool, is_private: bool, decorators: Vec<Expr>, span: Span },
    Getter { name: Identifier, body: Block, is_static: bool, is_private: bool, decorators: Vec<Expr>, span: Span },
    Setter { name: Identifier, param: Param, body: Block, is_static: bool, is_private: bool, decorators: Vec<Expr>, span: Span },
}
```

- [ ] **Step 2: Add `decorators` to ClassDecl**

In `crates/yps-parser/src/ast/stmt.rs`, modify `Stmt::ClassDecl`:

```rust
ClassDecl {
    name: Identifier,
    super_class: Option<Expr>,
    members: Vec<ClassMember>,
    decorators: Vec<Expr>,
    span: Span,
},
```

- [ ] **Step 3: Fix all compilation errors from new fields**

The new fields will cause compilation errors in:
- `crates/yps-parser/src/parser/mod.rs` — every place that constructs ClassMember or ClassDecl needs `decorators: vec![]`
- `crates/yps-interpreter/src/interpreter.rs:254` — the match arm for `Stmt::ClassDecl` needs to destructure `decorators`
- `crates/yps-interpreter/src/interpreter.rs:1377-1421` — every `ClassMember::*` match arm needs `..` or named binding

Add `decorators: vec![]` to all ClassMember constructions in parser:
- Line 1801 (Method)
- Line 1736 (Getter)
- Line 1772 (Setter)
- Line 1823 (Field)

Add `decorators: vec![]` to ClassDecl construction at line 1703.

In interpreter line 254:
```rust
Stmt::ClassDecl { name, super_class, members, decorators, span } => {
    self.exec_class_decl(name, super_class.as_ref(), members, decorators, *span)
}
```

Update `exec_class_decl` signature to accept `decorators: &[Expr]` (unused for now).

In interpreter member match (lines 1377-1421): add `..` to each ClassMember pattern to ignore the new field for now.

- [ ] **Step 4: Verify compilation**

Run: `cargo build --workspace`
Expected: compiles with no errors.

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: all existing tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/yps-parser/src/ast/stmt.rs crates/yps-parser/src/parser/mod.rs crates/yps-interpreter/src/interpreter.rs
git commit -m "feat(ast): add decorators field to ClassDecl and ClassMember"
```

---

### Task 3: Parser — Parse decorator expressions

**Files:**
- Modify: `crates/yps-parser/src/parser/mod.rs:526-551,1670-1704,1706-1825`
- Test: `crates/yps-parser/src/parser/mod.rs` (inline tests)

- [ ] **Step 1: Write the failing test for class decorator parsing**

In `crates/yps-parser/src/parser/mod.rs`, add to the inline test module:

```rust
#[test]
fn test_parse_class_decorator() {
    let src = "@лог клёво Животное { }";
    let source = SourceFile::new("test.yop".to_string(), src.to_string());
    let (tokens, _) = yps_lexer::Lexer::new(&source).tokenize();
    let (program, diags) = Parser::new(&tokens, &source).parse_program();
    assert!(diags.is_empty(), "Parse errors: {diags:?}");
    match &program.items[0] {
        Stmt::ClassDecl { decorators, .. } => {
            assert_eq!(decorators.len(), 1);
        }
        other => panic!("Expected ClassDecl, got {other:?}"),
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p yps-parser test_parse_class_decorator`
Expected: FAIL — `@` is parsed as unknown expression, class not recognized.

- [ ] **Step 3: Implement `parse_decorators()` helper**

Add a new method to Parser in `crates/yps-parser/src/parser/mod.rs`. Use `parse_prefix()` which parses a primary expression + postfix chains (member access, calls, indexing) — exactly what TC39 allows for decorator expressions:

```rust
fn parse_decorators(&mut self) -> Result<Vec<Expr>, ()> {
    let mut decorators = Vec::new();
    while matches!(self.current().kind, TokenKind::Punctuation(PunctuationKind::At)) {
        self.advance();
        let expr = self.parse_prefix()?;
        decorators.push(expr);
    }
    Ok(decorators)
}
```

- [ ] **Step 4: Wire decorator parsing into `parse_statement()`**

In `parse_statement()` (line 526), add a case for `@` before the `_ =>` fallback:

```rust
TokenKind::Punctuation(PunctuationKind::At) => {
    let decorators = self.parse_decorators()?;
    if matches!(self.current().kind, TokenKind::Keyword(KeywordKind::Class)) {
        self.parse_class_decl_with_decorators(decorators)
    } else {
        let span = self.current().span;
        self.push_error(span, "Декораторы можно применять только к классам");
        Err(())
    }
}
```

- [ ] **Step 5: Create `parse_class_decl_with_decorators()`**

Refactor `parse_class_decl()` to delegate to a new method that accepts decorators:

```rust
fn parse_class_decl(&mut self) -> Result<Stmt, ()> {
    self.parse_class_decl_with_decorators(vec![])
}

fn parse_class_decl_with_decorators(&mut self, decorators: Vec<Expr>) -> Result<Stmt, ()> {
    // Same body as current parse_class_decl, but pass decorators to ClassDecl:
    // Ok(Stmt::ClassDecl { name, super_class, members, decorators, span: Span { start, end } })
}
```

- [ ] **Step 6: Wire decorator parsing into `parse_class_member()`**

At the top of `parse_class_member()` (line 1706), parse decorators before `is_static`:

```rust
fn parse_class_member(&mut self, class_name: &str) -> Result<ClassMember, ()> {
    let decorators = self.parse_decorators()?;

    let start = self.current().span.start;

    let is_static = if matches!(self.current().kind, TokenKind::Keyword(KeywordKind::Static)) {
        self.advance();
        true
    } else {
        false
    };

    // ... rest of existing code, passing decorators to each ClassMember construction
```

Pass `decorators` to each ClassMember construction (Getter, Setter, Method, Field). For Constructor: if decorators is non-empty, emit error: `"Декораторы нельзя применять к конструктору"`.

- [ ] **Step 7: Run test to verify it passes**

Run: `cargo test -p yps-parser test_parse_class_decorator`
Expected: PASS

- [ ] **Step 8: Write additional parser tests**

```rust
#[test]
fn test_parse_member_decorator() {
    let src = "клёво Ж { @лог метод() { } }";
    let source = SourceFile::new("test.yop".to_string(), src.to_string());
    let (tokens, _) = yps_lexer::Lexer::new(&source).tokenize();
    let (program, diags) = Parser::new(&tokens, &source).parse_program();
    assert!(diags.is_empty(), "Parse errors: {diags:?}");
    match &program.items[0] {
        Stmt::ClassDecl { members, .. } => {
            match &members[0] {
                ClassMember::Method { decorators, .. } => assert_eq!(decorators.len(), 1),
                other => panic!("Expected Method, got {other:?}"),
            }
        }
        other => panic!("Expected ClassDecl, got {other:?}"),
    }
}

#[test]
fn test_parse_multiple_decorators() {
    let src = "@а @б клёво К { @в @г метод() { } }";
    let source = SourceFile::new("test.yop".to_string(), src.to_string());
    let (tokens, _) = yps_lexer::Lexer::new(&source).tokenize();
    let (program, diags) = Parser::new(&tokens, &source).parse_program();
    assert!(diags.is_empty(), "Parse errors: {diags:?}");
    match &program.items[0] {
        Stmt::ClassDecl { decorators, members, .. } => {
            assert_eq!(decorators.len(), 2);
            match &members[0] {
                ClassMember::Method { decorators, .. } => assert_eq!(decorators.len(), 2),
                other => panic!("Expected Method, got {other:?}"),
            }
        }
        other => panic!("Expected ClassDecl, got {other:?}"),
    }
}

#[test]
fn test_parse_decorator_with_args() {
    let src = "@лог(\"инфо\") клёво К { }";
    let source = SourceFile::new("test.yop".to_string(), src.to_string());
    let (tokens, _) = yps_lexer::Lexer::new(&source).tokenize();
    let (program, diags) = Parser::new(&tokens, &source).parse_program();
    assert!(diags.is_empty(), "Parse errors: {diags:?}");
    match &program.items[0] {
        Stmt::ClassDecl { decorators, .. } => {
            assert_eq!(decorators.len(), 1);
            assert!(matches!(decorators[0], Expr::Call { .. }));
        }
        other => panic!("Expected ClassDecl, got {other:?}"),
    }
}
```

- [ ] **Step 9: Run all tests**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 10: Commit**

```bash
git add crates/yps-parser/src/parser/mod.rs
git commit -m "feat(parser): parse decorator expressions on classes and class members"
```

---

### Task 4: Interpreter — ClassDef initializer storage and `pending_initializers` side channel

**Files:**
- Modify: `crates/yps-interpreter/src/value.rs:12-25`
- Modify: `crates/yps-interpreter/src/interpreter.rs`

- [ ] **Step 1: Add initializer vectors to ClassDef**

In `crates/yps-interpreter/src/value.rs`, add two fields to `ClassDef`:

```rust
pub struct ClassDef {
    // ... existing fields ...
    pub parent: Option<Box<ClassDef>>,
    pub static_initializers: Vec<Value>,
    pub instance_initializers: Vec<Value>,
}
```

- [ ] **Step 2: Add `pending_initializers` to Interpreter struct**

In `crates/yps-interpreter/src/interpreter.rs`, add a field to `Interpreter`:

```rust
pub struct Interpreter {
    pub env: Environment,
    pending_initializers: Vec<Value>,
}
```

Initialize it to `Vec::new()` in `Interpreter::new()`.

- [ ] **Step 3: Handle `__добавитьИнициализатор__` in `call_function`**

At the top of `call_function()` (line 1157), before the existing match, add:

```rust
if let Value::BuiltinFunction(ref bname) = func {
    if bname == "__добавитьИнициализатор__" {
        if let Some(init_fn) = args.into_iter().next() {
            self.pending_initializers.push(init_fn);
            return Ok(Value::Undefined);
        }
        return Err(RuntimeError::new("добавитьИнициализатор ожидает функцию", span));
    }
}
```

- [ ] **Step 4: Fix compilation — add default values everywhere ClassDef is constructed**

In `exec_class_decl()` at the ClassDef construction (line 1424), add:
```rust
static_initializers: Vec::new(),
instance_initializers: Vec::new(),
```

- [ ] **Step 5: Verify compilation**

Run: `cargo build --workspace`
Expected: compiles.

- [ ] **Step 6: Commit**

```bash
git add crates/yps-interpreter/src/value.rs crates/yps-interpreter/src/interpreter.rs
git commit -m "feat(interpreter): add initializer storage and pending_initializers for decorators"
```

---

### Task 5: Interpreter — Core decorator application logic

This is the main task. Implements the full TC39 decorator execution with correct order.

**Files:**
- Modify: `crates/yps-interpreter/src/interpreter.rs:1350-1441`
- Test: `crates/yps-interpreter/src/interpreter.rs` (inline tests)

- [ ] **Step 1: Write the failing test for method decorator**

Tests use `interp.get()` to check variable state — no output capture:

```rust
#[test]
fn test_method_decorator() {
    let interp = run_code(r#"
        йопта обёртка(метод, контекст) {
            отвечаю (...аргс) => {
                отвечаю метод(аргс[0], аргс[1]) * 2;
            };
        }

        клёво К {
            @обёртка
            сложить(а, б) {
                отвечаю а + б;
            }
        }

        гыы к = захуярить К();
        гыы рез = к.сложить(3, 4);
    "#);
    assert_eq!(interp.get("рез"), Some(Value::Number(14.0)));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p yps-interpreter test_method_decorator`
Expected: FAIL — decorators are parsed but ignored in interpreter. `рез` will be `7` not `14`.

- [ ] **Step 3: Implement `build_decorator_context` helper**

```rust
fn build_decorator_context(
    &self,
    kind: &str,
    name: &str,
    is_static: bool,
    is_private: bool,
    span: Span,
) -> Value {
    let mut ctx = HashMap::new();
    ctx.insert("вид".to_string(), Value::String(kind.to_string()));
    ctx.insert("имя".to_string(), Value::String(name.to_string()));
    ctx.insert("статичное".to_string(), Value::Boolean(is_static));
    ctx.insert("приватное".to_string(), Value::Boolean(is_private));
    ctx.insert("добавитьИнициализатор".to_string(),
        Value::BuiltinFunction("__добавитьИнициализатор__".to_string()));
    Value::Object(ctx)
}
```

- [ ] **Step 4: Implement `apply_member_decorators` helper**

Applies pre-evaluated decorator functions (already evaluated `Value`s, not `Expr`s) in reverse order (bottom-to-top):

```rust
fn apply_member_decorators(
    &mut self,
    value: Value,
    decorator_fns: &[Value],
    kind: &str,
    name: &str,
    is_static: bool,
    is_private: bool,
    span: Span,
) -> Result<(Value, Vec<Value>), RuntimeError> {
    if decorator_fns.is_empty() {
        return Ok((value, vec![]));
    }

    let mut current = value;
    let mut collected_initializers = Vec::new();

    for decorator_fn in decorator_fns.iter().rev() {
        self.pending_initializers.clear();
        let context = self.build_decorator_context(kind, name, is_static, is_private, span);
        let result = self.call_function(decorator_fn.clone(), vec![current.clone(), context], span)?;
        collected_initializers.extend(self.pending_initializers.drain(..));
        if !matches!(result, Value::Undefined) {
            current = result;
        }
    }

    Ok((current, collected_initializers))
}
```

- [ ] **Step 5: Rewrite `exec_class_decl` with TC39 execution order**

The key insight: ALL decorator expressions must be evaluated FIRST, THEN applied in order. This requires a two-pass approach inside `exec_class_decl`:

**Pass 1 — Evaluate all decorator expressions (top-to-bottom):**

```rust
fn exec_class_decl(
    &mut self,
    name: &yps_parser::ast::Identifier,
    super_class: Option<&Expr>,
    members: &[ClassMember],
    decorators: &[Expr],
    span: Span,
) -> Result<Option<ControlFlow>, RuntimeError> {
    let parent = /* ... existing parent resolution ... */;

    // PASS 1: Evaluate ALL decorator expressions first (top-to-bottom, class then members)
    let mut class_dec_fns = Vec::new();
    for dec_expr in decorators {
        class_dec_fns.push(self.eval_expr(dec_expr)?);
    }

    struct MemberInfo {
        kind: MemberKind,
        name: String,
        is_static: bool,
        is_private: bool,
        decorator_fns: Vec<Value>,
    }
    enum MemberKind { Method, Getter, Setter, Field, Constructor }

    let mut member_infos = Vec::new();
    for member in members {
        let dec_exprs = match member {
            ClassMember::Method { decorators, .. } |
            ClassMember::Field { decorators, .. } |
            ClassMember::Getter { decorators, .. } |
            ClassMember::Setter { decorators, .. } => decorators,
            ClassMember::Constructor { .. } => { member_infos.push(None); continue; }
        };
        let mut fns = Vec::new();
        for dec_expr in dec_exprs {
            fns.push(self.eval_expr(dec_expr)?);
        }
        member_infos.push(Some(fns));
    }
```

**Pass 2 — Process members, applying decorators in TC39 order (methods/getters/setters first, then fields):**

Process members in two sub-passes within the member loop:
1. First loop: process methods/getters/setters, skip fields
2. Second loop: process fields

For each decorated member, call `apply_member_decorators` with the pre-evaluated decorator `Value`s. Collect initializers into `static_inits` and `instance_inits` vectors.

**Pass 3 — Apply class decorators (bottom-to-top):**

After building ClassDef, apply class decorators and run static initializers.

See full implementation details in Step 6.

- [ ] **Step 6: Implement the full rewritten `exec_class_decl`**

The full method combines all three passes. Key changes from current code:
- Accepts `decorators: &[Expr]` parameter
- Pre-evaluates all decorator expressions
- Processes methods/getters/setters first (applying their decorators), then fields
- Builds ClassDef with `static_initializers` and `instance_initializers`
- Applies class decorators last
- Runs static initializers after class decorators
- Stores instance initializers on ClassDef for `construct_instance` to use

For methods: extract the decorated method back into `MethodDef` tuple:
```rust
let (decorated, inits) = self.apply_member_decorators(
    method_fn, &decorator_fns, "метод", &m_name, is_static, is_private, span
)?;
let entry = match decorated {
    Value::Function { params, body, env, .. } => (params, body, env),
    _ => return Err(RuntimeError::new("Декоратор метода должен вернуть функцию", span)),
};
```

For fields: if decorator returns a function, store it as a transform to apply during field initialization:
```rust
let (init_transform, inits) = self.apply_member_decorators(
    Value::Undefined, &decorator_fns, "поле", &f_name, is_static, is_private, span
)?;
```

This requires changing `field_inits` type from `Vec<(String, Option<Rc<Block>>)>` to `Vec<(String, Option<Rc<Block>>, Option<Value>)>` — third element is decorator transform. Update `ClassDef` and `init_fields()` accordingly.

- [ ] **Step 7: Run test to verify it passes**

Run: `cargo test -p yps-interpreter test_method_decorator`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add crates/yps-interpreter/src/interpreter.rs crates/yps-interpreter/src/value.rs
git commit -m "feat(interpreter): apply decorators to class members with TC39 execution order"
```

---

### Task 6: Interpreter — Field decorators

**Files:**
- Modify: `crates/yps-interpreter/src/interpreter.rs`
- Test: `crates/yps-interpreter/src/interpreter.rs` (inline tests)

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn test_field_decorator() {
    let interp = run_code(r#"
        йопта удвоить(_, контекст) {
            отвечаю (начальное) => {
                отвечаю начальное * 2;
            };
        }

        клёво К {
            @удвоить
            значение = 21;
        }

        гыы к = захуярить К();
        гыы рез = к.значение;
    "#);
    assert_eq!(interp.get("рез"), Some(Value::Number(42.0)));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p yps-interpreter test_field_decorator`
Expected: FAIL — field decorator transform not applied during init.

- [ ] **Step 3: Update `init_fields` to apply decorator transforms**

In `init_fields()`, when initializing a field: if third element of tuple is `Some(transform_fn)`, call it with the base value:

```rust
for (name, init_body, transform) in &class_def.field_inits {
    let base_val = if let Some(body) = init_body {
        // ... existing eval logic ...
    } else {
        Value::Undefined
    };
    let final_val = if let Some(tf) = transform {
        self.call_function(tf.clone(), vec![base_val], span)?
    } else {
        base_val
    };
    instance.insert(name.clone(), final_val);
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p yps-interpreter test_field_decorator`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/yps-interpreter/src/interpreter.rs crates/yps-interpreter/src/value.rs
git commit -m "feat(interpreter): apply field decorators with initializer transform"
```

---

### Task 7: Interpreter — Class decorators

**Files:**
- Modify: `crates/yps-interpreter/src/interpreter.rs`
- Test: `crates/yps-interpreter/src/interpreter.rs` (inline tests)

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn test_class_decorator() {
    let interp = run_code(r#"
        йопта проверить(класс, контекст) {
            гыы к = захуярить класс();
            к.проверен = правда;
            отвечаю класс;
        }

        @проверить
        клёво К { }
    "#);
    let class_val = interp.get("К");
    assert!(matches!(class_val, Some(Value::Class(_))));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p yps-interpreter test_class_decorator`
Expected: FAIL

- [ ] **Step 3: Implement class decorator application in `exec_class_decl`**

After building ClassDef and before defining in env (already structured in Task 5), apply class decorators:

```rust
let mut class_val = Value::Class(Rc::new(class_def));

for decorator_fn in class_dec_fns.iter().rev() {
    self.pending_initializers.clear();
    let context = self.build_decorator_context("класс", &name.name, false, false, span);
    let result = self.call_function(decorator_fn.clone(), vec![class_val.clone(), context], span)?;
    static_inits.extend(self.pending_initializers.drain(..));
    if !matches!(result, Value::Undefined) {
        class_val = result;
    }
}

for init in &static_inits {
    self.call_function(init.clone(), vec![], span)?;
}

self.env.define(name.name.clone(), class_val, false);
```

- [ ] **Step 4: Write test for class decorator context fields**

```rust
#[test]
fn test_class_decorator_context() {
    let interp = run_code(r#"
        гыы сохр;
        йопта запомнить(класс, контекст) {
            сохр = контекст;
            отвечаю класс;
        }

        @запомнить
        клёво МойКласс { }
    "#);
    let ctx = interp.get("сохр").unwrap();
    match ctx {
        Value::Object(map) => {
            assert_eq!(map.get("вид"), Some(&Value::String("класс".to_string())));
            assert_eq!(map.get("имя"), Some(&Value::String("МойКласс".to_string())));
            assert_eq!(map.get("статичное"), Some(&Value::Boolean(false)));
            assert_eq!(map.get("приватное"), Some(&Value::Boolean(false)));
        }
        _ => panic!("Expected Object context"),
    }
}
```

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/yps-interpreter/src/interpreter.rs
git commit -m "feat(interpreter): apply class decorators with TC39 Stage 3 semantics"
```

---

### Task 8: Interpreter — `addInitializer` execution

**Files:**
- Modify: `crates/yps-interpreter/src/interpreter.rs:1443-1523`
- Test: `crates/yps-interpreter/src/interpreter.rs` (inline tests)

- [ ] **Step 1: Write the failing test for instance addInitializer**

```rust
#[test]
fn test_add_initializer_instance() {
    let interp = run_code(r#"
        гыы счётчик = 0;
        йопта отслеживание(метод, контекст) {
            контекст.добавитьИнициализатор(() => {
                счётчик += 1;
            });
            отвечаю метод;
        }

        клёво К {
            @отслеживание
            метод() { }
        }

        гыы к1 = захуярить К();
        гыы к2 = захуярить К();
        гыы рез = счётчик;
    "#);
    assert_eq!(interp.get("рез"), Some(Value::Number(2.0)));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p yps-interpreter test_add_initializer_instance`
Expected: FAIL — `счётчик` stays 0 because instance initializers aren't run.

- [ ] **Step 3: Run instance initializers in `construct_instance()`**

In `construct_instance()`, after `self.init_fields(...)` and before running constructor, run instance initializers. `тырыпыры` is the internal name for `this` — set it so initializers can access the instance:

```rust
let instance_val = Value::Object(instance);

for init in &class_def.instance_initializers {
    let saved = self.env.clone();
    self.env.push_scope();
    self.env.define("тырыпыры".to_string(), instance_val.clone(), false);
    self.call_function(init.clone(), vec![], span)?;
    self.env = saved;
}
```

Insert this between the `let instance_val = Value::Object(instance);` line and the constructor call.

- [ ] **Step 4: Write test for static addInitializer**

```rust
#[test]
fn test_add_initializer_static() {
    let interp = run_code(r#"
        гыы инициализирован = лож;
        йопта регистрация(_, контекст) {
            контекст.добавитьИнициализатор(() => {
                инициализирован = правда;
            });
        }

        клёво К {
            @регистрация
            попонятия х = 1;
        }

        гыы рез = инициализирован;
    "#);
    assert_eq!(interp.get("рез"), Some(Value::Boolean(true)));
}
```

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/yps-interpreter/src/interpreter.rs
git commit -m "feat(interpreter): implement addInitializer for decorator context"
```

---

### Task 9: TC39 execution order verification and edge cases

**Files:**
- Test: `crates/yps-interpreter/src/interpreter.rs` (inline tests)

- [ ] **Step 1: Write test for TC39 decorator execution order**

Per TC39: decorator expressions evaluate top-to-bottom, applied bottom-to-top per element. Methods before fields before class.

```rust
#[test]
fn test_decorator_execution_order() {
    let interp = run_code(r#"
        гыы журнал = [];
        йопта д(тег) {
            журнал = втолкнуть(журнал, "выч:" + тег);
            отвечаю (значение, контекст) => {
                журнал = втолкнуть(журнал, "прим:" + тег + ">" + контекст.вид);
                отвечаю значение;
            };
        }

        @д("класс")
        клёво К {
            @д("метод")
            м() { }

            @д("поле")
            х = 1;
        }

        гыы рез = журнал;
    "#);
    let log = interp.get("рез").unwrap();
    match log {
        Value::Array(items) => {
            let strs: Vec<String> = items.iter().map(|v| v.to_string()).collect();
            assert_eq!(strs, vec![
                "выч:класс",
                "выч:метод",
                "выч:поле",
                "прим:метод>метод",
                "прим:поле>поле",
                "прим:класс>класс",
            ]);
        }
        _ => panic!("Expected Array"),
    }
}
```

- [ ] **Step 2: Run test**

Run: `cargo test -p yps-interpreter test_decorator_execution_order`

If it fails, the two-pass architecture from Task 5 needs adjustment. The pre-evaluation of all decorator expressions in Pass 1 should produce the correct "выч:" order, and the application in Pass 2 (methods → fields → class) should produce the correct "прим:" order.

- [ ] **Step 3: Write test for multiple decorators on one member (bottom-to-top)**

```rust
#[test]
fn test_multiple_decorators_order() {
    let interp = run_code(r#"
        гыы журнал = [];
        йопта первый(м, к) { журнал = втолкнуть(журнал, "первый"); отвечаю м; }
        йопта второй(м, к) { журнал = втолкнуть(журнал, "второй"); отвечаю м; }

        клёво К {
            @первый
            @второй
            метод() { }
        }

        гыы рез = журнал;
    "#);
    let log = interp.get("рез").unwrap();
    match log {
        Value::Array(items) => {
            let strs: Vec<String> = items.iter().map(|v| v.to_string()).collect();
            assert_eq!(strs, vec!["второй", "первый"]);
        }
        _ => panic!("Expected Array"),
    }
}
```

- [ ] **Step 4: Write test for decorator on getter/setter**

```rust
#[test]
fn test_getter_decorator() {
    let interp = run_code(r#"
        йопта удвоить(геттер, контекст) {
            отвечаю () => {
                отвечаю геттер() * 2;
            };
        }

        клёво К {
            #внутр = 10;

            @удвоить
            get значение() { отвечаю тырыпыры.#внутр; }
        }

        гыы к = захуярить К();
        гыы рез = к.значение;
    "#);
    assert_eq!(interp.get("рез"), Some(Value::Number(20.0)));
}
```

- [ ] **Step 5: Run all tests and lint**

```bash
cargo clippy --workspace --all-targets --all-features -D warnings
cargo fmt --all --check
cargo test
```

- [ ] **Step 6: Fix any issues**

- [ ] **Step 7: Commit**

```bash
git add crates/yps-interpreter/src/interpreter.rs
git commit -m "test: verify TC39 decorator execution order and edge cases"
```

---

### Task 10: Final verification and example

**Files:**
- Create: `examples/decorators.yop`
- Test: all crates

- [ ] **Step 1: Write example file**

Create `examples/decorators.yop`:

```yoptascript
йопта лог(метод, контекст) {
    гыы имя = контекст.имя;
    отвечаю (...аргс) => {
        сказать("[" + имя + "] вызов");
        гыы результат = метод(аргс[0], аргс[1]);
        сказать("[" + имя + "] результат: " + результат);
        отвечаю результат;
    };
}

клёво Калькулятор {
    @лог
    сложить(а, б) {
        отвечаю а + б;
    }

    @лог
    умножить(а, б) {
        отвечаю а * б;
    }
}

гыы к = захуярить Калькулятор();
к.сложить(2, 3);
к.умножить(4, 5);
```

- [ ] **Step 2: Run the example**

Run: `cargo run -p yps-cli -- examples/decorators.yop`
Expected output:
```
[сложить] вызов
[сложить] результат: 5
[умножить] вызов
[умножить] результат: 20
```

- [ ] **Step 3: Run full CI checks**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -D warnings
cargo test
```

- [ ] **Step 4: Final commit**

```bash
git add examples/decorators.yop
git commit -m "feat: add decorators example"
```
