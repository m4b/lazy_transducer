extern crate lazy_transducer;
#[macro_use]
extern crate scroll;
extern crate rayon;

use rayon::prelude::*;
use lazy_transducer::{LazyTransducer, ScrollTransducer};

use std::mem::size_of;
use std::str;
use scroll::{Pread, IOwrite, LE, BE};
use std::io::Cursor;

#[derive(Debug, Copy, Clone, Default, Pread, Pwrite, SizeWith, IOwrite, IOread)]
#[repr(C)]
struct Derp {
    one: u64,
    two: u16,
}

#[test]
fn parallel_scroll_transducer() {
    let derp = Derp::default();
    let mut bytes = Cursor::new(Vec::new());
    bytes.iowrite(derp).unwrap();
    bytes.iowrite(derp).unwrap();
    bytes.iowrite(derp).unwrap();
    bytes.iowrite(derp).unwrap();
    bytes.iowrite(derp).unwrap();
    let bytes = bytes.into_inner();
    let lt: ScrollTransducer<Derp, _> = ScrollTransducer::parse_with(&bytes, 5, LE).unwrap();
    let derps: Vec<Derp> = lt.into_par_iter().map(|mut derp| { derp.one = 0xdeadbeef; derp }).collect();
    for d in derps {
        assert_eq!(d.one, 0xdeadbeef);
    }
}

#[derive(Debug, Default)]
struct Derp2 {
    one: usize,
    two: f64,
}

fn get_str(input: &[u8; 15], pos: usize) -> &str {
    input.pread::<&str>(pos).unwrap()
}

#[test]
fn lifetime_transducer() {
    let bytes = b"sixteen candles";
    let lt: LazyTransducer<&[u8; 15], &str> = LazyTransducer::new(bytes, bytes.len(), get_str);
    for (idx, res) in lt.into_iter().enumerate() {
        println!("{}: {:?}", idx, res);
        assert!(true);
    }
}

#[test]
fn basic_transducer() {
    let bytes: Vec<u8> = vec![
        0, 0, 0, 0,
        0, 0, 0, 1,
        0, 0, 0, 2,
        0, 0, 0, 3,
        0, 0, 0, 4,
        0, 0, 0, 5,
        0, 0, 0, 6,
        0, 0, 0, 7,
        0, 0, 0, 8,
        0, 0, 0, 9,
        0, 0, 0, 10,
        0, 0, 0, 11,
        0, 0, 0, 12,
        0, 0, 0, 13,
        0, 0, 0, 14,
        0, 0, 0, 15,
        0, 0, 0, 16,
        0, 0, 0, 17,
        0, 0, 0, 18,
        0xde, 0xad, 0xbe, 0xef,
    ];
    let lt: LazyTransducer<&[u8], u32> = LazyTransducer::new(&bytes, bytes.len() / size_of::<u32>(), |input, idx| {
        let offset = size_of::<u32>() * idx;
        input.pread_with::<u32>(offset, BE).unwrap()
    });

    let deadbeef = lt.get(19).unwrap();
    assert_eq!(deadbeef, 0xdeadbeefu32);
    for (i, n) in (&lt).into_iter().enumerate() {
        if i != lt.len() - 1 {
            assert_eq!(i as u32, n);
        } else {
            assert_eq!(0xdeadbeef, n);
        }
    }

    let ns1: Vec<_> = lt.clone().into_par_iter().collect();
    let ns2: Vec<_> = lt.clone().into_iter().collect();
    assert_eq!(ns1.len(), ns2.len());
}
