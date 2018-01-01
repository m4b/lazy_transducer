//! # Introduction
//!
//! A lazy transducer is a declarative specification for the transformation of one type of data to another.
//!
//! The transformer is called the transducer, which receives the original source input, and an index
//! corresponding to the `i`th element, and returns the corresponding `i`th element out of that source.
//!
//! Importantly, lazy transducers are:
//!
//! 1. Lazy - it never parses any elements unless you request it to do so
//! 2. Iterable - one can iterate over every element
//! 3. Indexable - accessing an element is O(1)
//! 4. Parallel - one can iterate in parallel over every element
//!
//! When constructing a lazy transducer, it's important to remember that the transducer must be a `fn`
//! with signature `(Input, usize -> Output)`; a closure which **does not capture its environment** also
//! can be used.
//!
//! The input source needs to be copy; i.e., it should typically be a reference to `Vec<_>`, or a slice of bytes,
//! etc.
//!
//! Another detail that you should keep in mind is that the index is the `i`th element; therefore if
//! your backing data source is bytes, you need to know the _fixed length_ of your data structure you
//! are parsing out of the byte slice. It is up to the transducer implementation to determine this.
//!
//! In some cases, the size of the data structure/output element isn't known statically via `sizeof`, etc.  In these cases
//! it is suggested to pass this "context" in the input source during construction as a tuple, e.g.:
//! `(datasource, context)`, and have your transducer use the `datasource + context + the index` to correctly
//! marshall your data structure out of the bytes.
//!
//! This is the approach that the [scroll-based](type.ScrollTransducer.html) transducer takes.  See also
//! the [bincode example](struct.LazyTransducer.html#advanced-example) for a similar approach.
//!
//! The parallel implementation uses [rayon](https://docs.rs/rayon).
//!
//! # Example
//!
//! ```rust
//! extern crate lazy_transducer;
//! extern crate rayon;
//!
//! use rayon::prelude::*;
//! use lazy_transducer::LazyTransducer;
//!
//! use std::mem::size_of;
//! use std::mem::transmute;
//!
//! # fn main() {
//! let bytes: Vec<u8> = vec![1u8, 0, 2, 0, 3, 0, 4, 0];
//! let lt: LazyTransducer<_, &u16> = LazyTransducer::new(&bytes, 4, |input, index| {
//!     let start = size_of::<u16>() * index;
//!     unsafe { transmute::<_, &u16>(&input[start]) }
//! });
//!
//! // now iterate in parallel
//! lt.into_par_iter().for_each(|n| {
//!   // do stuff
//!   println!("{}", n);
//! });
//! # }
//! ```

extern crate rayon;
extern crate scroll;
#[macro_use]
extern crate failure;

mod builder;
pub use builder::*;

mod lazy_transducer;
pub use lazy_transducer::*;

pub use scroll::Endian;

/// The kind of errors for constructing lazy transducers
#[derive(Fail, Debug)]
pub enum TransducerError {
    #[fail(display = "Error during building: {}", _0)]
    BuilderError(String),
    #[fail(display = "Too many elements (size = {} * {}) requested from src of size: {}", nelements, sizeof_element, src_size)]
    ElementOverflow{ nelements: usize, sizeof_element: usize, src_size: usize },
}
