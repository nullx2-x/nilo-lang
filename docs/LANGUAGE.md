# Nilo Language Guide

This document describes Nilo 0.2. Statements end with `;`; blocks use braces; `//` and nested `/* ... */` comments are supported.

## Values and variables

```nilo
let count: int = 3;
let ratio: float = 0.5;
let active: bool = true;
let title: str = "Nilo";
let missing: str? = nil;
let numbers: list<int> = [1, 2, 3];
let metadata: map<str, any> = {"stable": false, "version": 2};
```

Annotations are checked whenever values enter a variable, function parameter, return slot, record field, or typed collection. Nilo does not silently coerce values; use `int(...)`, `float(...)`, or `str(...)` explicitly.

Built-in type names are `any`, `nil`, `bool`, `int`, `float`, `num`, `str`, `list`, `map`, `func`, `type`, and `module`. A trailing `?` accepts `nil` in addition to the named type.

## Functions and closures

```nilo
func make_counter(start: int) -> func {
    let value: int = start;
    func next() -> int {
        value = value + 1;
        return value;
    }
    return next;
}

let counter = make_counter(10);
print(counter()); // 11
```

Functions have lexical scope and may call themselves recursively. A function without an explicit `return` returns `nil`.

## Records

```nilo
type User {
    name: str;
    age: int;
}

let user: User = User("Ada", 36);
user.age = user.age + 1;
print(user.name, user.age);
```

The constructor arguments follow declaration order. Field writes retain the declared field type.

## Control flow

```nilo
for value in range(0, 10) {
    if (value == 2) {
        continue;
    } else if (value == 8) {
        break;
    }
    print(value);
}

let attempts: int = 0;
while (attempts < 3) {
    attempts = attempts + 1;
}
```

`nil`, `false`, numeric zero, empty strings, empty lists, and empty maps are falsey. Other values are truthy. `&&` and `||` short-circuit and produce booleans.

## Collections

Lists and maps are mutable reference values. Passing one to a function shares the same collection.

```nilo
let values: list<int> = [1, 2, 3];
values[0] = 10;
push(values, 4);
print(values[-1]);

let settings: map<str, any> = {"theme": "dark"};
settings.theme = "light";
settings["retries"] = 3;
```

Map keys may be strings, integers, or booleans. Property syntax is shorthand for a string key.

## Modules

`tools.nilo`:

```nilo
export let version: str = "0.2";
export func double(value: int) -> int {
    return value * 2;
}
```

Consumer:

```nilo
import "tools" as tools;
from "tools" import double;

print(tools.version);
print(double(21));
```

Relative modules resolve from the importing file. The `.nilo` extension is optional. Standard library modules use paths beginning with `std/`.

## Built-ins

- `print(values...)`
- `len(value)`
- `push(list, value)` / `pop(list)`
- `str(value)`, `int(value)`, `float(value)`, `bool(value)`
- `range(end)`, `range(start, end)`, `range(start, end, step)`
- `assert(condition, message?)`
- `type_of(value)`
- `keys(value)` / `values(value)`
- `clock()`

## Standard library

- `std/json`: `parse`, `stringify`
- `std/regex`: `compile`, `is_match`, `find`, `find_all`, `captures`, `replace`, `split`, `escape`, `flags`
- `std/fs`: `read_text`, `write_text`, `exists`, `list_dir`, `remove`
- `std/http`: `get`, `post`
- `std/time`: `now`, `sleep`
- `std/list`: `push`, `pop`, `join`, `reverse`, `sort`
- `std/string`: `split`, `trim`, `lower`, `upper`, `contains`, `replace`
- `std/math`: `abs`, `min`, `max`, `round`, `floor`, `ceil`, `pow`, `sqrt`

File-system paths resolve relative to the source file making the call. HTTP calls return a map with `status`, `headers`, and `body`.

## Current boundaries

Nilo 0.2 is a tree-walking interpreter. It has runtime annotation checks rather than full static inference, no user-defined generics, no async runtime, and no package registry. These are deliberate 0.x boundaries, not promises of current behavior.
