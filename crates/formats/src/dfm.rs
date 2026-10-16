//! Delphi 1 form files (`TPF0`), as stored in the executable's `RT_RCDATA`
//! resources and read by `TReader`.
//!
//! An object is its class name and component name (both short strings), its
//! properties, a 0 byte, its children, and another 0 byte. A property is its
//! name (dotted for nested ones: `Font.Color`) and a typed value. Delphi 1
//! has no filer flags in front of the object.

use std::fmt;

const SIGNATURE: &[u8; 4] = b"TPF0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The data doesn't start with `TPF0`.
    NoSignature,
    /// The data ends inside an object.
    Truncated(usize),
    /// A value type `TReader` doesn't know.
    UnknownValueType { at: usize, kind: u8 },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoSignature => write!(f, "no TPF0 signature"),
            Error::Truncated(at) => write!(f, "form data ends at {at:#x}"),
            Error::UnknownValueType { at, kind } => {
                write!(f, "unknown value type {kind} at {at:#x}")
            }
        }
    }
}

impl std::error::Error for Error {}

/// A property value (`TValueType`).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// `vaList`: `( ... )`, as used by `TStrings` properties.
    List(Vec<Value>),
    /// `vaInt8`, `vaInt16` and `vaInt32`.
    Int(i32),
    /// `vaExtended`: the 10-byte `Extended`, kept as stored.
    Extended([u8; 10]),
    /// `vaString`.
    String(Vec<u8>),
    /// `vaIdent`: an enumeration value, colour or event handler name.
    Ident(Vec<u8>),
    /// `vaFalse` and `vaTrue`.
    Bool(bool),
    /// `vaBinary`: `Picture.Data`, `Glyph.Data` and other `DefineBinaryProperty` data.
    Binary(Vec<u8>),
    /// `vaSet`: the names of the members.
    Set(Vec<Vec<u8>>),
}

impl Value {
    #[must_use]
    pub fn as_int(&self) -> Option<i32> {
        match self {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The bytes of a string or identifier.
    #[must_use]
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Value::String(s) | Value::Ident(s) => Some(s),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    pub class: Vec<u8>,
    pub name: Vec<u8>,
    /// In file order. Properties left at their default aren't stored.
    pub properties: Vec<(Vec<u8>, Value)>,
    pub children: Vec<Object>,
}

impl Object {
    /// Parses a form resource. Bytes after the root object (the resource
    /// table's padding) are ignored.
    ///
    /// # Errors
    /// If the data isn't a complete `TPF0` object.
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        if data.get(..4) != Some(SIGNATURE) {
            return Err(Error::NoSignature);
        }
        Reader { data, pos: 4 }.object()
    }

    /// The value of the property `name`.
    #[must_use]
    pub fn property(&self, name: &str) -> Option<&Value> {
        self.properties
            .iter()
            .find(|(n, _)| n == name.as_bytes())
            .map(|(_, v)| v)
    }

    /// This object and all its descendants, depth first.
    pub fn walk(&self) -> impl Iterator<Item = &Object> {
        let mut stack = vec![self];
        std::iter::from_fn(move || {
            let next = stack.pop()?;
            stack.extend(next.children.iter().rev());
            Some(next)
        })
    }

    /// The descendant (or this object) called `name`.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&Object> {
        self.walk().find(|o| o.name == name.as_bytes())
    }
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn bytes(&mut self, n: usize) -> Result<&[u8], Error> {
        let s = self
            .data
            .get(self.pos..self.pos + n)
            .ok_or(Error::Truncated(self.data.len()))?;
        self.pos += n;
        Ok(s)
    }

    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.bytes(1)?[0])
    }

    fn pstr(&mut self) -> Result<Vec<u8>, Error> {
        let n = self.byte()?;
        Ok(self.bytes(usize::from(n))?.to_vec())
    }

    /// `TReader.EndOfList`, consuming the 0 byte.
    fn end_of_list(&mut self) -> Result<bool, Error> {
        if *self.data.get(self.pos).ok_or(Error::Truncated(self.data.len()))? == 0 {
            self.pos += 1;
            return Ok(true);
        }
        Ok(false)
    }

    fn object(&mut self) -> Result<Object, Error> {
        let class = self.pstr()?;
        let name = self.pstr()?;
        let mut properties = Vec::new();
        while !self.end_of_list()? {
            let prop = self.pstr()?;
            properties.push((prop, self.value()?));
        }
        let mut children = Vec::new();
        while !self.end_of_list()? {
            children.push(self.object()?);
        }
        Ok(Object {
            class,
            name,
            properties,
            children,
        })
    }

    fn value(&mut self) -> Result<Value, Error> {
        let at = self.pos;
        Ok(match self.byte()? {
            1 => {
                let mut items = Vec::new();
                while !self.end_of_list()? {
                    items.push(self.value()?);
                }
                Value::List(items)
            }
            2 => Value::Int(i32::from(self.byte()?.cast_signed())),
            3 => {
                let b = self.bytes(2)?;
                Value::Int(i32::from(i16::from_le_bytes([b[0], b[1]])))
            }
            4 => {
                let b = self.bytes(4)?;
                Value::Int(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            }
            5 => {
                let mut x = [0; 10];
                x.copy_from_slice(self.bytes(10)?);
                Value::Extended(x)
            }
            6 => Value::String(self.pstr()?),
            7 => Value::Ident(self.pstr()?),
            8 => Value::Bool(false),
            9 => Value::Bool(true),
            10 => {
                let b = self.bytes(4)?;
                let n = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                let n = usize::try_from(n).map_err(|_| Error::Truncated(self.data.len()))?;
                Value::Binary(self.bytes(n)?.to_vec())
            }
            11 => {
                let mut members = Vec::new();
                loop {
                    let m = self.pstr()?;
                    if m.is_empty() {
                        break Value::Set(members);
                    }
                    members.push(m);
                }
            }
            kind => return Err(Error::UnknownValueType { at, kind }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let data = b"TPF0\x05TForm\x02F1\x04Left\x02\x05\x03Tag\x04\x00\x00\x01\x00\
            \x05Style\x0b\x06fsBold\x00\x05Items\x01\x06\x01a\x06\x01b\x00\x07Visible\x08\x00\
            \x06TTimer\x02T1\x08Interval\x03\xe8\x03\x00\x00\x00padding";
        let form = Object::parse(data).unwrap();
        assert_eq!(form.class, b"TForm");
        assert_eq!(form.property("Left"), Some(&Value::Int(5)));
        assert_eq!(form.property("Tag"), Some(&Value::Int(0x10000)));
        assert_eq!(form.property("Style"), Some(&Value::Set(vec![b"fsBold".to_vec()])));
        assert_eq!(
            form.property("Items"),
            Some(&Value::List(vec![
                Value::String(b"a".to_vec()),
                Value::String(b"b".to_vec())
            ]))
        );
        assert_eq!(form.property("Visible"), Some(&Value::Bool(false)));
        let timer = form.find("T1").unwrap();
        assert_eq!(timer.property("Interval").and_then(Value::as_int), Some(1000));
        assert_eq!(form.walk().count(), 2);
    }

    #[test]
    fn errors() {
        assert_eq!(Object::parse(b"TPF1"), Err(Error::NoSignature));
        assert_eq!(Object::parse(b"TPF0\x01A\x01B\x01X"), Err(Error::Truncated(10)));
        assert_eq!(
            Object::parse(b"TPF0\x01A\x01B\x01X\x0f"),
            Err(Error::UnknownValueType { at: 10, kind: 15 })
        );
    }
}
