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
//        let bytes = b"sixteen candles";
//        let i = 0;
//        let transducer = |input: &[u8; 15], pos| {
//            input.pread::<&str>(pos).unwrap()
//        };
//        let mut lt: LazyTransducer<&[u8; 15], &str> = LazyTransducer::new(bytes, bytes.len(), transducer);
//        for (s, res) in lt.into_iter().enumerate() {
//            println!("{}: {:?}", &s, res);
//            assert!(true);
//        }
//        assert!(true)
    }
}
