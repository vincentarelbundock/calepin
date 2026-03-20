/// Parser for Typst math expressions.
///
/// This is a hand-written recursive descent parser with Pratt-style operator
/// precedence, informed by the Parsec-based parser in `typst-hs/src/Typst/Parse.hs`.
///
/// We parse only the *content* between `$...$` — the caller strips the dollar signs.

use crate::ast::{MathArg, MathNode};
use crate::error::Error;
use crate::symbols::SHORTHANDS;

// ---------------------------------------------------------------------------
// Tokeniser
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Token {
    /// A number like `42` or `3.14`.
    Number(String),
    /// A multi-letter identifier like `alpha`, `sin`.
    Ident(String),
    /// A single non-space, non-special character (used as a math variable or symbol).
    Char(char),
    /// A string literal `"..."`.
    StringLit(String),
    /// A shorthand that resolved to a symbol name.
    Shorthand(String),
    /// `_`
    Underscore,
    /// `^`
    Caret,
    /// `/`
    Slash,
    /// `!` (factorial, only when no space before)
    Bang,
    /// `&`
    Ampersand,
    /// `\\` (linebreak)
    Backslash,
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `,`
    Comma,
    /// `;`
    Semicolon,
    /// `:`
    Colon,
    /// `.` (for field access on identifiers)
    Dot,
    /// End of input.
    Eof,
}

struct Lexer {
    chars: Vec<char>,
    pos: usize,
    tokens: Vec<(Token, usize, bool)>, // (token, start_pos, space_before)
}

impl Lexer {
    fn new(input: &str) -> Self {
        Lexer {
            chars: input.chars().collect(),
            pos: 0,
            tokens: Vec::new(),
        }
    }

    fn tokenize(&mut self) -> Vec<(Token, usize, bool)> {
        let mut space_before = false;
        while self.pos < self.chars.len() {
            let start = self.pos;
            let c = self.chars[self.pos];

            if c.is_whitespace() {
                self.pos += 1;
                while self.pos < self.chars.len() && self.chars[self.pos].is_whitespace() {
                    self.pos += 1;
                }
                space_before = true;
                continue;
            }

            let sb = space_before;
            space_before = false;

            match c {
                // String literal
                '"' => {
                    self.pos += 1;
                    let mut s = String::new();
                    while self.pos < self.chars.len() && self.chars[self.pos] != '"' {
                        if self.chars[self.pos] == '\\' && self.pos + 1 < self.chars.len() {
                            self.pos += 1;
                            match self.chars[self.pos] {
                                'n' => s.push('\n'),
                                't' => s.push('\t'),
                                '\\' => s.push('\\'),
                                '"' => s.push('"'),
                                other => {
                                    s.push('\\');
                                    s.push(other);
                                }
                            }
                        } else {
                            s.push(self.chars[self.pos]);
                        }
                        self.pos += 1;
                    }
                    if self.pos < self.chars.len() {
                        self.pos += 1; // skip closing "
                    }
                    self.tokens.push((Token::StringLit(s), start, sb));
                }

                // Escape sequence
                '\\' => {
                    self.pos += 1;
                    if self.pos < self.chars.len() && !self.chars[self.pos].is_whitespace() {
                        let ch = self.chars[self.pos];
                        self.pos += 1;
                        self.tokens.push((Token::Char(ch), start, sb));
                    } else {
                        // linebreak: `\` followed by whitespace
                        while self.pos < self.chars.len()
                            && self.chars[self.pos].is_whitespace()
                        {
                            self.pos += 1;
                        }
                        self.tokens.push((Token::Backslash, start, sb));
                    }
                }

                // Number
                _ if c.is_ascii_digit() => {
                    let mut num = String::new();
                    while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_digit() {
                        num.push(self.chars[self.pos]);
                        self.pos += 1;
                    }
                    if self.pos < self.chars.len() && self.chars[self.pos] == '.' {
                        num.push('.');
                        self.pos += 1;
                        while self.pos < self.chars.len()
                            && self.chars[self.pos].is_ascii_digit()
                        {
                            num.push(self.chars[self.pos]);
                            self.pos += 1;
                        }
                    }
                    self.tokens.push((Token::Number(num), start, sb));
                }

                // Try shorthands before single-char tokens
                _ if self.try_shorthand(start, sb) => {}

                // Multi-letter identifier (at least 2 letters, no _ or -)
                _ if is_ident_start(c) && c != '_' => {
                    // Peek ahead: if the next char is also a letter (forming 2+), it's an ident.
                    let next = self.chars.get(self.pos + 1).copied();
                    if next.map_or(false, |nc| is_math_ident_continue(nc)) {
                        let mut ident = String::new();
                        while self.pos < self.chars.len()
                            && is_math_ident_continue(self.chars[self.pos])
                        {
                            ident.push(self.chars[self.pos]);
                            self.pos += 1;
                        }
                        // Check for dotted access (e.g., arrow.r.double)
                        while self.pos < self.chars.len() && self.chars[self.pos] == '.' {
                            let dot_pos = self.pos;
                            self.pos += 1;
                            if self.pos < self.chars.len()
                                && is_ident_start(self.chars[self.pos])
                                && self.chars[self.pos] != '_'
                            {
                                ident.push('.');
                                while self.pos < self.chars.len()
                                    && is_math_ident_continue(self.chars[self.pos])
                                {
                                    ident.push(self.chars[self.pos]);
                                    self.pos += 1;
                                }
                            } else {
                                self.pos = dot_pos;
                                break;
                            }
                        }
                        self.tokens.push((Token::Ident(ident), start, sb));
                    } else {
                        // Single letter — treat as a Char (math variable).
                        self.pos += 1;
                        self.tokens.push((Token::Char(c), start, sb));
                    }
                }

                '_' => {
                    self.pos += 1;
                    self.tokens.push((Token::Underscore, start, sb));
                }
                '^' => {
                    self.pos += 1;
                    self.tokens.push((Token::Caret, start, sb));
                }
                '/' => {
                    self.pos += 1;
                    self.tokens.push((Token::Slash, start, sb));
                }
                '!' => {
                    self.pos += 1;
                    // Only emit Bang if next char is not '='
                    if self.pos < self.chars.len() && self.chars[self.pos] == '=' {
                        // This is `!=`, should have been caught by shorthands
                        self.pos -= 1;
                        self.try_shorthand(start, sb);
                        if self.pos == start {
                            // fallback
                            self.pos += 1;
                            self.tokens.push((Token::Bang, start, sb));
                        }
                    } else {
                        self.tokens.push((Token::Bang, start, sb));
                    }
                }
                '&' => {
                    self.pos += 1;
                    self.tokens.push((Token::Ampersand, start, sb));
                }
                '(' => {
                    self.pos += 1;
                    self.tokens.push((Token::LParen, start, sb));
                }
                ')' => {
                    self.pos += 1;
                    self.tokens.push((Token::RParen, start, sb));
                }
                '[' => {
                    self.pos += 1;
                    self.tokens.push((Token::LBracket, start, sb));
                }
                ']' => {
                    self.pos += 1;
                    self.tokens.push((Token::RBracket, start, sb));
                }
                '{' => {
                    self.pos += 1;
                    self.tokens.push((Token::LBrace, start, sb));
                }
                '}' => {
                    self.pos += 1;
                    self.tokens.push((Token::RBrace, start, sb));
                }
                ',' => {
                    self.pos += 1;
                    self.tokens.push((Token::Comma, start, sb));
                }
                ';' => {
                    self.pos += 1;
                    self.tokens.push((Token::Semicolon, start, sb));
                }
                ':' => {
                    self.pos += 1;
                    self.tokens.push((Token::Colon, start, sb));
                }
                '.' => {
                    self.pos += 1;
                    self.tokens.push((Token::Dot, start, sb));
                }
                '\u{221A}' => {
                    // √ Unicode sqrt symbol
                    self.pos += 1;
                    // If followed by '(', treat as 'root' identifier for function call
                    if self.pos < self.chars.len() && self.chars[self.pos] == '(' {
                        self.tokens.push((Token::Ident("root".into()), start, sb));
                    } else {
                        self.tokens.push((Token::Ident("sqrt".into()), start, sb));
                    }
                }
                _ => {
                    self.pos += 1;
                    self.tokens.push((Token::Char(c), start, sb));
                }
            }
        }
        self.tokens.push((Token::Eof, self.pos, false));
        self.tokens.clone()
    }

    /// Try to match a shorthand at the current position. Returns true if matched.
    fn try_shorthand(&mut self, start: usize, space_before: bool) -> bool {
        let remaining: String = self.chars[self.pos..].iter().collect();
        for &(short, sym_name) in SHORTHANDS.iter() {
            if remaining.starts_with(short) {
                // For single-char shorthands like `*` and `~`, only match if they
                // wouldn't be better handled as a regular char in certain contexts.
                self.pos += short.len();
                self.tokens
                    .push((Token::Shorthand(sym_name.to_string()), start, space_before));
                return true;
            }
        }
        false
    }
}

fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_alphabetic()
}

fn is_math_ident_continue(c: char) -> bool {
    // In Typst math mode, multi-letter identifiers contain only letters (no _ or -)
    c.is_alphabetic()
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

pub struct Parser {
    tokens: Vec<(Token, usize, bool)>,
    pos: usize,
}

impl Parser {
    pub fn new(input: &str) -> Self {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize();
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos].0
    }

    fn peek_space_before(&self) -> bool {
        self.tokens[self.pos].2
    }

    fn start_pos(&self) -> usize {
        self.tokens[self.pos].1
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens[self.pos].0.clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        tok
    }

    fn expect(&mut self, expected: &Token) -> Result<(), Error> {
        if self.peek() == expected {
            self.advance();
            Ok(())
        } else {
            Err(Error::Parse {
                message: format!("expected {:?}, got {:?}", expected, self.peek()),
                position: self.start_pos(),
            })
        }
    }

    /// Parse the entire math input into a list of nodes.
    pub fn parse(&mut self) -> Result<Vec<MathNode>, Error> {
        let mut nodes = Vec::new();
        while *self.peek() != Token::Eof {
            // Insert Space node when there's whitespace before the current token
            if self.peek_space_before() && !nodes.is_empty() {
                nodes.push(MathNode::Space);
            }
            nodes.push(self.parse_expr(0)?);
        }
        // Collapse adjacent spaces and trim.
        Ok(collapse_spaces(nodes))
    }

    /// Pratt parser entry point.
    /// Binding powers (matching the Haskell precedence levels):
    ///   30 = `/` (fraction)
    ///   40 = `_` `^` (attachment, general)
    ///   50 = `!` (factorial)
    ///   60 = implicit function call
    ///   70 = `_` `^` (attachment with number/group)
    fn parse_expr(&mut self, min_bp: u8) -> Result<MathNode, Error> {
        let mut lhs = self.parse_atom()?;

        loop {
            // Check for postfix/infix operators
            let (op_bp, is_postfix) = match self.peek() {
                Token::Slash => (30, false),
                Token::Underscore | Token::Caret => {
                    // Precedence depends on what follows: number/group = 70, else = 40
                    let bp = self.peek_attachment_bp();
                    (bp, false)
                }
                Token::Bang if !self.peek_space_before() => (50, true),
                Token::LParen if !self.peek_space_before() && is_callable(&lhs) => (60, true),
                _ => break,
            };

            if op_bp < min_bp {
                break;
            }

            if is_postfix {
                match self.peek().clone() {
                    Token::Bang => {
                        self.advance();
                        lhs = MathNode::Group {
                            open: None,
                            close: None,
                            children: vec![lhs, MathNode::Text("!".into())],
                        };
                    }
                    Token::LParen => {
                        // Implicit function call: ident(args)
                        let name = extract_name(&lhs);
                        let args = self.parse_func_args()?;
                        lhs = MathNode::FuncCall { name, args };
                    }
                    _ => break,
                }
                continue;
            }

            // Infix
            match self.peek().clone() {
                Token::Slash => {
                    self.advance();
                    let rhs = self.parse_expr(op_bp + 1)?;
                    lhs = MathNode::Frac(Box::new(lhs), Box::new(hide_outer_parens(rhs)));
                }
                Token::Underscore => {
                    self.advance();
                    let sub = self.parse_expr(op_bp + 1)?;
                    lhs = attach_bottom(lhs, hide_outer_parens(sub));
                }
                Token::Caret => {
                    self.advance();
                    let sup = self.parse_expr(op_bp + 1)?;
                    lhs = attach_top(lhs, hide_outer_parens(sup));
                }
                _ => break,
            }
        }

        Ok(lhs)
    }

    /// Peek ahead to determine the binding power of `_` or `^`.
    fn peek_attachment_bp(&self) -> u8 {
        // Look at the token after `_`/`^`
        let next_pos = self.pos + 1;
        if next_pos < self.tokens.len() {
            match &self.tokens[next_pos].0 {
                Token::Number(_) | Token::LParen | Token::LBrace | Token::LBracket => 70,
                _ => 40,
            }
        } else {
            40
        }
    }

    /// Parse a base math atom.
    fn parse_atom(&mut self) -> Result<MathNode, Error> {
        match self.peek().clone() {
            Token::Number(n) => {
                self.advance();
                Ok(MathNode::Text(n))
            }
            Token::Ident(name) => {
                self.advance();
                Ok(MathNode::Ident(name))
            }
            Token::Char(c) => {
                self.advance();
                Ok(MathNode::Text(c.to_string()))
            }
            Token::Shorthand(sym) => {
                self.advance();
                Ok(MathNode::Ident(sym))
            }
            Token::StringLit(s) => {
                self.advance();
                Ok(MathNode::StringLit(s))
            }
            Token::Ampersand => {
                self.advance();
                Ok(MathNode::AlignPoint)
            }
            Token::Backslash => {
                self.advance();
                Ok(MathNode::Linebreak)
            }
            Token::LParen => self.parse_group('(', ')'),
            Token::LBrace => self.parse_group('{', '}'),
            Token::LBracket => self.parse_group('[', ']'),
            Token::Dot => {
                self.advance();
                Ok(MathNode::Text(".".into()))
            }
            Token::Colon => {
                self.advance();
                Ok(MathNode::Text(":".into()))
            }
            Token::Comma => {
                self.advance();
                Ok(MathNode::Text(",".into()))
            }
            Token::Semicolon => {
                self.advance();
                Ok(MathNode::Text(";".into()))
            }
            Token::Eof => Err(Error::Parse {
                message: "unexpected end of input".into(),
                position: self.start_pos(),
            }),
            other => {
                let pos = self.start_pos();
                self.advance();
                Err(Error::Parse {
                    message: format!("unexpected token {:?}", other),
                    position: pos,
                })
            }
        }
    }

    /// Parse a grouped expression `(...)`, `{...}`, or `[...]`.
    fn parse_group(&mut self, open: char, close: char) -> Result<MathNode, Error> {
        self.advance(); // consume opening delimiter
        let close_tok = match close {
            ')' => Token::RParen,
            '}' => Token::RBrace,
            ']' => Token::RBracket,
            _ => unreachable!(),
        };
        let mut children = Vec::new();
        while *self.peek() != close_tok && *self.peek() != Token::Eof {
            // Insert Space node when there's space between tokens and we have content
            if self.peek_space_before() && !children.is_empty() {
                children.push(MathNode::Space);
            }
            children.push(self.parse_expr(0)?);
        }
        if *self.peek() == close_tok {
            self.advance();
        }
        Ok(MathNode::Group {
            open: Some(open.to_string()),
            close: Some(close.to_string()),
            children,
        })
    }

    /// Parse function call arguments inside `(...)`.
    fn parse_func_args(&mut self) -> Result<Vec<MathArg>, Error> {
        self.expect(&Token::LParen)?;
        let mut args = Vec::new();

        // Try to detect array args (rows separated by `;`)
        // First, try parsing as array args
        if let Some(array_arg) = self.try_parse_array_arg()? {
            args.push(array_arg);
            if *self.peek() == Token::RParen {
                self.advance();
                return Ok(args);
            }
        }

        while *self.peek() != Token::RParen && *self.peek() != Token::Eof {
            // Try named arg: `ident: value`
            if let Some(named) = self.try_parse_named_arg()? {
                args.push(named);
            } else {
                // Positional arg: math content up to `,` or `)`
                let content = self.parse_math_content(&[Token::Comma, Token::RParen])?;
                if !content.is_empty() {
                    args.push(MathArg::Positional(content));
                }
            }
            if *self.peek() == Token::Comma {
                self.advance();
            }
        }
        if *self.peek() == Token::RParen {
            self.advance();
        }
        Ok(args)
    }

    /// Try to parse an array argument (contains `;` separators).
    fn try_parse_array_arg(&mut self) -> Result<Option<MathArg>, Error> {
        // Scan ahead to see if there's a `;` before `)`.
        let mut depth = 0;
        let mut has_semi = false;
        for i in self.pos..self.tokens.len() {
            match &self.tokens[i].0 {
                Token::LParen | Token::LBrace | Token::LBracket => depth += 1,
                Token::RParen if depth > 0 => depth -= 1,
                Token::RParen => break,
                Token::RBrace | Token::RBracket if depth > 0 => depth -= 1,
                Token::Semicolon if depth == 0 => {
                    has_semi = true;
                    break;
                }
                Token::Eof => break,
                _ => {}
            }
        }

        if !has_semi {
            return Ok(None);
        }

        // Parse rows separated by `;`, cells separated by `,`
        let mut rows = Vec::new();
        loop {
            let row = self.parse_row()?;
            rows.push(row);
            if *self.peek() == Token::Semicolon {
                self.advance();
            } else {
                break;
            }
        }
        Ok(Some(MathArg::Array(rows)))
    }

    /// Parse a single row of cells separated by `,`.
    fn parse_row(&mut self) -> Result<Vec<Vec<MathNode>>, Error> {
        let mut cells = Vec::new();
        loop {
            let cell =
                self.parse_math_content(&[Token::Comma, Token::Semicolon, Token::RParen])?;
            cells.push(cell);
            if *self.peek() == Token::Comma {
                self.advance();
                // If next is `;` or `)`, this was a trailing comma
                if *self.peek() == Token::Semicolon || *self.peek() == Token::RParen {
                    break;
                }
            } else {
                break;
            }
        }
        Ok(cells)
    }

    /// Try to parse a named argument `ident: value`.
    fn try_parse_named_arg(&mut self) -> Result<Option<MathArg>, Error> {
        let saved_pos = self.pos;
        if let Token::Ident(name) = self.peek().clone() {
            self.advance();
            if *self.peek() == Token::Colon {
                self.advance();
                let content = self.parse_math_content(&[Token::Comma, Token::RParen])?;
                return Ok(Some(MathArg::Named(name, content)));
            }
            // Not a named arg, restore position
            self.pos = saved_pos;
        }
        Ok(None)
    }

    /// Parse math content up to (but not consuming) any of the stop tokens.
    fn parse_math_content(&mut self, stops: &[Token]) -> Result<Vec<MathNode>, Error> {
        let mut nodes = Vec::new();
        while !stops.contains(self.peek()) && *self.peek() != Token::Eof {
            if self.peek_space_before() && !nodes.is_empty() {
                nodes.push(MathNode::Space);
            }
            nodes.push(self.parse_expr(0)?);
        }
        Ok(nodes)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn is_callable(node: &MathNode) -> bool {
    // Only multi-letter identifiers are callable (function names).
    // Single-letter variables like `f` followed by `(x)` are NOT function calls
    // in Typst math — they are juxtaposition. This matches the Haskell parser's
    // behavior where `stLastMathTok` is checked: a single-letter Text is not callable.
    match node {
        MathNode::Ident(_) => true,
        _ => false,
    }
}

fn extract_name(node: &MathNode) -> String {
    match node {
        MathNode::Ident(s) => s.clone(),
        MathNode::Text(s) => s.clone(),
        _ => "unknown".into(),
    }
}

/// Remove outer parentheses from a group node, matching `hideOuterParens` in the Haskell code.
fn hide_outer_parens(node: MathNode) -> MathNode {
    match node {
        MathNode::Group {
            open: Some(ref o),
            close: Some(ref c),
            ref children,
        } if o == "(" && c == ")" => MathNode::Group {
            open: None,
            close: None,
            children: children.clone(),
        },
        other => other,
    }
}

/// Attach a subscript, merging with existing Attach nodes.
fn attach_bottom(base: MathNode, sub: MathNode) -> MathNode {
    match base {
        MathNode::Attach {
            base: b,
            bottom: None,
            top,
        } => MathNode::Attach {
            base: b,
            bottom: Some(Box::new(sub)),
            top,
        },
        _ => MathNode::Attach {
            base: Box::new(base),
            bottom: Some(Box::new(sub)),
            top: None,
        },
    }
}

/// Attach a superscript, merging with existing Attach nodes.
fn attach_top(base: MathNode, sup: MathNode) -> MathNode {
    match base {
        MathNode::Attach {
            base: b,
            bottom,
            top: None,
        } => MathNode::Attach {
            base: b,
            bottom,
            top: Some(Box::new(sup)),
        },
        _ => MathNode::Attach {
            base: Box::new(base),
            bottom: None,
            top: Some(Box::new(sup)),
        },
    }
}

/// Collapse adjacent Space nodes and trim leading/trailing spaces.
fn collapse_spaces(nodes: Vec<MathNode>) -> Vec<MathNode> {
    let mut result = Vec::new();
    for node in nodes {
        if node == MathNode::Space {
            if let Some(last) = result.last() {
                if *last != MathNode::Space {
                    result.push(MathNode::Space);
                }
            }
        } else {
            result.push(node);
        }
    }
    // Trim leading/trailing spaces
    while result.first() == Some(&MathNode::Space) {
        result.remove(0);
    }
    while result.last() == Some(&MathNode::Space) {
        result.pop();
    }
    result
}

/// Public parse function.
pub fn parse(input: &str) -> Result<Vec<MathNode>, Error> {
    let mut parser = Parser::new(input);
    parser.parse()
}
