#![feature(test)]
extern crate test;
use test::Bencher;

use core::hint::black_box;
use isbn2::*;
use std::str::FromStr;

#[bench]
fn bench_hyphenate_isbn10(b: &mut Bencher) {
    let digits = Isbn10::new(black_box([9, 9, 7, 1, 5, 0, 2, 1, 0, 0])).unwrap();
    b.iter(|| black_box(digits.hyphenate()))
}

#[bench]
fn bench_hyphenate_isbn13(b: &mut Bencher) {
    let digits = Isbn13::new(black_box([9, 7, 8, 3, 1, 6, 1, 4, 8, 4, 1, 0, 0])).unwrap();
    b.iter(|| black_box(digits.hyphenate()))
}

#[bench]
fn bench_to_string_isbn10(b: &mut Bencher) {
    let digits = Isbn10::new(black_box([9, 9, 7, 1, 5, 0, 2, 1, 0, 0])).unwrap();
    b.iter(|| black_box(digits.to_string()))
}

#[bench]
fn bench_to_string_isbn13(b: &mut Bencher) {
    let digits = Isbn13::new(black_box([9, 7, 8, 3, 1, 6, 1, 4, 8, 4, 1, 0, 0])).unwrap();
    b.iter(|| black_box(digits.to_string()))
}

#[bench]
fn bench_from_string_isbn10(b: &mut Bencher) {
    let str = black_box("85-359-0277-5");
    b.iter(|| black_box(Isbn10::from_str(str)))
}

#[bench]
fn bench_from_string_isbn13(b: &mut Bencher) {
    let str = black_box("978-3-16-148410-0");
    b.iter(|| black_box(Isbn13::from_str(str)))
}
