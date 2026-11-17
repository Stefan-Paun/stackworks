# Setzwerk

Setzwerk is a small C-like language whose compiler (`crates/setz`) translates a `.sz` file into
Stapelwerk assembly. The output is ordinary, commented assembly: it assembles with the normal
assembler, runs in the normal machine and can be read line by line next to the source. It is a
teaching compiler in the tradition of many small single-pass compilers (Wirth's PL/0, the "Tiny C"
family, Crenshaw's *Let's Build a Compiler*); it makes no claim to new techniques.

```
stapelwerk compile prog.sz            # assembly on stdout
stapelwerk compile prog.sz -o prog.s  # assembly into a file
stapelwerk run prog.s --in "3 4"      # run it like any other program
```

Contents: [Example](#example) · [Lexical structure](#lexical-structure) · [Grammar](#grammar) ·
[Semantics](#semantics) · [Names and scopes](#names-and-scopes) · [Code generation](#code-generation) ·
[Stack frame](#stack-frame) · [Limits and errors](#limits-and-errors) · [Tests](#tests)

## Example

```
// Factorial, recursively.
fn fak(n) {
    if (n <= 1) { return 1; }
    return n * fak(n - 1);
}

fn main() {
    var n = read();
    print(fak(n));
}
```

## Lexical structure

- Source is UTF-8; only ASCII may appear outside comments.
- Whitespace separates tokens. `//` starts a comment that runs to the end of the line.
- Identifiers: `[A-Za-z_][A-Za-z0-9_]*`, case-sensitive.
- Keywords (cannot be used as names): `fn var if else while break continue return read print putc`.
- Integer literals: decimal `42`, hexadecimal `0x2A`, binary `0b101010`, character `'A'`, `'\n'`
  (escapes `\n \t \0 \\ \'`). A literal must be at most 4294967295 (2³² − 1); its 32 bits are read
  as an i32, so `0xFFFFFFFF` is −1 and `-2147483648` works (negation of 2147483648 wraps).
- Operators and punctuation: `( ) { } , ; = + - * / % << >> < <= > >= == != & ^ | && || ! ~`.

## Grammar

EBNF; `{ x }` is zero or more, `[ x ]` optional.

```
program    = { global | function } ;
global     = "var" IDENT [ "=" [ "-" ] INT ] ";" ;
function   = "fn" IDENT "(" [ IDENT { "," IDENT } ] ")" block ;

block      = "{" { statement } "}" ;
statement  = "var" IDENT [ "=" expr ] ";"
           | IDENT "=" expr ";"
           | "if" "(" expr ")" block [ "else" ( block | ifstmt ) ]
           | "while" "(" expr ")" block
           | "break" ";" | "continue" ";"
           | "return" [ expr ] ";"
           | "print" "(" expr ")" ";"
           | "putc" "(" expr ")" ";"
           | block
           | expr ";" ;

expr       = or ;
or         = and { "||" and } ;
and        = bitor { "&&" bitor } ;
bitor      = bitxor { "|" bitxor } ;
bitxor     = bitand { "^" bitand } ;
bitand     = equality { "&" equality } ;
equality   = relation { ( "==" | "!=" ) relation } ;
relation   = shift { ( "<" | "<=" | ">" | ">=" ) shift } ;
shift      = additive { ( "<<" | ">>" ) additive } ;
additive   = term { ( "+" | "-" ) term } ;
term       = unary { ( "*" | "/" | "%" ) unary } ;
unary      = ( "-" | "!" | "~" ) unary | primary ;
primary    = INT | IDENT | IDENT "(" [ expr { "," expr } ] ")" | "read" "(" ")" | "(" expr ")" ;
```

Precedence and associativity are those of C (all binary operators associate to the left). Bodies of
`if`, `else` and `while` are always blocks (braces required), which removes the dangling-else
question; `else if` is allowed.

## Semantics

There is exactly one type: the 32-bit signed integer (i32). Every value, variable, parameter and
return value is an i32.

| Construct | Meaning |
|---|---|
| `+ - *`, unary `-` | two's complement, wrapping modulo 2³² (never an error) |
| `/`, `%` | signed, rounding toward 0; the remainder has the sign of the dividend. Division by 0 stops the program with `fault.div_zero`. `-2147483648 / -1` = −2147483648, `% -1` = 0 |
| `<<`, `>>` | shift count taken `& 31`; `>>` is arithmetic (copies the sign bit) |
| `& ^ \| ~` | bitwise |
| `< <= > >= == !=` | signed comparison, result 1 or 0 |
| `!x` | 1 if x is 0, else 0 |
| `a && b`, `a \|\| b` | short-circuit; result 1 or 0; `b` is evaluated only if needed |
| `if`, `while` | a condition is true if it is not 0 |
| `read()` | next number of the input (`sys READ_INT`); no number left → `fault.input_end` in test mode |
| `print(e)` | writes e as a number (`sys WRITE_INT`) |
| `putc(e)` | writes the byte `e & 0xFF` (`sys WRITE_CHAR`) |
| `return e;` | returns e; `return;` and falling off the end of a function return 0 |
| `break`, `continue` | innermost `while` loop; outside a loop they are compile errors |

**Evaluation order** is strictly left to right: the left operand before the right, arguments from
the first to the last, the right-hand side of `x = e` before the store. This matters because
`read()`, `print` inside called functions, and faults are observable.

**Program start.** If the program defines `fn main()` (no parameters), execution starts by calling
`main`; when `main` returns, the machine halts. A program without `main` is a *library*: its code
starts directly with the first function. That is what function-test levels (`call = LABEL`) need;
running such a file on its own is meaningless (the first function would return to nowhere).

**Global variables** live in `.data`, are initialised with a constant (default 0) and are visible
in all functions. **Local variables** must be declared with `var` before use and are initialised
(default 0); parameters behave exactly like local variables.

## Names and scopes

- Functions and global variables share one namespace (they become assembler labels). Their names
  must not be reserved by the assembler (registers, mnemonics such as `add` or `div`, `li`, system
  call names, built-in constants such as `SCREEN` or `RED`, all case-insensitive, and names that
  start with `__`), because the label would not assemble; the compiler reports this.
- Locals and parameters live in block scopes. A name may shadow a global or a name of an enclosing
  block, but not one declared in the same block (parameters count as the outermost block of the
  function). A new name becomes visible after its `var` statement, so in `var x = x + 1;` the
  right-hand side still means the outer `x`.
- A function may be called before its definition (recursion and mutual recursion work). The number
  of arguments must match the number of parameters.
- `main` must have no parameters.

## Code generation

The compiler is a classic one-pass-per-stage design: lexer → recursive-descent parser → name and
arity check → code generator that walks the tree once. There is no optimiser beyond a few local
choices; the goal is assembly a learner can follow.

- **Expressions** are evaluated into `r0`. For a binary operator the left value is pushed on the
  stack, the right one is computed into `r0`, moved to `r1`, the left one popped back into `r0`, and
  then `op r0, r0, r1`. Temporaries on the stack survive calls, so `f(1) + f(2)` needs no special
  case. Two local shortcuts: if the right operand is a literal that fits the immediate of the
  instruction (`add r0, r0, 5`), or a plain variable or literal (loaded straight into `r1`), the
  push/pop is skipped.
- **Conditions** in `if`/`while` with a comparison at the top compile to `cmp` and the inverted
  conditional jump (`x < y` → `cmp r0, r1` / `jge .else`). `&&` chains jump to the same false label.
  Other conditions compare their value with 0.
- **Comparisons as values** produce 1 or 0 with `mov r0, 1` / `j<cc> .done` / `mov r0, 0`
  (`mov` does not change the flags).
- **Literals** outside −32768…32767 are loaded with `li` (two words).
- **Labels**: each function becomes a global label with its own name; everything inside a
  function uses local labels (`.if3_else`, `.while2_top`, `.exit`) so two functions never clash.
- **Comments**: every statement is preceded by a comment with its source line, so `stapelwerk run
  --trace` and the debugger can be read next to the source.

### Calling convention

Setzwerk follows the [calling convention of the machine](ISA.md#calling-convention), so compiled
functions can be called from hand-written assembly (and from function tests) and vice versa:

- Arguments 1–4 in `r0`–`r3`, further arguments on the stack, pushed from right to left (the 5th
  argument is at `[fp + 8]` in the callee). Return value in `r0`.
- The caller first evaluates **all** arguments left to right and pushes each value, because a later
  argument may itself call a function and destroy `r0`–`r3`. Then it pops the first four into
  `r0`–`r3` (for more than four arguments it first copies the stack arguments into the right order),
  calls, and removes the stack arguments with `add sp, sp, 4·k`.
- The generated code uses only `r0`, `r1`, `fp` and `sp`. It never touches `r4`–`r7`, so it
  preserves them trivially; `fp` and `sp` are restored by the epilogue.

## Stack frame

```
          higher addresses
[fp + 8 + 4·k]  stack argument 5 + k          (pushed by the caller)
[fp + 4]        return address                (pushed by call)
[fp + 0]        old fp                        (push fp)
[fp − 4]        slot 1: parameter 1 (copied from r0)
   …            slots 2–4: parameters 2–4 (from r1–r3)
   …            further slots: each `var` in the order it appears in the source
[fp − 4·N]      slot N                        ← sp after the prologue
   …            temporaries pushed while evaluating expressions
          lower addresses
```

```
name:                       ; prologue
        push fp
        mov  fp, sp
        sub  sp, sp, 4·N    ; omitted when N = 0
        st   r0, [fp - 4]   ; one store per register parameter
        …                   ; body
        mov  r0, 0          ; falling off the end returns 0
.exit:                      ; epilogue, every `return` jumps here
        mov  sp, fp
        pop  fp
        ret
```

Every `var` gets its own slot for the whole function, even in sibling blocks (simple and easy to
see in the stack view, at the price of a few bytes). The stack segment has 8 KiB, so very deep
recursion stops with `fault.stack_overflow`, exactly like hand-written assembly.

## Limits and errors

Errors are reported as `file:line:col: message` and the compiler stops at the first one. It never
panics, whatever the input (tested with 10 000 random and mutated inputs).

| Limit | Value | Why |
|---|---|---|
| nesting of expressions and blocks | 200 | the parser is recursive |
| local slots per function | 4000 | offsets must fit `[fp - 32768]` |
| parameters per function | 64 | |
| program size | text ≤ 16128 bytes, data ≤ 8192 bytes | the machine's segments; checked by assembling the output |

## Tests

- `crates/setz/tests/differential.rs`: 200 random programs (seeded, deterministic) are compiled,
  assembled and run on the machine and also executed by a separate AST interpreter
  (`crates/setz/src/interp.rs`); output and end state (normal end, `div_zero`, `input_end`) must
  match.
- `crates/setz/tests/fuzz.rs`: 10 000 random byte strings, token soups and mutations of valid
  programs; the compiler must not panic, and whatever it accepts must assemble.
- `crates/cli/tests/setz_programs.rs`: every program in `crates/setz/programs/` is compiled and run
  with `stapelwerk run`; the output must equal the `.out` file. The files in
  `crates/setz/levels/` are compiled and run against their level with `stapelwerk run --level`.
