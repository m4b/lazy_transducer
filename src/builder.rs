use std::fmt::Debug;
use std::marker::PhantomData;

use scroll::{self, ctx};
use scroll::ctx::SizeWith;
use failure::Error;

use {LazyTransducer, ScrollTransducer};

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
    pub fn parse_with<Ctx, E>(self, ctx: Ctx) -> Result<ScrollTransducer<'a, Output, Ctx>, Error>

    where
        Ctx: Default + Copy,
        E: From<scroll::Error> + Debug,
        Output: 'a + ctx::TryFromCtx<'a, Ctx, Error =E, Size = usize> + SizeWith<Ctx, Units = usize>
    {
        ScrollTransducer::parse_with(self.input, self.count, ctx)
    }
}
