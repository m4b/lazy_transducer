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

    /// Get an element out of the lazy transducer
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
/// ```no_test, rust
/// //extern crate lazy_transducer;
/// //#[macro_use]
/// //extern crate scroll;
/// use lazy_transducer;
/// //use scroll;
/// use lazy_transducer::ScrollTransducer;
///
/// #[repr(C)]
/// #[derive(Debug, Clone, Copy, PartialEq, Default)]
/// //#[derive(Pread, Pwrite, SizeWith)]
/// pub struct Rel {
///     pub r_offset: u32,
///     pub r_info: u32,
/// }
///
/// let bytes = vec![0, 0, 0, 4, 0, 0, 0, 5, 0, 0, 0, 1, 0, 0, 0, 5];
/// let lt: ScrollTransducer<Rel> = ScrollTransducer::parse_with(&bytes, 2, scroll::LE).unwrap();
/// for reloc in lt.into_iter() {
///   assert_eq!(reloc.r_info, 5);
///   println!("{:?}", reloc);
/// }
///
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
    /// use lazy_transducer::{ScrollTransducer, Endian};
    /// //extern crate rayon;
    /// //use rayon::prelude::*;
    ///
    /// let bytes = vec![1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 0xde, 0xad, 0xbe, 0xef];
    /// let lt = ScrollTransducer::parse_with(&bytes, 4, Endian::Little);
    /// lt.into_iter().for_each(|n| {
    ///   println!("{}", n);
    /// });
    /// assert!(false);
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

pub struct IntoIter<'a, Input: 'a + Copy, Output: 'a> {
    current: usize,
    lt: LazyTransducer<'a, Input, Output>,
}

pub struct Iter<'b, 'a: 'b, Input: 'a + Copy, Output: 'a> {
    current: usize,
    lt: &'b LazyTransducer<'a, Input, Output>,
}

impl<'a, 'b, Input: Copy, Output> Iterator for Iter<'a, 'b, Input, Output> {
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
    type IntoIter = Iter<'b, 'a, Input, Output>;

    fn into_iter(self) -> Self::IntoIter {
        Iter {
            current: 0,
            lt: self,
        }
    }
}

impl<'a, Input: Copy, Output> ExactSizeIterator for IntoIter<'a, Input, Output> {
    fn len(&self) -> usize {
        self.lt.count
    }
}

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
