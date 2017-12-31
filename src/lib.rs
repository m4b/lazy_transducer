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
