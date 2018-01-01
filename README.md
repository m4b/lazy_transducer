# Lazy Transducer [![Build Status](https://travis-ci.org/m4b/lazy_transducer.svg?branch=master)](https://travis-ci.org/m4b/lazy_transducer)

Lazy transducers are generic, lazy, parallel, iterators transforming one data source into `n` output data types.

See the online [documentation](https://docs.rs/lazy_transducer) for more information.

## Using

Add this to your `Cargo.toml`

```toml
[dependencies]
lazy_transducer = "0.1"
```

## Example

```rust
extern crate lazy_transducer;
extern crate rayon;

use rayon::prelude::*;
use lazy_transducer::LazyTransducer;

fn main() {
  let data = [0xdeadbeefu32, 0xcafed00d];
  let lt: LazyTransducer<&[u32], u64> = LazyTransducer::new(&data, 2, |input, idx| input[idx] as u64);

  let cafedood = lt.get(1).expect("has 2 elements");
  assert_eq!(cafedood, 0xcafed00d);

  lt.into_par_iter().for_each(|elem| {
    println!("{}", elem);
  });
}
```
