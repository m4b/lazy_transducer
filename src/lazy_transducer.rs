use std::fmt::Debug;
use std::marker::PhantomData;

use rayon::iter::*;
use rayon::iter::plumbing::*;
use failure::Error;

use scroll::{self, ctx, Pread};
use scroll::ctx::SizeWith;

use TransducerError;

/// A lazy transducer is a declarative specification for the transformation of one type of data to another.
///
/// The transformer is called the transducer, which receives the original source input, and an index
/// corresponding to the `i`th element, and returns the corresponding `i`th element out of that source.
///
/// Importantly, lazy transducers are:
///
/// 1. Lazy - it never parses any elements unless you request it to do so
/// 2. Iterable - one can iterate over every element
/// 3. Indexable - accessing an element can be O(1)
/// 4. Parallel - one can iterate in parallel over every element
#[derive(Debug)]
pub struct LazyTransducer<'a, Input, Output>
    where Input: 'a + Copy,
          Output: 'a,
{
    pub(crate) count: usize,
    pub(crate) contents: Input,
    pub(crate) transducer: fn(Input, usize) -> Output,
    pub(crate) _marker: PhantomData<&'a Output>
}

impl<'a, Input, Output> LazyTransducer<'a, Input, Output>
    where Input: 'a + Copy,
          Output: 'a
{
    /// How many elements are contained in this lazy transducer
    pub fn len(&self) -> usize {
        self.count
    }
    /// Create a new LazyTransducer with `count` elements in `contents`, using `transducer` to extract
    /// them.
    ///
    /// # Basic Example
    ///
    /// We need to provide a data source, the number of elements in the data source, and the means of
    /// extracting the elements out of the data source (the transducer).
    ///
    /// For a simple case, we can consider a backing array of `u32`s, which we cast to `u64`s.
    ///
    /// ```rust
    /// extern crate lazy_transducer;
    /// use lazy_transducer::LazyTransducer;
    ///
    /// # fn main() {
    /// let data = [0xdeadbeefu32, 0xcafed00d];
    /// let lt: LazyTransducer<&[u32], u64> = LazyTransducer::new(&data, 2, |input, idx| input[idx] as u64);
    ///
    /// let cafedood = lt.get(1).expect("has 2 elements");
    /// assert_eq!(cafedood, 0xcafed00d);
    ///
    /// for (i, elem) in lt.into_iter().enumerate() {
    ///   println!("{}: {}", i, elem);
    /// }
    /// # }
    /// ```
    ///
    /// # Advanced Example
    ///
    /// This example uses the [bincode](https://github.com/TyOverby/bincode) binary serializer as
    /// its transducer.
    ///
    /// ```rust
    /// extern crate lazy_transducer;
    /// #[macro_use]
    /// extern crate serde_derive;
    /// extern crate serde;
    /// extern crate bincode;
    /// extern crate rayon;
    ///
    /// use lazy_transducer::LazyTransducer;
    /// use bincode::{serialize, deserialize, Infinite, Error};
    /// use rayon::prelude::*;
    ///
    /// #[derive(Debug, PartialEq, Serialize, Deserialize)]
    /// pub struct Foo {
    ///   x: u64,
    ///   y: f32,
    ///   z: bool,
    /// }
    ///
    /// fn run() -> Result<(), Error> {
    ///   let foo1 = Foo { x: 0xcafed00d, y: 0.75, z: false };
    ///   let foo2 = Foo { x: 0xdeadbeef, y: 0.50, z: true };
    ///
    ///   // we need to serialize the data, which we do by extending a byte vector with the individually
    ///   // serialized components
    ///   let mut data = serialize(&foo1, Infinite)?;
    ///   let sizeof_serialized_element = data.len();
    ///   data.extend_from_slice(&serialize(&foo2, Infinite)?);
    ///
    ///   // we construct our transducer by providing the serialized bytes _and_ the size of a serialized
    ///   // element as input; our transducer just reads at the appropriate byte offset, and deserializes!
    ///   let lt: LazyTransducer<_, Result<Foo, Error>> =
    ///     LazyTransducer::new((data.as_slice(), sizeof_serialized_element),
    ///                         2,
    ///                         |(input, size), idx| {
    ///                            deserialize(&input[(idx * size)..])
    ///   });
    ///
    ///   let foo2_ = lt.get(1).expect("has 2 elements")?;
    ///   assert_eq!(foo2, foo2_);
    ///
    ///   // and now with the help of rayon, we iterate over the items in parallel
    ///   lt.into_par_iter().for_each(|elem| {
    ///     println!("{:?}", elem);
    ///   });
    ///   Ok(())
    /// }
    /// # fn main() { run().unwrap() }
    /// ```
    pub fn new(contents: Input,
               count: usize,
               transducer: fn(Input, usize) -> Output)
               -> Self
    {
        LazyTransducer {
            count,
            contents,
            transducer,
            _marker: PhantomData::default(),
        }
    }

    /// Get an element out of the lazy transducer, returning `None` if the index is greater than
    /// the number of elements in this lazy transducer.
    ///
    /// # Example
    ///
    /// ```rust
    /// extern crate lazy_transducer;
    /// use lazy_transducer::LazyTransducer;
    ///
    /// # fn main() {
    /// let data = [0xdeadbeefu64, 0xcafed00d];
    /// let lt: LazyTransducer<&[u64], &u64> = LazyTransducer::new(&data, 2, |input, idx| &input[idx]);
    ///
    /// let cafedood = lt.get(1).expect("has 2 elements");
    /// assert_eq!(*cafedood, 0xcafed00d);
    ///
    /// assert!(lt.get(2).is_none());
    /// # }
    /// ```
    #[inline]
    pub fn get(&self, idx: usize) -> Option<Output> {
        if idx >= self.count {
            None
        } else {
            Some((self.transducer)(self.contents, idx))
        }
    }
}

/// A scroll-based transducer only requires a parsing context for construction.
/// The correct method is statically dispatched according to the output type, and the bounds are checked
/// according to the size of the input and the number of elements requested from the byte source.
///
/// In order to use this, you must implement TryFromCtx and SizeWith, which you can usually derive
/// with `#[derive(Pread, SizeWith)]`
///
/// # Example
///
/// ```rust
/// extern crate lazy_transducer;
/// #[macro_use]
/// extern crate scroll;
/// use lazy_transducer::ScrollTransducer;
///
/// #[derive(Debug, Pread, SizeWith)]
/// #[repr(C)]
/// pub struct Rel {
///     pub r_offset: u32,
///     pub r_info: u32,
/// }
///
/// # fn main () {
/// let bytes = vec![4, 0, 0, 0, 5, 0, 0, 0, 1, 0, 0, 0, 5, 0, 0, 0];
/// let lt: ScrollTransducer<Rel, scroll::Endian> = ScrollTransducer::parse_with(&bytes, 2, scroll::LE).unwrap();
/// for reloc in lt.into_iter() {
///   assert_eq!(reloc.r_info, 5);
///   println!("{:?}", reloc);
/// }
/// # }
/// ```
pub type ScrollTransducer<'a, Output, Ctx = scroll::Endian> = LazyTransducer<'a, (&'a[u8], Ctx), Output>;

impl<'a, Output, Ctx, E> ScrollTransducer<'a, Output, Ctx>
    where
        Ctx: Copy + Default,
        Output: 'a + ctx::TryFromCtx<'a, Ctx, Error = E, Size = usize> + SizeWith<Ctx, Units = usize>,
        E: From<scroll::Error> + Debug,
{
    /// The transducer is just `pread`, whose impl is defined by the user, or via derive macro.
    /// We unwrap because we bounds checked on creation
    fn pread((input, ctx): (&'a [u8], Ctx), idx: usize) -> Output {
        let offset = Output::size_with(&ctx) * idx;
        input.pread_with(offset, ctx).unwrap()
    }
    /// Create a new scroll-based lazy transducer,
    /// using the given context to parse `count` elements out of `contents`
    ///
    /// # Example
    ///
    /// ```rust
    /// extern crate lazy_transducer;
    /// extern crate rayon;
    /// use lazy_transducer::{ScrollTransducer, Endian};
    /// use rayon::prelude::*;
    ///
    /// # fn main() {
    /// let bytes = vec![1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 0xef, 0xbe, 0xad, 0xde];
    /// let lt: ScrollTransducer<u32> = ScrollTransducer::parse_with(&bytes, 4, Endian::Little).unwrap();
    ///
    /// let deadbeef = lt.get(3).expect("has 4 elements");
    /// assert_eq!(deadbeef, 0xdeadbeef);
    ///
    /// lt.into_par_iter().for_each(|n| {
    ///   println!("{:?}", n);
    /// });
    /// # }
    /// ```
    pub fn parse_with(contents: &'a [u8],
                      count: usize,
                      ctx: Ctx,
    ) -> Result<Self, Error>
    {
        let sizeof_element = Output::size_with(&ctx);
        let total_size = sizeof_element * count;
        if total_size > contents.len() {
            Err(TransducerError::ElementOverflow{ nelements: count, sizeof_element, src_size: contents.len() }.into())
        } else {
            Ok(LazyTransducer {
                contents: (contents, ctx),
                count,
                transducer: Self::pread,
                _marker: PhantomData::default(),
            })
        }
    }
}

impl<'a, Input: Copy, Output> Clone for LazyTransducer<'a, Input, Output> {
    fn clone(&self) -> Self {
        LazyTransducer {
            count: self.count,
            contents: self.contents,
            transducer: self.transducer.clone(),
            _marker: PhantomData::default(),
        }
    }
}

/// A generic iterator over the elements produced by the lazy transducer
pub struct IntoIter<'a, Input: 'a + Copy, Output: 'a> {
    current: usize,
    lt: LazyTransducer<'a, Input, Output>,
}

impl<'a, Input: Copy, Output> Iterator for IntoIter<'a, Input, Output> {
    type Item = Output;
    fn next (&mut self) -> Option<Self::Item> {
        if self.current >= self.lt.count {
            None
        } else {
            let output = self.lt.get(self.current);
            self.current += 1;
            output
        }
    }
}

impl<'a, Input: Copy, Output> IntoIterator for LazyTransducer<'a, Input, Output> {
    type Item = Output;
    type IntoIter = IntoIter<'a, Input, Output>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            current: 0,
            lt: self,
        }
    }
}

impl<'a, 'b, Input: Copy, Output> IntoIterator for &'b LazyTransducer<'a, Input, Output> {
    type Item = Output;
    type IntoIter = IntoIter<'a, Input, Output>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            current: 0,
            lt: self.clone(),
        }
    }
}

impl<'a, Input: Copy, Output> ExactSizeIterator for IntoIter<'a, Input, Output> {
    fn len(&self) -> usize {
        self.lt.count
    }
}

/// A generic, parallel iterator over the elements produced by the lazy transducer.
///
/// This implements rayon's ParallelIterator trait, so you need only use `rayon::prelude::*` and
/// iterate in parallel via the `into_par_iter()` method.
pub struct IntoParIter<'a, Input: 'a + Copy, Output: 'a> {
    current: usize,
    lt: LazyTransducer<'a, Input, Output>,
}

impl<'a, Input: Sync + Copy + Send, Output: Send + Sync> IntoParallelIterator for LazyTransducer<'a, Input, Output> {
    type Iter = IntoParIter<'a, Input, Output>;
    type Item = Output;

    fn into_par_iter(self) -> Self::Iter {
        IntoParIter {
            current: 0,
            lt: self,
        }
    }
}

impl<'a, Input: Sync + Send + Copy, Output: Send + Sync> ParallelIterator for IntoParIter<'a, Input, Output> {
    type Item = Output;
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item> {
        bridge_unindexed(Producer { top: self.lt.count, lt: &self.lt, current: self.current }, consumer)
    }
}

/// The parallel iterator producer for a lazy transducer, required by rayon.
pub struct Producer<'b, 'a: 'b, Input: 'a + Sync + Copy + Send, Output: 'a + Send + Sync> {
    lt: &'b LazyTransducer<'a, Input, Output>,
    current: usize,
    top: usize,
}

impl<'b, 'a, Input: Sync + Copy + Send, Output: Send + Sync> Iterator for Producer<'b, 'a, Input, Output> {
    type Item = Output;
    fn next (&mut self) -> Option<Self::Item> {
        if self.current >= self.top || self.current >= self.lt.count {
            None
        } else {
            let output = self.lt.get(self.current);
            self.current += 1;
            output
        }
    }
}

impl<'b, 'a, Input: Send + Sync + Copy, Output: Sync + Send> UnindexedProducer for Producer<'b, 'a, Input, Output> {
    type Item = Output;

    fn split(mut self) -> (Self, Option<Self>) {
        let len = self.top - self.current;
        if len > 1 {
            let old_top = self.top;
            let split_len = len / 2;
            self.top = self.current + split_len;
            let right = Producer { lt: self.lt, current: self.top, top: old_top };
            (self, Some(right))
        } else {
            (self, None)
        }
    }

    fn fold_with<F>(self, folder: F) -> F
        where F: Folder<Self::Item>
    {
        folder.consume_iter(self.into_iter())
    }
}
