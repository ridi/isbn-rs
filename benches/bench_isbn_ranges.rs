#![cfg(feature = "runtime-ranges")]
#![feature(test)]
extern crate test;
use test::Bencher;

use core::hint::black_box;
use isbn2::*;
use std::str::FromStr;

fn open_range() -> IsbnRange {
    IsbnRange::from_file("isbn-ranges/RangeMessage.xml").unwrap()
}

#[bench]
fn bench_open_range(b: &mut Bencher) {
    b.iter(|| black_box(open_range()))
}

#[bench]
fn bench_hyphenate_isbn10(b: &mut Bencher) {
    let range = open_range();
    let digits = Isbn10::new(black_box([9, 9, 7, 1, 5, 0, 2, 1, 0, 0])).unwrap();
    b.iter(|| black_box(range.hyphenate_isbn10(&digits)))
}

#[bench]
fn bench_hyphenate_isbn13(b: &mut Bencher) {
    let range = open_range();
    let digits = Isbn13::new(black_box([9, 7, 8, 3, 1, 6, 1, 4, 8, 4, 1, 0, 0])).unwrap();
    b.iter(|| black_box(range.hyphenate_isbn13(&digits)))
}
