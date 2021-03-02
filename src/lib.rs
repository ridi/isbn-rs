#![no_std]
//! A library for handling [International Standard Book Number], or ISBNs.
//!
//! # Examples
//!
//! ```
//! use isbn::{Isbn10, Isbn13};
//!
//! let isbn_10 = Isbn10::new(8, 9, 6, 6, 2, 6, 1, 2, 6, 4).unwrap();
//! assert_eq!(isbn_10.hyphenate().unwrap().as_str(), "89-6626-126-4");
//! assert_eq!(isbn_10.registration_group(), Ok("Korea, Republic"));
//! assert_eq!("89-6626-126-4".parse(), Ok(isbn_10));
//!
//! let isbn_13 = Isbn13::new(9, 7, 8, 1, 4, 9, 2, 0, 6, 7, 6, 6, 5).unwrap();
//! assert_eq!(isbn_13.hyphenate().unwrap().as_str(), "978-1-4920-6766-5");
//! assert_eq!(isbn_13.registration_group(), Ok("English language"));
//! assert_eq!("978-1-4920-6766-5".parse(), Ok(isbn_13));
//! ```
//!
//! [International Standard Book Number]: https://www.isbn-international.org/

use core::char;
use core::fmt;
use core::num::ParseIntError;
use core::str::FromStr;

use arrayvec::{ArrayString, ArrayVec, CapacityError};

pub type IsbnResult<T> = Result<T, IsbnError>;

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

/// An International Standard Book Number, either ISBN10 or ISBN13.
///
/// # Examples
///
/// ```
/// use isbn::{Isbn, Isbn10, Isbn13};
///
/// let isbn_10 = Isbn::_10(Isbn10::new(8, 9, 6, 6, 2, 6, 1, 2, 6, 4).unwrap());
/// let isbn_13 = Isbn::_13(Isbn13::new(9, 7, 8, 1, 4, 9, 2, 0, 6, 7, 6, 6, 5).unwrap());
///
/// assert_eq!("89-6626-126-4".parse(), Ok(isbn_10));
/// assert_eq!("978-1-4920-6766-5".parse(), Ok(isbn_13));
/// ```
#[derive(Debug, PartialEq)]
pub enum Isbn {
    _10(Isbn10),
    _13(Isbn13),
}

struct Group<'a> {
    name: &'a str,
    segment_length: usize,
}

impl Isbn {
    /// Hyphenate an ISBN into its parts:
    ///
    /// * GS1 Prefix (ISBN-13 only)
    /// * Registration group
    /// * Registrant
    /// * Publication
    /// * Check digit
    ///
    /// ```
    /// use isbn::{Isbn, Isbn10, Isbn13};
    ///
    /// let isbn_10 = Isbn::_10(Isbn10::new(8, 9, 6, 6, 2, 6, 1, 2, 6, 4).unwrap());
    /// let isbn_13 = Isbn::_13(Isbn13::new(9, 7, 8, 1, 4, 9, 2, 0, 6, 7, 6, 6, 5).unwrap());
    ///
    /// assert_eq!(isbn_10.hyphenate().unwrap().as_str(), "89-6626-126-4");
    /// assert_eq!(isbn_13.hyphenate().unwrap().as_str(), "978-1-4920-6766-5");
    /// ```
    pub fn hyphenate(&self) -> Result<ArrayString<[u8; 17]>, IsbnError> {
        match *self {
            Isbn::_10(ref c) => c.hyphenate(),
            Isbn::_13(ref c) => c.hyphenate(),
        }
    }

    /// Retrieve the name of the registration group.
    ///
    /// ```
    /// use isbn::{Isbn, Isbn10, Isbn13};
    ///
    /// let isbn_10 = Isbn::_10(Isbn10::new(8, 9, 6, 6, 2, 6, 1, 2, 6, 4).unwrap());
    /// let isbn_13 = Isbn::_13(Isbn13::new(9, 7, 8, 1, 4, 9, 2, 0, 6, 7, 6, 6, 5).unwrap());
    ///
    /// assert_eq!(isbn_10.registration_group(), Ok("Korea, Republic"));
    /// assert_eq!(isbn_13.registration_group(), Ok("English language"));
    /// ```
    pub fn registration_group(&self) -> Result<&str, IsbnError> {
        match *self {
            Isbn::_10(ref c) => c.registration_group(),
            Isbn::_13(ref c) => c.registration_group(),
        }
    }
}

impl fmt::Display for Isbn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Isbn::_10(ref c) => c.fmt(f),
            Isbn::_13(ref c) => c.fmt(f),
        }
    }
}

impl From<Isbn10> for Isbn {
    fn from(isbn10: Isbn10) -> Isbn {
        Isbn::_10(isbn10)
    }
}

impl From<Isbn13> for Isbn {
    fn from(isbn13: Isbn13) -> Isbn {
        Isbn::_13(isbn13)
    }
}

impl FromStr for Isbn {
    type Err = IsbnError;
    fn from_str(s: &str) -> Result<Isbn, IsbnError> {
        Parser::new(s)?.read_isbn()
    }
}

// Insert hyphens at specified indices.
fn hyphenate(digits: &[u8], indices: &[usize]) -> ArrayString<[u8; 17]> {
    let mut hyphenated = ArrayString::<[u8; 17]>::new();
    for (i, x) in digits.iter().enumerate() {
        if indices.contains(&i) {
            hyphenated.push('-')
        }
        hyphenated.push(char::from_digit((*x).into(), 10).unwrap());
    }
    hyphenated
}

/// 10-digit ISBN format.
#[derive(Debug, PartialEq, Copy, Clone, Hash)]
pub struct Isbn10 {
    digits: [u8; 10],
}

impl Isbn10 {
    /// Creates a new ISBN10 code from 10 digits.
    ///
    /// # Examples
    ///
    /// ```
    /// use isbn::Isbn10;
    ///
    /// let isbn10 = Isbn10::new(8, 9, 6, 6, 2, 6, 1, 2, 6, 4).unwrap();
    /// ```
    pub fn new(
        a: u8,
        b: u8,
        c: u8,
        d: u8,
        e: u8,
        f: u8,
        g: u8,
        h: u8,
        i: u8,
        j: u8,
    ) -> IsbnResult<Isbn10> {
        let digits = [a, b, c, d, e, f, g, h, i, j];
        if Isbn10::calculate_check_digit(&digits) == j {
            Ok(Isbn10 { digits })
        } else {
            Err(IsbnError::InvalidChecksum)
        }
    }

    /// Convert ISBN-13 to ISBN-10, if applicable.
    ///
    /// ```
    /// use isbn::{Isbn10, Isbn13};
    ///
    /// let isbn_13 = Isbn13::new(9, 7, 8, 1, 4, 9, 2, 0, 6, 7, 6, 6, 5).unwrap();
    /// assert_eq!(Isbn10::try_from(isbn_13), "1-4920-6766-0".parse());
    /// ```
    pub fn try_from(isbn13: Isbn13) -> IsbnResult<Self> {
        let d = isbn13.digits;
        if d[..3] != [9, 7, 8] {
            Err(IsbnError::InvalidConversion)
        } else {
            let c = Isbn10::calculate_check_digit(&isbn13.digits[3..]);
            Isbn10::new(d[3], d[4], d[5], d[6], d[7], d[8], d[9], d[10], d[11], c)
        }
    }

    fn calculate_check_digit(digits: &[u8]) -> u8 {
        let sum: usize = digits
            .iter()
            .enumerate()
            .take(9)
            .map(|(i, &d)| d as usize * (10 - i))
            .sum();
        let check_digit = (11 - (sum % 11)) % 11;
        check_digit as u8
    }

    /// Hyphenate an ISBN-10 into its parts:
    ///
    /// * Registration group
    /// * Registrant
    /// * Publication
    /// * Check digit
    ///
    /// ```
    /// use isbn::Isbn10;
    ///
    /// let isbn_10 = Isbn10::new(8, 9, 6, 6, 2, 6, 1, 2, 6, 4).unwrap();
    /// assert_eq!(isbn_10.hyphenate().unwrap().as_str(), "89-6626-126-4");
    /// ```
    pub fn hyphenate(&self) -> Result<ArrayString<[u8; 17]>, IsbnError> {
        let registration_group_segment_length =
            Isbn::get_ean_ucc_group("978", self.segment(0))?.segment_length;
        let registrant_segment_length = Isbn::get_registration_group(
            &self.group_prefix(registration_group_segment_length),
            self.segment(registration_group_segment_length),
        )?
        .segment_length;
        let hyphen_at = [
            registration_group_segment_length,
            registration_group_segment_length + registrant_segment_length,
            9,
        ];

        Ok(hyphenate(&self.digits, &hyphen_at))
    }

    /// Retrieve the name of the registration group.
    ///
    /// ```
    /// use isbn::Isbn10;
    ///
    /// let isbn_10 = Isbn10::new(8, 9, 6, 6, 2, 6, 1, 2, 6, 4).unwrap();
    /// assert_eq!(isbn_10.registration_group(), Ok("Korea, Republic"));
    /// ```
    pub fn registration_group(&self) -> Result<&str, IsbnError> {
        let registration_group_segment_length =
            Isbn::get_ean_ucc_group("978", self.segment(0))?.segment_length;

        Ok(Isbn::get_registration_group(
            &self.group_prefix(registration_group_segment_length),
            self.segment(registration_group_segment_length),
        )?
        .name)
    }

    fn segment(&self, base: usize) -> u32 {
        (0..7).fold(0, |s, i| {
            s + u32::from(*self.digits.get(base + i).unwrap_or(&0)) * 10_u32.pow(6 - i as u32)
        })
    }

    fn group_prefix(&self, length: usize) -> ArrayString<[u8; 17]> {
        Isbn13::from(*self).group_prefix(length)
    }
}

impl fmt::Display for Isbn10 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for x in &self.digits {
            match x {
                10 => write!(f, "X")?,
                _ => write!(f, "{}", x)?,
            }
        }
        Ok(())
    }
}

impl FromStr for Isbn10 {
    type Err = IsbnError;
    fn from_str(s: &str) -> Result<Isbn10, IsbnError> {
        let mut p = Parser::new(s)?;
        if p.digits.len() != 10 {
            Err(IsbnError::InvalidLength)
        } else {
            p.read_isbn10()
        }
    }
}

/// 13-digit ISBN format.
#[derive(Debug, PartialEq, Copy, Clone, Hash)]
pub struct Isbn13 {
    digits: [u8; 13],
}

impl Isbn13 {
    /// Creates a new ISBN13 code from 13 digits.
    ///
    /// # Examples
    ///
    /// ```
    /// use isbn::Isbn13;
    ///
    /// let isbn13 = Isbn13::new(9, 7, 8, 1, 4, 9, 2, 0, 6, 7, 6, 6, 5).unwrap();
    /// ```
    pub fn new(
        a: u8,
        b: u8,
        c: u8,
        d: u8,
        e: u8,
        f: u8,
        g: u8,
        h: u8,
        i: u8,
        j: u8,
        k: u8,
        l: u8,
        m: u8,
    ) -> IsbnResult<Isbn13> {
        let digits = [a, b, c, d, e, f, g, h, i, j, k, l, m];
        if Isbn13::calculate_check_digit(&digits) == m {
            Ok(Isbn13 { digits })
        } else {
            Err(IsbnError::InvalidChecksum)
        }
    }

    fn calculate_check_digit(digits: &[u8]) -> u8 {
        let sum: usize = digits
            .iter()
            .enumerate()
            .take(12)
            .map(|(i, &d)| d as usize * (3 - 2 * ((i + 1) % 2)))
            .sum();
        let check_digit = (10 - (sum % 10)) % 10;
        check_digit as u8
    }

    fn ean_ucc_group(&self) -> Result<Group, IsbnError> {
        let mut s = ArrayString::<[u8; 3]>::new();
        for i in 0..3 {
            s.push(char::from_digit(self.digits[i].into(), 10).unwrap());
        }
        Isbn::get_ean_ucc_group(&s, self.segment(0))
    }

    /// Hyphenate an ISBN-13 into its parts:
    ///
    /// * GS1 Prefix
    /// * Registration group
    /// * Registrant
    /// * Publication
    /// * Check digit
    ///
    /// ```
    /// use isbn::Isbn13;
    ///
    /// let isbn_13 = Isbn13::new(9, 7, 8, 1, 4, 9, 2, 0, 6, 7, 6, 6, 5).unwrap();
    /// assert_eq!(isbn_13.hyphenate().unwrap().as_str(), "978-1-4920-6766-5");
    /// ```
    pub fn hyphenate(&self) -> Result<ArrayString<[u8; 17]>, IsbnError> {
        let registration_group_segment_length = self.ean_ucc_group()?.segment_length;
        let registrant_segment_length = Isbn::get_registration_group(
            &self.group_prefix(registration_group_segment_length),
            self.segment(registration_group_segment_length),
        )?
        .segment_length;
        let hyphen_at = [
            3,
            3 + registration_group_segment_length,
            3 + registration_group_segment_length + registrant_segment_length,
            12,
        ];

        Ok(hyphenate(&self.digits, &hyphen_at))
    }

    /// Retrieve the name of the registration group.
    ///
    /// ```
    /// use isbn::Isbn13;
    ///
    /// let isbn_13 = Isbn13::new(9, 7, 8, 1, 4, 9, 2, 0, 6, 7, 6, 6, 5).unwrap();
    /// assert_eq!(isbn_13.registration_group(), Ok("English language"));
    /// ```
    pub fn registration_group(&self) -> Result<&str, IsbnError> {
        let registration_group_segment_length = self.ean_ucc_group()?.segment_length;

        Ok(Isbn::get_registration_group(
            &self.group_prefix(registration_group_segment_length),
            self.segment(registration_group_segment_length),
        )?
        .name)
    }

    fn segment(&self, base: usize) -> u32 {
        (3..9).fold(0, |s, i| {
            s + u32::from(*self.digits.get(base + i).unwrap_or(&0)) * 10_u32.pow(9 - i as u32)
        })
    }

    fn group_prefix(&self, length: usize) -> ArrayString<[u8; 17]> {
        hyphenate(&self.digits[0..length + 3], &[3])
    }
}

impl fmt::Display for Isbn13 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for x in &self.digits {
            write!(f, "{}", x)?;
        }
        Ok(())
    }
}

impl From<Isbn10> for Isbn13 {
    fn from(isbn10: Isbn10) -> Isbn13 {
        let mut v = ArrayVec::<[u8; 13]>::new();
        v.extend([9, 7, 8].iter().cloned());
        v.extend(isbn10.digits[..9].iter().cloned());
        let c = Isbn13::calculate_check_digit(&v);
        let d = isbn10.digits;
        Isbn13::new(
            9, 7, 8, d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7], d[8], c,
        )
        .unwrap()
    }
}

impl FromStr for Isbn13 {
    type Err = IsbnError;
    fn from_str(s: &str) -> Result<Isbn13, IsbnError> {
        let mut p = Parser::new(s)?;
        if p.digits.len() != 13 {
            Err(IsbnError::InvalidLength)
        } else {
            p.read_isbn13()
        }
    }
}

/// An error which can be returned when parsing an ISBN.
#[derive(Debug, PartialEq)]
pub enum IsbnError {
    /// The given string is too short or too long to be an ISBN.
    InvalidLength,
    /// Encountered an invalid digit while parsing.
    InvalidDigit,
    /// Encountered an invalid ISBN registration group.
    InvalidGroup,
    /// Encountered a range not defined for use at this time.
    UndefinedRange,
    /// Failed to validate checksum.
    InvalidChecksum,
    /// Failed to convert to ISBN10.
    InvalidConversion,
}

impl fmt::Display for IsbnError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            IsbnError::InvalidLength => write!(
                f,
                "The given string is too short or too long to be an ISBN."
            ),
            IsbnError::InvalidDigit => write!(f, "Encountered an invalid digit while parsing."),
            IsbnError::InvalidGroup => write!(f, "Encountered an invalid ISBN registration group."),
            IsbnError::UndefinedRange => {
                write!(f, "Encountered a range not defined for use at this time.")
            }
            IsbnError::InvalidChecksum => write!(f, "Failed to validate checksum."),
            IsbnError::InvalidConversion => write!(f, "Failed to convert to ISBN10."),
        }
    }
}

impl From<ParseIntError> for IsbnError {
    fn from(_: ParseIntError) -> Self {
        IsbnError::InvalidDigit
    }
}

impl From<CapacityError<u8>> for IsbnError {
    fn from(_: CapacityError<u8>) -> Self {
        IsbnError::InvalidLength
    }
}

#[derive(Debug, Clone)]
struct Parser {
    digits: ArrayVec<[u8; 13]>,
}

impl Parser {
    pub fn new(s: &str) -> Result<Parser, IsbnError> {
        let mut digits = ArrayVec::new();
        let mut has_x = false;

        for c in s.chars() {
            match c {
                '-' | ' ' => {},
                'X' => if digits.len() == 9 {
                    has_x = true;
                    digits.push(10);
                } else {
                    return Err(IsbnError::InvalidDigit);
                },
                '0'..='9' => if has_x {
                    return Err(IsbnError::InvalidDigit);
                } else {
                    digits.try_push(c.to_digit(10).unwrap() as u8)?
                },
                _ => return Err(IsbnError::InvalidDigit),
            }
        }

        Ok(Parser { digits })
    }

    fn read_isbn(&mut self) -> Result<Isbn, IsbnError> {
        match self.digits.len() {
            10 => self.read_isbn10().map(Isbn::_10),
            13 => self.read_isbn13().map(Isbn::_13),
            _ => Err(IsbnError::InvalidLength),
        }
    }

    fn read_isbn13(&mut self) -> Result<Isbn13, IsbnError> {
        let check_digit = Isbn13::calculate_check_digit(&self.digits);
        if check_digit == *self.digits.last().unwrap() {
            let mut a = [0u8; 13];
            a.clone_from_slice(&self.digits[..13]);
            Ok(Isbn13 { digits: a })
        } else {
            Err(IsbnError::InvalidDigit)
        }
    }

    fn read_isbn10(&mut self) -> Result<Isbn10, IsbnError> {
        let check_digit = Isbn10::calculate_check_digit(&self.digits);
        if check_digit == *self.digits.last().unwrap() {
            let mut a = [0; 10];
            a.clone_from_slice(&self.digits);
            Ok(Isbn10 { digits: a })
        } else {
            Err(IsbnError::InvalidDigit)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_str_isbn10() {
        // Wikipedia ISBN-10 check digit calculation example
        assert!(Isbn::from_str("0-306-40615-2").is_ok());

        // Wikipedia ISBN-10 check digit calculation invalid example
        assert!(Isbn::from_str("99999-999-9-X").is_err());

        // Wikipedia Registrant element examples
        assert!(Isbn::from_str("99921-58-10-7").is_ok());
        assert!(Isbn::from_str("9971-5-0210-0").is_ok());
        assert!(Isbn::from_str("960-425-059-0").is_ok());
        assert!(Isbn::from_str("80-902734-1-6").is_ok());
        assert!(Isbn::from_str("85-359-0277-5").is_ok());
        assert!(Isbn::from_str("1-84356-028-3").is_ok());
        assert!(Isbn::from_str("0-684-84328-5").is_ok());
        assert!(Isbn::from_str("0-8044-2957-X").is_ok());
        assert!(Isbn::from_str("0-85131-041-9").is_ok());
        assert!(Isbn::from_str("0-943396-04-2").is_ok());
        assert!(Isbn::from_str("0-9752298-0-X").is_ok());
    }

    #[test]
    fn test_from_str_isbn13() {
        // Wikipedia Example
        assert!(Isbn13::from_str("978-3-16-148410-0").is_ok());

        // Wikipedia ISBN-13 check digit calculation example
        assert!(Isbn13::from_str("978-0-306-40615-7").is_ok());
    }
    
    #[test]
    fn test_invalid_isbn_strings_no_panic() {
        assert!(Isbn::from_str("L").is_err());
        assert!(Isbn::from_str("").is_err());
        assert!(Isbn::from_str("01234567890123456789").is_err());
        assert!(Isbn::from_str("ⱧňᚥɂᛢĞžᚪ©ᛟƚ¶G").is_err());

        assert!(Isbn10::from_str("").is_err());
        assert!(Isbn10::from_str("01234567890").is_err());
        assert!(Isbn10::from_str("01234567X9").is_err());
        assert!(Isbn10::from_str("012345678").is_err());

        assert!(Isbn13::from_str("").is_err());
        assert!(Isbn13::from_str("012345678901X").is_err());
        assert!(Isbn13::from_str("01234567890X2").is_err());
        assert!(Isbn13::from_str("012345678").is_err());
        assert!(Isbn13::from_str("0123456789012345").is_err());

    }
}
