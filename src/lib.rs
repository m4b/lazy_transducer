extern crate rayon;
extern crate scroll;
#[macro_use]
extern crate failure;

use rayon::iter::*;
use rayon::iter::plumbing::*;
use failure::Error;

use std::fmt::Debug;
use std::marker::PhantomData;

/// The kind of errors for constructing lazy transducers
#[derive(Fail, Debug)]
pub enum TransducerError {
    #[fail(display = "Error during building: {}", _0)]
    BuilderError(String),
    #[fail(display = "Too many elements (size = {} * {}) requested from src of size: {}", nelements, sizeof_element, src_size)]
    ElementOverflow{ nelements: usize, sizeof_element: usize, src_size: usize },
}

mod builder;
pub use builder::*;

#[derive(Debug)]
pub struct LazyTransducer<'a, Input, Output>
    where Input: 'a + Copy,
          Output: 'a,
{
    count: usize,
    contents: Input,
    transducer: fn(Input, usize) -> Output,
    _marker: PhantomData<&'a Output>
}

impl<'a, Input, Output> LazyTransducer<'a, Input, Output>
    where Input: 'a + Copy,
          Output: 'a
{
    pub fn len(&self) -> usize {
        self.count
    }
    pub fn new(contents: Input,
               len: usize,
               transducer: fn(Input, usize) -> Output)
               -> Self
    {
        LazyTransducer {
            count: len,
            contents,
            transducer,
            _marker: PhantomData::default(),
        }
    }

    #[inline]
    pub fn get(&self, idx: usize) -> Option<Output> {
        if idx >= self.count {
            None
        } else {
            Some((self.transducer)(self.contents, idx))
        }
    }
}

use scroll::{ctx, Pread};
use scroll::ctx::SizeWith;

impl<'a, Output, Ctx: Copy + Default, E: From<scroll::Error> + Debug> LazyTransducer<'a, (&'a [u8], Ctx), Output>
    where Output: 'a + ctx::TryFromCtx<'a, Ctx, Error = E, Size = usize> + SizeWith<Ctx, Units = usize> {
    pub fn empty() -> Self {
        Self::parse_with(&[], 0, Ctx::default()).unwrap()
    }
    pub fn transducer((input, ctx): (&'a [u8], Ctx), idx: usize) -> Output {
        let off = Output::size_with(&ctx) * idx;
        input.pread_with(off, ctx).unwrap()
    }
    pub fn parse_with(contents: &'a [u8],
                      count: usize,
                      ctx: Ctx,
    ) -> Result<Self, Error>
    {
        let sizeof_element = Output::size_with(&ctx);
        let total_size = sizeof_element * count;
        if total_size > contents.len() && total_size != 0 {
            Err(TransducerError::ElementOverflow{ nelements: count, sizeof_element, src_size: contents.len() }.into())
        } else {
            Ok(LazyTransducer {
                contents: (contents, ctx),
                count: count,
                transducer: Self::transducer,
                _marker: PhantomData::default(),
            })
        }
    }
}

pub type ScrollTransducer<'a, Output, Ctx = scroll::Endian> = LazyTransducer<'a, (&'a[u8], Ctx), Output>;

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
            //println!("({} - {:?}) left {:?} - right {:?}", len, og, self.current..self.top, self.top..old_top);
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

#[cfg(test)]
mod tests {
    extern crate scroll;
    use super::*;
    use tests::scroll::Pread;
    #[derive(Debug, Default)]
    struct Derp {
        one: usize,
        two: f64,
    }

    #[test]
    fn simple_lazy_transducer() {
        let bytes = b"sixteen candles";
        let i = 0;
        let transducer = |input, pos| {
            input.pread::<&str>(pos)
        };
        let mut lt = LazyTransducer::new(bytes, bytes.len(), transducer);
        for (s, res) in lt.into_iter().enumerate() {
            println!("{}: {:?}", &s, res);
            assert!(true);
        }
        assert!(true)
    }
}
