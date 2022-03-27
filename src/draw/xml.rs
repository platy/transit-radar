#[macro_export]
macro_rules! xml_format_args {
    // ends a tag
    (@inner(> $($attrs:tt)*) -> ($($pattern:expr),*), ($($args:expr),*)) => {
        $crate::xml_format_args!(@outer($($attrs)*) -> ($($pattern),*, ">"), ($($args),*))
    };
    // ends a self-closing element
    (@inner(/> $($attrs:tt)*) -> ($($pattern:expr),*), ($($args:expr),*)) => {
        $crate::xml_format_args!(@outer($($attrs)*) -> ($($pattern),*, " />"), ($($args),*))
    };
    // matches an attribute with a singly-hyphenated name
    (@inner($aname1:ident-$aname2:ident $($attrs:tt)*) -> ($($pattern:expr),*), ($($args:expr),*)) => {
        $crate::xml_format_args!(@attr($($attrs)*) -> ($($pattern),*, " ", stringify!($aname1), "-", stringify!($aname2)), ($($args),*))
    };
    // matches an attribute which fits in a rust identifier
    (@inner($aname:ident $($attrs:tt)*) -> ($($pattern:expr),*), ($($args:expr),*)) => {
        $crate::xml_format_args!(@attr($($attrs)*) -> ($($pattern),*, " ", stringify!($aname)), ($($args),*))
    };

    // an expression, evaluating to an iterable as a comma-separated attribute value
    (@attr(=[$avalue:expr,] $($attrs:tt)*) -> ($($pattern:expr),*), ($($args:expr),*)) => {
        $crate::xml_format_args!(@inner($($attrs)*) -> ($($pattern),*, "=\"{}\""), ($($args,)* crate::draw::xml::JoinList { list: $avalue, join: "," }))
    };
    // an expression as an attribute value
    (@attr(={$avalue:expr} $($attrs:tt)*) -> ($($pattern:expr),*), ($($args:expr),*)) => {
        $crate::xml_format_args!(@inner($($attrs)*) -> ($($pattern),*, "=\"{}\""), ($($args,)* $avalue))
    };
    // a literal as an attribute value
    (@attr(=$avalue:literal $($attrs:tt)*) -> ($($pattern:expr),*), ($($args:expr),*)) => {
        $crate::xml_format_args!(@inner($($attrs)*) -> ($($pattern),*, "=\"", $avalue, "\""), ($($args),*))
    };

    // starts a tag
    (@outer(<$name:ident $($attrs:tt)*) -> ($($pattern:expr),*), ($($args:expr),*)) => {
        $crate::xml_format_args!(@inner($($attrs)*) -> ($($pattern),*, "<", stringify!($name)), ($($args),*))
    };
    // matches an end tag
    (@outer(</$name:ident> $($attrs:tt)*) -> ($($pattern:expr),*), ($($args:expr),*)) => {
        $crate::xml_format_args!(@outer($($attrs)*) -> ($($pattern),*, "</", stringify!($name), ">"), ($($args),*))
    };
    // matches a text expression
    (@outer({$text:expr} $($attrs:tt)*) -> ($($pattern:expr),*), ($($args:expr),*)) => {
        $crate::xml_format_args!(@outer($($attrs)*) -> ($($pattern),*, "{}"), ($($args,)* $text))
    };
    // matches a text literal
    (@outer($text:literal $($attrs:tt)*) -> ($($pattern:expr),*), ($($args:expr),*)) => {
        $crate::xml_format_args!(@outer($($attrs)*) -> ($($pattern),*, $text), ($($args),*))
    };
    // matches the end of the xml
    (@outer() -> ($($pattern:expr),*), ($($args:expr),*)) => {
        format_args!(concat!($($pattern),*, "\n"), $($args),*)
    };

    // matches the start of a tag, for opening the xml
    (<$($attrs:tt)*) => {
        $crate::xml_format_args!(@outer(<$($attrs)*) -> (""), ())
    };
}

/// Write XML
#[macro_export]
macro_rules! write_xml {
    ($dst:expr, $($attrs:tt)*) => {
        $dst.write_fmt($crate::xml_format_args!($($attrs)*))
    }
}

/// Format a self-closing xml element and it's attributes as a `String`
#[macro_export]
macro_rules! format_xml {
    ($($attrs:tt)*) => {{
        let mut s = String::new();
        std::fmt::Write::write_fmt(&mut s, $crate::xml_format_args!($($attrs)*)).unwrap();
        s
    }}
}

pub struct JoinList<D, I>
where
    D: std::fmt::Display,
    I: IntoIterator<Item = D> + Copy,
{
    pub list: I,
    pub join: &'static str,
}

impl<D, I> std::fmt::Display for JoinList<D, I>
where
    D: std::fmt::Display,
    I: IntoIterator<Item = D> + Copy,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut iter = self.list.into_iter();
        if let Some(i) = iter.next() {
            i.fmt(f)?;
        }
        for i in iter {
            f.write_str(self.join)?;
            i.fmt(f)?;
        }
        Ok(())
    }
}

#[test]
fn self_closing() {
    assert_eq!(format_xml!(<tag />), r#"<tag />"#);
}

#[test]
fn self_closing_attribute() {
    assert_eq!(format_xml!(<tag att={"val"} />), r#"<tag att="val" />"#);
}

#[test]
fn self_closing_attributes() {
    assert_eq!(
        format_xml!(<tag att={"val"} att2={"val2"} />),
        r#"<tag att="val" att2="val2" />"#
    );
}

#[test]
fn literal_attribute() {
    assert_eq!(format_xml!(<tag att="val" />), r#"<tag att="val" />"#);
}

#[test]
fn self_closing_hyphenated_attribute() {
    assert_eq!(
        format_xml!(<tag the-att={"val"} />),
        r#"<tag the-att="val" />"#
    );
}

#[test]
fn literal_hyphenated_attribute() {
    assert_eq!(
        format_xml!(<tag the-att="val" />),
        r#"<tag the-att="val" />"#
    );
}

#[test]
fn comma_separated_attribute() {
    let list = &[1, 2, 3];
    // trace_macros!(true);
    // trace_macros!(false);
    assert_eq!(format_xml!(<tag att=[list,] />), r#"<tag att="1,2,3" />"#);
}

#[test]
fn text_containing() {
    assert_eq!(format_xml!(<tag>{"text"}</tag>), r#"<tag>text</tag>"#);
}

#[test]
fn text_literal_containing() {
    assert_eq!(format_xml!(<tag>"text"</tag>), r#"<tag>text</tag>"#);
}

#[test]
fn element_containing() {
    assert_eq!(format_xml!(<tag><inner /></tag>), r#"<tag><inner /></tag>"#);
}
