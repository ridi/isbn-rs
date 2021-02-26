use std::fs::File;
use std::io::BufReader;
use std::num::NonZeroUsize;
use std::path::Path;
use std::str::FromStr;

use arrayvec::ArrayString;
use indexmap::IndexMap;
use quick_xml::{events::Event, Reader};

use crate::{Group, Isbn, Isbn10, Isbn13, IsbnError, IsbnObject};

struct Segment {
    name: String,
    // (start, stop, ?length).
    ranges: Vec<((u32, u32), Option<NonZeroUsize>)>,
}

pub struct IsbnRange {
    source: Option<String>,
    serial_number: Option<String>,
    date: String,
    ean_ucc_group: IndexMap<u16, Segment>,
    registration_group: IndexMap<(u16, u32), Segment>,
}

#[derive(Debug)]
pub enum IsbnRangeError {
    NoIsbnRangeMessageTag,
    NoEanUccPrefixes,
    NoEanUccPrefix,
    NoRegistrationGroups,
    NoGroup,
    NoMessageDate,
    PrefixTooLong,
    InvalidPrefixChar,
    BadLengthString,
    LengthTooLarge,
    BadRange,
    NoDashInRange,
    Xml(quick_xml::Error),
    WrongXmlStart,
    MissingXmlStart,
    WrongXmlBody,
    WrongXmlEnd,
    MissingXmlEnd,
    FileError(std::io::Error),
}

impl From<quick_xml::Error> for IsbnRangeError {
    fn from(e: quick_xml::Error) -> Self {
        Self::Xml(e)
    }
}

impl From<std::io::Error> for IsbnRangeError {
    fn from(e: std::io::Error) -> Self {
        Self::FileError(e)
    }
}

fn read_xml_tag(
    reader: &mut Reader<BufReader<File>>,
    buf: &mut Vec<u8>,
    name: &[u8],
) -> Result<String, IsbnRangeError> {
    match reader.read_event(buf)? {
        Event::Start(e) => {
            if e.name() != name {
                return Err(IsbnRangeError::WrongXmlStart);
            }
        }
        _ => return Err(IsbnRangeError::MissingXmlStart),
    };
    buf.clear();
    let res = match reader.read_event(buf)? {
        Event::Text(e) => e.unescape_and_decode(&reader)?,
        _ => return Err(IsbnRangeError::WrongXmlBody),
    };
    match reader.read_event(buf)? {
        Event::End(e) => {
            if e.name() != name {
                return Err(IsbnRangeError::WrongXmlEnd);
            }
        }
        _ => return Err(IsbnRangeError::MissingXmlEnd),
    };
    buf.clear();
    Ok(res)
}

fn read_xml_start(
    reader: &mut Reader<BufReader<File>>,
    buf: &mut Vec<u8>,
    name: &[u8],
) -> Result<bool, IsbnRangeError> {
    if let Event::Start(e) = reader.read_event(buf)? {
        let res = e.name() == name;
        buf.clear();
        Ok(res)
    } else {
        buf.clear();
        Ok(false)
    }
}

impl Segment {
    fn from_reader(
        reader: &mut Reader<BufReader<File>>,
        buf: &mut Vec<u8>,
    ) -> Result<Self, IsbnRangeError> {
        let name = read_xml_tag(reader, buf, b"Agency")?;
        let mut ranges = Vec::new();

        match reader.read_event(buf)? {
            Event::Start(e) => {
                if e.name() != b"Rules" {
                    return Err(IsbnRangeError::WrongXmlStart);
                }
            }
            _ => return Err(IsbnRangeError::MissingXmlStart),
        };
        buf.clear();

        loop {
            match reader.read_event(buf)? {
                Event::Start(e) => {
                    if e.name() != b"Rule" {
                        return Err(IsbnRangeError::WrongXmlStart);
                    }
                }
                Event::End(e) => {
                    if e.name() == b"Rules" {
                        break;
                    }
                }
                _ => return Err(IsbnRangeError::MissingXmlStart),
            };
            buf.clear();

            let range = read_xml_tag(reader, buf, b"Range")?;
            let length = read_xml_tag(reader, buf, b"Length")?;

            ranges.push((
                {
                    let mid = range.find('-').ok_or(IsbnRangeError::NoDashInRange)?;
                    let (a, b) = range.split_at(mid);
                    (
                        u32::from_str(a).map_err(|_| IsbnRangeError::BadRange)?,
                        u32::from_str(b.split_at(1).1).map_err(|_| IsbnRangeError::BadRange)?,
                    )
                },
                {
                    if length.len() != 1 {
                        return Err(IsbnRangeError::BadLengthString);
                    }
                    let length = usize::from_str_radix(&length, 10)
                        .map_err(|_| IsbnRangeError::BadLengthString)?;
                    if length > 7 {
                        return Err(IsbnRangeError::LengthTooLarge);
                    }
                    NonZeroUsize::new(length)
                },
            ));

            match reader.read_event(buf)? {
                Event::End(e) => {
                    if e.name() != b"Rule" {
                        return Err(IsbnRangeError::WrongXmlEnd);
                    }
                }
                _ => return Err(IsbnRangeError::MissingXmlEnd),
            };
            buf.clear();
        }

        match reader.read_event(buf)? {
            Event::End(e) => match e.name() {
                b"EAN.UCC" | b"Group" => {}
                _ => return Err(IsbnRangeError::WrongXmlEnd),
            },
            _ => return Err(IsbnRangeError::MissingXmlEnd),
        };
        buf.clear();

        Ok(Segment { name, ranges })
    }

    fn group(&self, segment: u32) -> Result<Group, IsbnError> {
        for ((start, stop), length) in &self.ranges {
            if segment >= *start && segment < *stop {
                let segment_length = usize::from(length.ok_or(IsbnError::UndefinedRange)?);
                return Ok(Group {
                    name: &self.name,
                    segment_length,
                });
            }
        }
        Err(IsbnError::InvalidGroup)
    }
}
impl IsbnRange {
    fn read_ean_ucc_group(
        reader: &mut Reader<BufReader<File>>,
        buf: &mut Vec<u8>,
    ) -> Result<IndexMap<u16, Segment>, IsbnRangeError> {
        buf.clear();
        let mut res = IndexMap::new();
        loop {
            match reader.read_event(buf)? {
                Event::Start(e) => {
                    if e.name() != b"EAN.UCC" {
                        return Err(IsbnRangeError::NoEanUccPrefix);
                    }
                }
                Event::End(e) if e.name() == b"EAN.UCCPrefixes" => {
                    return Ok(res);
                }
                _ => return Err(IsbnRangeError::WrongXmlEnd),
            };
            buf.clear();

            let mut prefix_val = 0u16;
            for (i, char) in read_xml_tag(reader, buf, b"Prefix")?.chars().enumerate() {
                if i == 3 {
                    return Err(IsbnRangeError::PrefixTooLong);
                }
                prefix_val = (prefix_val << 4)
                    | char.to_digit(10).ok_or(IsbnRangeError::InvalidPrefixChar)? as u16;
            }

            res.insert(prefix_val, Segment::from_reader(reader, buf)?);
        }
    }

    fn read_registration_group(
        reader: &mut Reader<BufReader<File>>,
        buf: &mut Vec<u8>,
    ) -> Result<IndexMap<(u16, u32), Segment>, IsbnRangeError> {
        buf.clear();
        let mut res = IndexMap::new();
        loop {
            match reader.read_event(buf)? {
                Event::Start(e) => {
                    if e.name() != b"Group" {
                        return Err(IsbnRangeError::NoGroup);
                    }
                }
                Event::End(e) if e.name() == b"RegistrationGroups" => {
                    return Ok(res);
                }
                _ => return Err(IsbnRangeError::WrongXmlEnd),
            };
            buf.clear();

            let mut prefix_val = 0u16;
            let mut registration_group_element = 0u32;
            for (i, char) in read_xml_tag(reader, buf, b"Prefix")?.chars().enumerate() {
                match i {
                    0..=2 => {
                        prefix_val = (prefix_val << 4)
                            | char.to_digit(10).ok_or(IsbnRangeError::InvalidPrefixChar)? as u16;
                    }
                    3 => {
                        if char != '-' {
                            return Err(IsbnRangeError::PrefixTooLong);
                        }
                    }
                    _ => {
                        registration_group_element = (registration_group_element << 4)
                            | char.to_digit(10).ok_or(IsbnRangeError::InvalidPrefixChar)?;
                    }
                }
            }

            res.insert(
                (prefix_val, registration_group_element),
                Segment::from_reader(reader, buf)?,
            );
        }
    }

    /// Opens the RangeMessage.xml file and loads the ranges into memory. Should be used if the
    /// ISBN ranges need to be current. Up-to-date ISBN ranges can be downloaded from the following
    /// sources:
    /// * https://www.isbn-international.org/export_rangemessage.xml
    /// * https://github.com/philippeitis/isbn-ranges/blob/master/RangeMessage.xml
    ///
    /// ```
    /// use isbn2::{Isbn, Isbn10, Isbn13, IsbnRange};
    ///
    /// let isbn_ranges = IsbnRange::from_file("isbn-ranges/RangeMessage.xml").unwrap();
    /// let isbn_10 = Isbn::_10(Isbn10::new([8, 9, 6, 6, 2, 6, 1, 2, 6, 4]).unwrap());
    /// let isbn_13 = Isbn::_13(Isbn13::new([9, 7, 8, 1, 4, 9, 2, 0, 6, 7, 6, 6, 5]).unwrap());
    ///
    /// assert_eq!(isbn_ranges.hyphenate_isbn(&isbn_10).unwrap().as_str(), "89-6626-126-4");
    /// assert_eq!(isbn_ranges.hyphenate_isbn(&isbn_13).unwrap().as_str(), "978-1-4920-6766-5");
    /// ```
    /// # Errors
    /// If the RangeMessage is in an unexpected format or does not exist, an error will be returned.
    pub fn from_file<P: AsRef<Path>>(p: P) -> Result<Self, IsbnRangeError> {
        let mut reader = Reader::from_reader(BufReader::new(File::open(p)?));
        reader.trim_text(true);
        let mut buf = Vec::new();
        loop {
            if read_xml_start(&mut reader, &mut buf, b"ISBNRangeMessage")? {
                break;
            }
        }

        let m_source = b"MessageSource";
        let m_serial_number = b"MessageSerialNumber";
        let m_date = b"MessageDate";
        let mut fields = vec![m_date.as_ref(), m_serial_number.as_ref(), m_source.as_ref()];
        let mut vals = vec![];
        while !fields.is_empty() {
            let mut buf1 = Vec::new();
            let mut buf2 = Vec::new();
            let e1 = reader.read_event(&mut buf)?;
            let e2 = reader.read_event(&mut buf1)?;
            let e3 = reader.read_event(&mut buf2)?;
            match (e1, e2, e3) {
                (Event::Start(es), Event::Text(et), Event::End(ee)) => {
                    while let Some(name) = fields.pop() {
                        if es.name() == name {
                            vals.push(Some(et.unescape_and_decode(&reader)?));
                            if ee.name() != name {
                                return Err(IsbnRangeError::WrongXmlEnd);
                            }
                            break;
                        } else {
                            vals.push(None);
                        }
                    }
                }
                _ => return Err(IsbnRangeError::MissingXmlStart),
            }
            buf.clear();
        }

        let date = vals.pop().flatten().ok_or(IsbnRangeError::NoMessageDate)?;
        let serial_number = vals.pop().expect("This error should not be possible");
        let source = vals.pop().expect("This error should not be possible");

        if !read_xml_start(&mut reader, &mut buf, b"EAN.UCCPrefixes")? {
            return Err(IsbnRangeError::NoEanUccPrefixes);
        }

        let ean_ucc_group = Self::read_ean_ucc_group(&mut reader, &mut buf)?;

        if !read_xml_start(&mut reader, &mut buf, b"RegistrationGroups")? {
            return Err(IsbnRangeError::NoRegistrationGroups);
        }

        let registration_group = Self::read_registration_group(&mut reader, &mut buf)?;

        Ok(IsbnRange {
            source,
            serial_number,
            date,
            ean_ucc_group,
            registration_group,
        })
    }

    /// Hyphenate an ISBN into its parts:
    ///
    /// * GS1 Prefix (ISBN-13 only)
    /// * Registration group
    /// * Registrant
    /// * Publication
    /// * Check digit
    ///
    /// ```
    /// use isbn2::{Isbn, Isbn10, Isbn13, IsbnRange};
    ///
    /// let isbn_ranges = IsbnRange::from_file("isbn-ranges/RangeMessage.xml").unwrap();
    /// let isbn_10 = Isbn::_10(Isbn10::new([8, 9, 6, 6, 2, 6, 1, 2, 6, 4]).unwrap());
    /// let isbn_13 = Isbn::_13(Isbn13::new([9, 7, 8, 1, 4, 9, 2, 0, 6, 7, 6, 6, 5]).unwrap());
    ///
    /// assert_eq!(isbn_ranges.hyphenate_isbn(&isbn_10).unwrap().as_str(), "89-6626-126-4");
    /// assert_eq!(isbn_ranges.hyphenate_isbn(&isbn_13).unwrap().as_str(), "978-1-4920-6766-5");
    /// ```
    /// # Errors
    /// If the ISBN is not valid, as determined by the current ISBN rules, an error will be
    /// returned.
    pub fn hyphenate_isbn(&self, isbn: &Isbn) -> Result<ArrayString<[u8; 17]>, IsbnError> {
        match isbn {
            Isbn::_10(isbn) => self.hyphenate_isbn_object(isbn),
            Isbn::_13(isbn) => self.hyphenate_isbn_object(isbn),
        }
    }

    /// Hyphenate an ISBN-10 into its parts:
    ///
    /// * Registration group
    /// * Registrant
    /// * Publication
    /// * Check digit
    ///
    /// ```
    /// use isbn2::{Isbn10, IsbnRange};
    ///
    /// let isbn_ranges = IsbnRange::from_file("isbn-ranges/RangeMessage.xml").unwrap();
    /// let isbn_10 = Isbn10::new([8, 9, 6, 6, 2, 6, 1, 2, 6, 4]).unwrap();
    /// assert_eq!(isbn_ranges.hyphenate_isbn10(&isbn_10).unwrap().as_str(), "89-6626-126-4");
    /// ```
    /// # Errors
    /// If the ISBN is not valid, as determined by the current ISBN rules, an error will be
    /// returned.
    pub fn hyphenate_isbn10(&self, isbn: &Isbn10) -> Result<ArrayString<[u8; 17]>, IsbnError> {
        self.hyphenate_isbn_object(isbn)
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
    /// use isbn2::{Isbn13, IsbnRange};
    ///
    /// let isbn_ranges = IsbnRange::from_file("isbn-ranges/RangeMessage.xml").unwrap();
    /// let isbn_13 = Isbn13::new([9, 7, 8, 1, 4, 9, 2, 0, 6, 7, 6, 6, 5]).unwrap();
    /// assert_eq!(isbn_ranges.hyphenate_isbn13(&isbn_13).unwrap().as_str(), "978-1-4920-6766-5");
    /// ```
    /// # Errors
    /// If the ISBN is not valid, as determined by the current ISBN rules, an error will be
    /// returned.
    pub fn hyphenate_isbn13(&self, isbn: &Isbn13) -> Result<ArrayString<[u8; 17]>, IsbnError> {
        self.hyphenate_isbn_object(isbn)
    }

    fn hyphenate_isbn_object(
        &self,
        isbn: &impl IsbnObject,
    ) -> Result<ArrayString<[u8; 17]>, IsbnError> {
        let segment = self
            .ean_ucc_group
            .get(&isbn.prefix_element())
            .ok_or(IsbnError::InvalidGroup)?;
        let registration_group_segment_length = segment.group(isbn.segment(0))?.segment_length;
        let segment = self
            .registration_group
            .get(&(
                isbn.prefix_element(),
                isbn.group_prefix(registration_group_segment_length),
            ))
            .ok_or(IsbnError::InvalidGroup)?;
        let registrant_segment_length = segment
            .group(isbn.segment(registration_group_segment_length))?
            .segment_length;

        let hyphen_at = [
            registration_group_segment_length,
            registration_group_segment_length + registrant_segment_length,
        ];

        Ok(isbn.hyphenate_with(hyphen_at))
    }

    /// Retrieve the name of the registration group.
    ///
    /// ```
    /// use isbn2::{Isbn, Isbn10, Isbn13, IsbnRange};
    ///
    /// let isbn_ranges = IsbnRange::from_file("isbn-ranges/RangeMessage.xml").unwrap();
    /// let isbn_10 = Isbn::_10(Isbn10::new([8, 9, 6, 6, 2, 6, 1, 2, 6, 4]).unwrap());
    /// let isbn_13 = Isbn::_13(Isbn13::new([9, 7, 8, 1, 4, 9, 2, 0, 6, 7, 6, 6, 5]).unwrap());
    ///
    /// assert_eq!(isbn_ranges.get_registration_group_isbn(&isbn_10), Ok("Korea, Republic"));
    /// assert_eq!(isbn_ranges.get_registration_group_isbn(&isbn_13), Ok("English language"));
    /// ```
    ///
    /// # Errors
    /// If the ISBN is not valid, as determined by `self`, an error will be
    /// returned.
    pub fn get_registration_group_isbn(&self, isbn: &Isbn) -> Result<&str, IsbnError> {
        match isbn {
            Isbn::_10(isbn) => self.get_registration_group_isbn_object(isbn),
            Isbn::_13(isbn) => self.get_registration_group_isbn_object(isbn),
        }
    }

    /// Retrieve the name of the registration group.
    ///
    /// ```
    /// use isbn2::{Isbn10, IsbnRange};
    ///
    /// let isbn_ranges = IsbnRange::from_file("isbn-ranges/RangeMessage.xml").unwrap();
    /// let isbn_10 = Isbn10::new([8, 9, 6, 6, 2, 6, 1, 2, 6, 4]).unwrap();
    /// assert_eq!(isbn_ranges.get_registration_group_isbn10(&isbn_10), Ok("Korea, Republic"));
    /// ```
    /// # Errors
    /// If the ISBN is not valid, as determined by the current ISBN rules, an error will be
    /// returned.
    pub fn get_registration_group_isbn10(&self, isbn: &Isbn10) -> Result<&str, IsbnError> {
        self.get_registration_group_isbn_object(isbn)
    }

    /// Retrieve the name of the registration group.
    ///
    /// ```
    /// use isbn2::{Isbn13, IsbnRange};
    ///
    /// let isbn_ranges = IsbnRange::from_file("isbn-ranges/RangeMessage.xml").unwrap();
    /// let isbn_13 = Isbn13::new([9, 7, 8, 1, 4, 9, 2, 0, 6, 7, 6, 6, 5]).unwrap();
    /// assert_eq!(isbn_ranges.get_registration_group_isbn13(&isbn_13), Ok("English language"));
    /// ```
    /// # Errors
    /// If the ISBN is not valid, as determined by the current ISBN rules, an error will be
    /// returned.
    pub fn get_registration_group_isbn13(&self, isbn: &Isbn13) -> Result<&str, IsbnError> {
        self.get_registration_group_isbn_object(isbn)
    }

    fn get_registration_group_isbn_object(
        &self,
        isbn: &impl IsbnObject,
    ) -> Result<&str, IsbnError> {
        let segment = self
            .ean_ucc_group
            .get(&isbn.prefix_element())
            .ok_or(IsbnError::InvalidGroup)?;
        let registration_group_segment_length = segment.group(isbn.segment(0))?.segment_length;
        let segment = self
            .registration_group
            .get(&(
                isbn.prefix_element(),
                isbn.group_prefix(registration_group_segment_length),
            ))
            .ok_or(IsbnError::InvalidGroup)?;
        Ok(&segment
            .group(isbn.segment(registration_group_segment_length))?
            .name)
    }

    pub fn date(&self) -> &str {
        &self.date
    }

    pub fn serial_number(&self) -> Option<&str> {
        match &self.serial_number {
            Some(s) => Some(s.as_str()),
            None => None,
        }
    }

    pub fn source(&self) -> Option<&str> {
        match &self.source {
            Some(s) => Some(s.as_str()),
            None => None,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_isbn_range_opens() {
        let range = IsbnRange::from_file("./isbn-ranges/RangeMessage.xml");
        assert!(range.is_ok());
        let range = range.unwrap();
        assert_eq!(
            range.source,
            Some(String::from("International ISBN Agency"))
        );
    }

    #[test]
    fn test_hyphenation() {
        let range = IsbnRange::from_file("./isbn-ranges/RangeMessage.xml").unwrap();
        assert!(range
            .hyphenate_isbn(&Isbn::from_str("0-9752298-0-X").unwrap())
            .is_ok());
        assert!(range
            .hyphenate_isbn(&Isbn::from_str("978-3-16-148410-0").unwrap())
            .is_ok());
    }
}
