use std::fmt::Debug;
use std::marker::PhantomData;

use scroll::{self, ctx};
use scroll::ctx::SizeWith;
use failure::Error;

use {LazyTransducer, ScrollTransducer, TransducerError};

/// A builder is useful for when the transducer needs to be constructed incrementally, or
/// only when after certain information is present.
///
/// # Example
///
/// ```rust
/// use lazy_transducer::{LazyTransducer, Builder};
/// use std::mem::size_of;
/// use std::mem::transmute;
///
/// let bytes: Vec<u8> = vec![1u8, 0, 2, 0, 3, 0, 4, 0];
/// let mut builder = Builder::new(&bytes);
/// let maybe_number_of_elements = Some(4);
/// if let Some(count) = maybe_number_of_elements {
///   builder = builder.count(count);
/// }
/// // if the count was None, we'd have 0 elements, but the transducer is still constructable,
/// // similar to an empty iterator
/// let lt: LazyTransducer<_, u16> = builder.transducer(|input, index| {
///   let start = size_of::<u16>() * index;
///   unsafe { *transmute::<_, &u16>(&input[start]) }
/// });
///  // note: the data will be 1, 2, 3, 4, for little-endian machines, but not for big-endian
/// for (i, n) in lt.into_iter().enumerate() {
///   println!("{}: {}", i, n);
/// }
/// ```
pub struct Builder<'a, Input, Output>
    where Input: 'a + Copy,
          Output: 'a {
    input: Input,
    //input: Option<Input>,
    count: usize,
    //transducer: Option<fn(Input, usize) -> Output>,
    _marker: PhantomData<&'a (Input, Output)>,
}

impl<'a, Input, Output> Builder<'a, Input, Output>
    where Input: 'a + Copy,
          Output: 'a {
    // todo: do this later
//    pub fn empty() -> Self {
//        Builder {
//            input: None,
//            transducer: None,
//            count: 0,
//            _marker: PhantomData::default(),
//        }
//    }
    pub fn new(input: Input) -> Self {
        Builder {
            input,
            count: 0,
            _marker: PhantomData::default(),
        }
    }
    pub fn input(mut self, input: Input) -> Self {
        self.input = input;
        self
    }
    pub fn count(mut self, count: usize) -> Self {
        self.count = count;
        self
    }
    // TODO: make this return
    pub fn transducer(self, transducer: fn(Input, usize) -> Output) -> LazyTransducer<'a, Input, Output> {
        LazyTransducer {
            contents: self.input,
            count: self.count,
            _marker: PhantomData::default(),
            transducer,
        }
    }
}

impl<'a, Output> Builder<'a, &'a [u8], Output>
{
    ///
    pub fn parse_with<Ctx, E>(self, ctx: Ctx) -> Result<ScrollTransducer<'a, Output, Ctx>, Error>
    where
        Ctx: Default + Copy,
        E: From<scroll::Error> + Debug,
        Output: 'a + ctx::TryFromCtx<'a, Ctx, Error = E, Size = usize> + SizeWith<Ctx, Units = usize>
    {
        ScrollTransducer::parse_with(self.input, self.count, ctx)
    }
}
