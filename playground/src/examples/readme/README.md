# tinyqueue

A small, fast priority queue for TypeScript. Zero dependencies, 1 KB gzipped.

> [!NOTE]
> This README is an example. Edit it on the left and watch every format
> update on the right.

## Install

```sh
pnpm add tinyqueue
```

## Usage

```ts
import { TinyQueue } from "tinyqueue";

const queue = new TinyQueue<number>([5, 1, 3]);
queue.push(2);
console.log(queue.pop()); // 1
```

## Complexity

| Operation | Time        | Notes                     |
|-----------|-------------|---------------------------|
| `push`    | $O(\log n)$ | sift up                   |
| `pop`     | $O(\log n)$ | sift down                 |
| `peek`    | $O(1)$      | reads the root            |

A binary heap with $n$ items has height $h = \lfloor \log_2 n \rfloor$, so
both operations touch at most $h + 1$ nodes[^heap].

## Roadmap

- [x] `push`, `pop`, `peek`
- [x] Custom comparators
- [ ] `decreaseKey`

[^heap]: See any algorithms textbook, chapter on heaps.
