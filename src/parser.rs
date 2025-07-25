use nom::branch::alt;
use nom::bytes::complete::{tag, take_until, take_while, take_while1};
use nom::character::complete::{multispace0, space0, space1, u8};
use nom::combinator::{opt, verify};
use nom::error::Error;
use nom::multi::{many_m_n, many1, separated_list0, separated_list1};
use nom::sequence::{delimited, preceded, separated_pair, terminated};
use nom::{AsChar, IResult, Parser};

use std::cell::Cell;

pub type Properties<'a> = Vec<Property<'a>>;

#[derive(Debug)]
struct Property<'a> {
    name: &'a str,
    kind: PropertyKind<'a>,
}

#[derive(Debug)]
enum PropertyKind<'a> {
    String(&'a str),
    List(Vec<Fragment<'a>>),
}

#[derive(Debug)]
pub struct File<'a> {
    // Option<(non empty) Vec> is isomorphic to Vec, but I think it's better to be semantically clear
    properties: Option<Properties<'a>>,
    paragraphs: Vec<Paragraph<'a>>,
}

#[derive(Debug)]
pub struct Paragraph<'a> {
    heading: Option<Heading<'a>>,
    body: Vec<Fragment<'a>>,
}

#[derive(Debug)]
pub struct Heading<'a> {
    level: u8,
    text: &'a str,
}

#[derive(Debug)]
enum Fragment<'a> {
    Link(Link<'a>),
    UnorderedListBlock(Vec<Vec<Fragment<'a>>>),
    OrderedListBlock(Vec<Vec<Fragment<'a>>>),
    FormattedText {
        kind: FormattedTextKind,
        body: &'a str,
    },
    QuoteBlock(Vec<Vec<Fragment<'a>>>),
    InlineCode(&'a str),
    CodeBlock {
        lang: Option<&'a str>,
        body: &'a str,
    },
    PlainStr(&'a str),
}

#[derive(Debug)]
enum Link<'a> {
    Wiki { to: &'a str, text: &'a str },
    External(&'a str, &'a str),
}

#[derive(Debug)]
enum FormattedTextKind {
    Bold,
    Italics,
    Strikethrough,
    InlineCode,
}

macro_rules! not_in {
    ($($args:literal),*) => {
        |c: char| {!(not_in_inner!(c, $($args),*) || c.is_newline())}
    }
}

macro_rules! not_in_inner {
    ($c:ident, $arg:literal, $($args:literal),+) => {
        $c == $arg || not_in_inner!($c, $($args),*)
    };
    ($c:ident, $arg:literal) => {
        $c == $arg
    };
}

pub fn parse_file<'a>(file: &'a str) -> Result<File<'a>, nom::Err<Error<&'a str>>> {
    let (file, properties) = opt(parse_properties).parse(file)?;
    let (_, paragraphs) = terminated(
        separated_list1(tag("\n\n"), parse_paragraph),
        (multispace0, nom::combinator::eof),
    )
    .parse(file)?;

    Ok(File {
        properties,
        paragraphs,
    })
}

fn parse_properties<'a>(file: &'a str) -> IResult<&'a str, Properties<'a>> {
    let (file, res) = delimited(
        tag("---\n"),
        separated_list0(tag("\n"), parse_property),
        (multispace0, tag("---")),
    )
    .parse(file)?;

    Ok((file, res))
}

fn parse_property<'a>(file: &'a str) -> IResult<&'a str, Property<'a>> {
    let (file, name) = terminated(
        preceded(
            multispace0,
            take_while1(|c: char| !(c == ':' || c == '-' || c.is_space() || c.is_newline())),
        ),
        tag(":"),
    )
    .parse(file)?;

    let (file, kind) = alt((
        parse_property_list,
        take_while(not_in!('[', ']')).map(PropertyKind::String),
    ))
    .parse(file)?;

    Ok((file, Property { name, kind }))
}

fn parse_property_list<'a>(file: &'a str) -> IResult<&'a str, PropertyKind<'a>> {
    preceded(
        multispace0,
        separated_list1(
            tag("\n"),
            preceded(
                (space0, tag("-"), space1),
                alt((
                    delimited(
                        tag("\"[["),
                        take_while1(|c: char| {
                            !(c == ':' || c == '[' || c == ']' || c.is_newline())
                        }),
                        tag("]]\""),
                    )
                    .map(|s| Fragment::Link(Link::Wiki { to: s, text: s })),
                    take_while1(|c: char| !(c == ':' || c == '[' || c == ']' || c.is_newline()))
                        .map(Fragment::PlainStr),
                )),
            ),
        ),
    )
    .map(PropertyKind::List)
    .parse(file)
}

fn parse_paragraph<'a>(file: &'a str) -> IResult<&'a str, Paragraph<'a>> {
    let (file, heading) = opt(parse_heading).parse(file)?;
    let (file, body) = preceded(
        multispace0,
        separated_list0((space0, opt(tag("\n"))), parse_fragment),
    )
    .parse(file)?;

    Ok((file, Paragraph { heading, body }))
}

fn parse_fragment<'a>(file: &'a str) -> IResult<&'a str, Fragment<'a>> {
    alt((
        parse_link,
        parse_unordered_list_block,
        parse_ordered_list_block,
        parse_formatted_text,
        parse_quote_block,
        parse_code_block,
        parse_string.map(Fragment::PlainStr),
    ))
    .parse(file)
}

fn parse_heading<'a>(file: &'a str) -> IResult<&'a str, Heading<'a>> {
    (many_m_n(1, 6, tag("#")), parse_string)
        .map(|(v, s)| Heading {
            level: v.len() as u8,
            text: s,
        })
        .parse(file)
}

fn parse_link<'a>(file: &'a str) -> IResult<&'a str, Fragment<'a>> {
    fn parse_wikilink<'a>(file: &'a str) -> IResult<&'a str, Fragment<'a>> {
        let (file, inner) =
            delimited(tag("[["), take_while1(not_in!(']', '[', '|')), tag("]]")).parse(file)?;

        Ok((
            file,
            Fragment::Link(Link::Wiki {
                to: inner,
                text: inner,
            }),
        ))
    }

    fn parse_alias_wikilink<'a>(file: &'a str) -> IResult<&'a str, Fragment<'a>> {
        let (file, (to, alias)) = delimited(
            tag("[["),
            separated_pair(
                take_while1(not_in!(']', '[', '|')),
                tag("|"),
                take_while1(not_in!(']', '[', '|')),
            ),
            tag("]]"),
        )
        .parse(file)?;

        Ok((file, Fragment::Link(Link::Wiki { to, text: alias })))
    }

    fn parse_ext_link<'a>(file: &'a str) -> IResult<&'a str, Fragment<'a>> {
        let (file, txt) =
            delimited(tag("["), take_while1(not_in!(']', '[')), tag("]")).parse(file)?;
        let (file, link) =
            delimited(tag("("), take_while1(not_in!('(', ')')), tag(")")).parse(file)?;

        Ok((file, Fragment::Link(Link::External(txt, link))))
    }

    alt((parse_alias_wikilink, parse_wikilink, parse_ext_link)).parse(file)
}

fn parse_unordered_list_block<'a>(file: &'a str) -> IResult<&'a str, Fragment<'a>> {
    let (file, inner) = many1(preceded(
        (tag("\n"), multispace0, tag("-"), multispace0),
        separated_list0(multispace0, parse_fragment),
    ))
    .parse(file)?;

    Ok((file, Fragment::UnorderedListBlock(inner)))
}

fn parse_ordered_list_block<'a>(file: &'a str) -> IResult<&'a str, Fragment<'a>> {
    let cur = Cell::new(0u8);
    let verifier = |n: &u8| {
        let cur_v = cur.get();
        if cur_v < 2 && *n < 2 {
            cur.set(*n);
            true
        } else if *n == cur_v + 1 {
            cur.set(*n);
            true
        } else {
            false
        }
    };
    let (file, inner) = many1(preceded(
        (multispace0, verify(u8, verifier), tag("."), space0),
        separated_list0(space0, parse_fragment),
    ))
    .parse(file)?;

    Ok((file, Fragment::OrderedListBlock(inner)))
}

fn parse_formatted_text<'a>(file: &'a str) -> IResult<&'a str, Fragment<'a>> {
    fn parse_bold<'a>(file: &'a str) -> IResult<&'a str, Fragment<'a>> {
        delimited(tag("**"), parse_string, tag("**"))
            .map(|s| Fragment::FormattedText {
                kind: FormattedTextKind::Bold,
                body: s,
            })
            .parse(file)
    }

    fn parse_italics<'a>(file: &'a str) -> IResult<&'a str, Fragment<'a>> {
        delimited(tag("*"), parse_string, tag("*"))
            .map(|s| Fragment::FormattedText {
                kind: FormattedTextKind::Italics,
                body: s,
            })
            .parse(file)
    }

    fn parse_strikethrough<'a>(file: &'a str) -> IResult<&'a str, Fragment<'a>> {
        delimited(tag("~~"), parse_string, tag("~~"))
            .map(|s| Fragment::FormattedText {
                kind: FormattedTextKind::Strikethrough,
                body: s,
            })
            .parse(file)
    }

    fn parse_inline_code<'a>(file: &'a str) -> IResult<&'a str, Fragment<'a>> {
        delimited(tag("`"), parse_string, tag("`"))
            .map(|s| Fragment::FormattedText {
                kind: FormattedTextKind::InlineCode,
                body: s,
            })
            .parse(file)
    }

    let res = alt((
        parse_bold,
        parse_italics,
        parse_strikethrough,
        parse_inline_code,
    ))
    .parse(file)?;

    Ok(res)
}

fn parse_quote_block<'a>(file: &'a str) -> IResult<&'a str, Fragment<'a>> {
    let (file, inner) = many1(preceded(
        (tag("\n"), multispace0, tag(">"), multispace0),
        separated_list0(multispace0, parse_fragment),
    ))
    .parse(file)?;

    Ok((file, Fragment::QuoteBlock(inner)))
}

fn parse_code_block<'a>(file: &'a str) -> IResult<&'a str, Fragment<'a>> {
    let (file, (lang, body)) = delimited(
        tag("```\n"),
        (opt(terminated(parse_word, tag("\n"))), parse_code),
        tag("\n```"),
    )
    .parse(file)?;

    Ok((file, Fragment::CodeBlock { lang, body }))
}

fn parse_code<'a>(file: &'a str) -> IResult<&'a str, &'a str> {
    take_until("\n```").parse(file)
}

fn parse_string<'a>(file: &'a str) -> IResult<&'a str, &'a str> {
    take_while1(|c: char| {
        !(c == '*' || c == '`' || c == '~' || c == '|' || c == '[' || c == ']' || c.is_newline())
    })
    .parse(file)
}

fn parse_word<'a>(file: &'a str) -> IResult<&'a str, &'a str> {
    take_while1(|c: char| {
        !(c == '*'
            || c == '`'
            || c == '~'
            || c == '['
            || c == ']'
            || c == '|'
            || c.is_space()
            || c.is_newline())
    })
    .parse(file)
}
