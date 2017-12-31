use std::fmt::Debug;
use std::marker::PhantomData;

use scroll::{self, ctx};
use scroll::ctx::SizeWith;
use failure::Error;

use {LazyTransducer, ScrollTransducer, TransducerError};

/// A builder is useful for when the transducer needs to be constructed incrementally, i.e.,
/// certain information is present later on, or is optional, etc.
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
///     let start = size_of::<u16>() * index;
///     unsafe { *transmute::<_, &u16>(&input[start]) }
///   })
///   .finish()
///   .unwrap();
///
///  // note: the data will be 1, 2, 3, 4, for little-endian machines, but not for big-endian
/// for (i, n) in lt.into_iter().enumerate() {
///   println!("{}: {}", i, n);
/// }
/// ```
pub struct Builder<'a, Input, Output>
    where Input: 'a + Copy,
          Output: 'a {
    input: Option<Input>,
    count: usize,
    transducer: Option<fn(Input, usize) -> Output>,
    _marker: PhantomData<&'a (Input, Output)>,
}

impl<'a, Input, Output> Builder<'a, Input, Output>
    where Input: 'a + Copy,
          Output: 'a {
    /// Creates an empty builder; you must set the input and transducer before calling `finish`
    /// otherwise this is a runtime error.
    pub fn empty() -> Self {
        Builder {
            input: None,
            count: 0,
            transducer: None,
            _marker: PhantomData::default(),
        }
    }
    /// Create a new builder with the given `input`; you must set the transducer before calling `finish`
    /// otherwise this is a runtime error.
    pub fn new(input: Input) -> Self {
        Builder {
            input: Some(input),
            count: 0,
            transducer: None,
            _marker: PhantomData::default(),
        }
    }
    /// Set (or reset) the input.
    pub fn input(mut self, input: Input) -> Self {
        self.input = Some(input);
        self
    }
    /// Set the number of output elements in the input source.
    pub fn count(mut self, count: usize) -> Self {
        self.count = count;
        self
    }
    /// Set the transducer from input source to output elements.
    pub fn transducer(mut self, transducer: fn(Input, usize) -> Output) -> Self {
        self.transducer = Some(transducer);
        self
    }
    /// Finish building the lazy transducer, and return it; if the input source or the transducer is missing
    /// this is a runtime error.
    pub fn finish(self) -> Result<LazyTransducer<'a, Input, Output>, Error> {
        let contents = self.input.ok_or(TransducerError::BuilderError("No input given".to_string()))?;
        let transducer = self.transducer.ok_or(TransducerError::BuilderError("No transducer given".to_string()))?;
        Ok(LazyTransducer {
                contents,
                count: self.count,
                transducer,
                _marker: PhantomData::default(),
        })
    }
}

impl<'a, Output> Builder<'a, &'a [u8], Output>
{
    /// Create a scroll-based transducer with the given parsing `ctx`.
    pub fn parse_with<Ctx, E>(self, ctx: Ctx) -> Result<ScrollTransducer<'a, Output, Ctx>, Error>
    where
        Ctx: Default + Copy,
        E: From<scroll::Error> + Debug,
        Output: 'a + ctx::TryFromCtx<'a, Ctx, Error = E, Size = usize> + SizeWith<Ctx, Units = usize>
    {
        let input = self.input.ok_or(TransducerError::BuilderError("No input given".to_string()))?;
        ScrollTransducer::parse_with(input, self.count, ctx)
    }
}
