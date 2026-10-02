---
name: complexity
description: Strategy for code complexity control (mostly visible as LOC count).
---

## How to measure?

Use `tools/complexity.py`. It will run tokei over all non-test Rust files and will output
2 lists: files in `80-90` range and files with more than 90 LOC.

## Why?

Main points:

* readability
* absense of obscure logic/hidden loops/complexity
* testability
* reusability
* technical debt reduction

## Strategy

The stategy of dealing with complexity depends on the code - is it dense or sparse.

### Sparse code

Sparse code consists of small, separate functions, easily testable in isolation, with
relatively small number of arguments and local variables.

The complexity control strategy for sparse code - split the file across natural lines,
keep semantically close functions together. Usually splitting into 2 files is enough
to get the file below 80 LOC.

Since test files are not limited in complexity, they don't need to be split following
the split of the module, unless it is semantically appropriate for better reusability.

### Dense code

Dense code consists of larger functions with complex logic, with relatively large
number of local variables.

Dense code doesn't benefit from splitting much - the overhead of importing functions
from submodules and pass multiple parameters is the 'use' clause (one or several lines)
+ argument passing, and since `cargo fmt` will insert extra line breaks to improve
readability, the net gain is very small and sometimes negative.

The best strategy for dense code is to add a newtype which encapsulates the large local
state and then implement that newtype in splittable methods.

In this case, some tests from the original module can follow the split as well, testing
the newtype in isolation.

Sometimes, code doesn't need to be split into a separate file - just extracting some
common repeatable logic into a separate method/function can be enough to reduce LOC count.

## Don't

Don't try to reduce line of code by manipulating the whitespace. `cargo fmt` will undo
most of the manipulations.

Don't try to reduce number of `use` clauses by providing full path at the call site.

Don't try to condense code by removing empty lines or comments - tokei doesn't count them.

Don't add large comment blocks - if the code at hand requires a large comment block, it is
already too complex and can benefit from splitting into more simple reusable snippets.

## Do

Promote reusability. It is much better if the newly split code can be reused by
other modules. It is worth it to scout similar files to see if they're solving similar
problems and try to come up with common solution that will save overall LOC number
rather than reducing LOC count in the particular file. This is the highest priority goal.

Reduce average indentation level. Higher indentation usually means complex logic and dense code.
Lower indentation means more sparse code, easier to maintain and control.

Keep semantically similar functions together if possible.

Prefer generics over `dyn Trait` objects.

If newtype is created to contain dense logic, prefer methods with `&self` or `&mut self`
to functions with `&Newtype` or `&mut Newtype` arguments if semantically justifiable.
