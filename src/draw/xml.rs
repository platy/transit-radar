

/// Write a self-closing svg element and it's attributes
#[macro_export]
macro_rules! write_xml_element {
    ($dst:expr, <$name:ident $($aname:ident={$avalue:expr})* />) => {
        $dst.write_fmt($crate::write_xml_element!(@attributes($(($aname=$avalue))*) -> concat!("<", stringify!($name)), ()))
    };
    (@attributes(($aname:ident=$avalue:expr) $(($bname:ident=$bvalue:expr))*) -> $pattern:expr, ($($args:tt),*)) => {
        $crate::write_xml_element!(@attributes($(($bname=$bvalue))*) -> concat!($pattern, " ", stringify!($aname), "=\"{}\""), ($($args,)* $avalue))
    };
    (@attributes() -> $pattern:expr, ($($args:tt),*)) => {
        format_args!(concat!($pattern, "/>"), $($args),*)
    };
}
