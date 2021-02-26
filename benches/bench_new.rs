#![feature(test)]
extern crate test;
use test::Bencher;

use core::hint::black_box;
use isbn2::*;

#[bench]
fn bench_calculate_check_digit_10(b: &mut Bencher) {
    b.iter(|| black_box(Isbn10::new(black_box([9, 9, 7, 1, 5, 0, 2, 1, 0, 0]))))
}

#[bench]
fn bench_calculate_check_digit_13(b: &mut Bencher) {
    b.iter(|| {
        black_box(Isbn13::new(black_box([
            9, 7, 8, 3, 1, 6, 1, 4, 8, 4, 1, 0, 0,
        ])))
    })
}

#[bench]
fn bench_convert(b: &mut Bencher) {
    let a = black_box(Isbn10::new([9, 9, 7, 1, 5, 0, 2, 1, 0, 0]).unwrap());
    b.iter(|| black_box(Isbn13::from(a)))
}
