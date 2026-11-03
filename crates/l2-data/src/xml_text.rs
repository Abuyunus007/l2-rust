//! Entity references in the text of hand-written quick-xml readers (G-09).
//!
//! quick-xml 0.42 no longer folds `&amp;` into `Event::Text`: it reports every reference as its
//! own `Event::GeneralRef`. A reader that collects only `Event::Text` therefore drops the
//! reference and silently shortens the value (`a&amp;b` becomes `ab`). Java's DOM parser
//! (Xerces) resolves the five predefined entities and the numeric character references, and
//! stops with a fatal error on any other name (no DTD declares one). [`push_general_ref`] does
//! the same.
//!
//! Use it next to the text arm:
//! ```ignore
//! Event::Text(t) => text.push_str(&t.xml10_content()),
//! Event::GeneralRef(r) => push_general_ref(&mut text, &r)?, // or warn and stop on Err
//! ```
//! Attribute values need nothing: `Attribute::normalized_value` resolves them.

use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::BytesRef;

/// A reference the parser cannot resolve (an undeclared entity name or a bad character number).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("The entity \"{0}\" was referenced, but not declared.")]
pub struct UnknownEntity(pub String);

/// Appends what a `&name;` or `&#N;` reference stands for to `out`.
///
/// `&amp;`, `&lt;`, `&gt;`, `&quot;` and `&apos;` and the decimal and hex character references
/// resolve; anything else is an [`UnknownEntity`] (Java: a `SAXParseException`, the file fails).
pub fn push_general_ref(out: &mut String, reference: &BytesRef<'_>) -> Result<(), UnknownEntity> {
    let name: &str = reference;
    let unknown = || UnknownEntity(name.to_owned());
    match reference.resolve_char_ref() {
        Ok(Some(c)) => out.push(c),
        Ok(None) => out.push_str(resolve_predefined_entity(name).ok_or_else(unknown)?),
        Err(_) => return Err(unknown()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    /// The text of the document, collected the way a hand-written reader does.
    fn text_of(xml: &str) -> Result<String, UnknownEntity> {
        let mut reader = Reader::from_str(xml);
        let mut text = String::new();
        loop {
            match reader.read_event().unwrap() {
                Event::Text(t) => text.push_str(&t.xml10_content()),
                Event::GeneralRef(r) => push_general_ref(&mut text, &r)?,
                Event::Eof => return Ok(text),
                _ => {}
            }
        }
    }

    #[test]
    fn predefined_entities_resolve() {
        assert_eq!(
            text_of("<a>x &amp; y &lt;b&gt; &quot;q&quot; &apos;s&apos;</a>").unwrap(),
            "x & y <b> \"q\" 's'"
        );
    }

    #[test]
    fn character_references_resolve() {
        assert_eq!(text_of("<a>&#65;&#x42;&#x20AC;</a>").unwrap(), "AB\u{20AC}");
    }

    #[test]
    fn unknown_entity_is_an_error() {
        assert_eq!(text_of("<a>&nbsp;</a>"), Err(UnknownEntity("nbsp".to_owned())));
    }
}
