/// Parser for LaTeX math expressions.
///
/// Hand-written recursive descent parser that tokenizes LaTeX math and
/// produces an AST suitable for Typst emission.

use super::ast::LatexNode;

// ---------------------------------------------------------------------------
// Tokeniser
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Token {
    /// `\commandname` (including the backslash).
    Command(String),
    /// A single letter variable.
    Letter(char),
    /// A digit sequence (possibly with decimal point).
    Number(String),
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `^`
    Caret,
    /// `_`
    Underscore,
    /// `&`
    Ampersand,
    /// `\\` (double backslash linebreak)
    Linebreak,
    /// A single punctuation/operator character.
    Char(char),
    /// Whitespace (collapsed).
    Space,
    /// End of input.
    Eof,
}

struct Lexer {
    chars: Vec<char>,
    pos: usize,
}

impl Lexer {
    fn new(input: &str) -> Self {
        Lexer {
            chars: input.chars().collect(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();

        while let Some(c) = self.peek() {
            match c {
                // Whitespace
                _ if c.is_whitespace() => {
                    while self.peek().is_some_and(|c| c.is_whitespace()) {
                        self.advance();
                    }
                    tokens.push(Token::Space);
                }

                // Backslash: command or linebreak or escaped char
                '\\' => {
                    self.advance();
                    match self.peek() {
                        // Double backslash: linebreak
                        Some('\\') => {
                            self.advance();
                            tokens.push(Token::Linebreak);
                        }
                        // Command: \letters
                        Some(c) if c.is_ascii_alphabetic() => {
                            let mut cmd = String::from('\\');
                            while self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
                                cmd.push(self.advance().unwrap());
                            }
                            tokens.push(Token::Command(cmd));
                        }
                        // Escaped special char: \{ \} \| \, \; \: \! \  \#  \% \& \_
                        Some(ch) => {
                            self.advance();
                            let mut cmd = String::from('\\');
                            cmd.push(ch);
                            tokens.push(Token::Command(cmd));
                        }
                        None => {
                            tokens.push(Token::Command("\\".into()));
                        }
                    }
                }

                // Grouping
                '{' => {
                    self.advance();
                    tokens.push(Token::LBrace);
                }
                '}' => {
                    self.advance();
                    tokens.push(Token::RBrace);
                }

                // Sub/superscript
                '^' => {
                    self.advance();
                    tokens.push(Token::Caret);
                }
                '_' => {
                    self.advance();
                    tokens.push(Token::Underscore);
                }

                // Alignment
                '&' => {
                    self.advance();
                    tokens.push(Token::Ampersand);
                }

                // Numbers
                _ if c.is_ascii_digit() => {
                    let mut num = String::new();
                    while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                        num.push(self.advance().unwrap());
                    }
                    if self.peek() == Some('.') {
                        num.push(self.advance().unwrap());
                        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                            num.push(self.advance().unwrap());
                        }
                    }
                    tokens.push(Token::Number(num));
                }

                // Letters (single-char math variables)
                _ if c.is_ascii_alphabetic() => {
                    self.advance();
                    tokens.push(Token::Letter(c));
                }

                // Everything else: operators, punctuation
                _ => {
                    self.advance();
                    tokens.push(Token::Char(c));
                }
            }
        }

        tokens.push(Token::Eof);
        tokens
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

pub(super) struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(input: &str) -> Self {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize();
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens.get(self.pos).cloned().unwrap_or(Token::Eof);
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    fn expect(&mut self, expected: &Token) -> bool {
        if self.peek() == expected {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Skip whitespace tokens.
    fn skip_space(&mut self) {
        while *self.peek() == Token::Space {
            self.advance();
        }
    }

    /// Parse the entire math expression.
    pub fn parse(&mut self) -> Vec<LatexNode> {
        self.parse_sequence(&[Token::Eof])
    }

    /// Parse nodes until hitting a stop token (not consumed).
    fn parse_sequence(&mut self, stops: &[Token]) -> Vec<LatexNode> {
        let mut nodes = Vec::new();
        while !stops.contains(self.peek()) && *self.peek() != Token::Eof {
            if *self.peek() == Token::Space {
                self.advance();
                if !nodes.is_empty() {
                    nodes.push(LatexNode::Space);
                }
                continue;
            }
            nodes.push(self.parse_expr());
        }
        trim_spaces(&mut nodes);
        nodes
    }

    /// Parse a single expression (atom + optional sub/superscript).
    fn parse_expr(&mut self) -> LatexNode {
        let mut base = self.parse_atom();

        // Collect sub/superscripts. Peek past whitespace but only consume
        // it if we actually find a sub/superscript operator.
        loop {
            let saved = self.pos;
            self.skip_space();
            match self.peek() {
                Token::Underscore => {
                    self.advance();
                    let sub = self.parse_atom();
                    base = attach_sub(base, sub);
                }
                Token::Caret => {
                    self.advance();
                    let sup = self.parse_atom();
                    base = attach_sup(base, sup);
                }
                Token::Command(cmd) if cmd == "\\limits" => {
                    self.advance();
                    base = LatexNode::Limits(Box::new(base));
                }
                Token::Command(cmd) if cmd == "\\nolimits" => {
                    self.advance();
                    base = LatexNode::Scripts(Box::new(base));
                }
                _ => {
                    // Not a sub/superscript: restore position so spaces are preserved
                    self.pos = saved;
                    break;
                }
            }
        }

        base
    }

    /// Parse a single atom (the smallest unit).
    fn parse_atom(&mut self) -> LatexNode {
        self.skip_space();
        match self.peek().clone() {
            Token::Letter(c) => {
                self.advance();
                LatexNode::Letter(c)
            }
            Token::Number(n) => {
                self.advance();
                LatexNode::Number(n)
            }
            Token::Char(c) => {
                self.advance();
                LatexNode::Char(c)
            }
            Token::LBrace => self.parse_braced_group(),
            Token::Command(cmd) => self.parse_command(&cmd.clone()),
            Token::Ampersand => {
                self.advance();
                LatexNode::AlignPoint
            }
            Token::Linebreak => {
                self.advance();
                LatexNode::Linebreak
            }
            _ => {
                // Unexpected token, consume and emit as raw text
                let tok = self.advance();
                LatexNode::Raw(format!("{:?}", tok))
            }
        }
    }

    /// Parse `{...}` braced group.
    fn parse_braced_group(&mut self) -> LatexNode {
        self.expect(&Token::LBrace);
        let children = self.parse_sequence(&[Token::RBrace]);
        self.expect(&Token::RBrace);
        LatexNode::Group(children)
    }

    /// Parse a single argument (either a braced group or a single atom).
    fn parse_arg(&mut self) -> LatexNode {
        self.skip_space();
        if *self.peek() == Token::LBrace {
            self.parse_braced_group()
        } else {
            self.parse_atom()
        }
    }

    /// Parse an optional `[...]` argument. Returns None if not present.
    fn parse_opt_arg(&mut self) -> Option<LatexNode> {
        self.skip_space();
        if *self.peek() == Token::Char('[') {
            self.advance();
            let children = self.parse_sequence(&[Token::Char(']')]);
            if *self.peek() == Token::Char(']') {
                self.advance();
            }
            Some(LatexNode::Group(children))
        } else {
            None
        }
    }

    /// Parse a command and its arguments.
    fn parse_command(&mut self, cmd: &str) -> LatexNode {
        self.advance(); // consume the command token

        match cmd {
            // Fractions
            "\\frac" | "\\dfrac" | "\\tfrac" => {
                let num = self.parse_arg();
                let den = self.parse_arg();
                LatexNode::Frac(Box::new(num), Box::new(den))
            }

            // Roots
            "\\sqrt" => {
                let opt = self.parse_opt_arg();
                let body = self.parse_arg();
                match opt {
                    Some(index) => LatexNode::Root(Box::new(index), Box::new(body)),
                    None => LatexNode::Sqrt(Box::new(body)),
                }
            }

            // Binomial
            "\\binom" | "\\dbinom" | "\\tbinom" => {
                let upper = self.parse_arg();
                let lower = self.parse_arg();
                LatexNode::Binom(Box::new(upper), Box::new(lower))
            }

            // Left/right delimiters
            "\\left" => self.parse_left_right(),

            // \middle delimiter
            "\\middle" => {
                let delim = self.parse_delimiter();
                LatexNode::Middle(delim)
            }

            // Text
            "\\text" | "\\textrm" | "\\textit" | "\\textbf" | "\\mbox" | "\\hbox" => {
                let body = self.parse_arg();
                let text = extract_text(&body);
                LatexNode::Text(text)
            }

            // Operator name
            "\\operatorname" => {
                let body = self.parse_arg();
                let name = extract_text(&body);
                LatexNode::OperatorName(name)
            }

            // Style commands
            "\\mathbf" | "\\mathit" | "\\mathrm" | "\\mathsf" | "\\mathtt"
            | "\\mathbb" | "\\mathcal" | "\\mathfrak" | "\\boldsymbol" | "\\bm" => {
                let body = self.parse_arg();
                LatexNode::Style(cmd.to_string(), Box::new(body))
            }

            // Accents (single-argument)
            "\\hat" | "\\tilde" | "\\dot" | "\\ddot" | "\\dddot" | "\\ddddot"
            | "\\acute" | "\\grave" | "\\bar" | "\\breve" | "\\check"
            | "\\mathring" | "\\vec" => {
                let body = self.parse_arg();
                LatexNode::Accent(cmd.to_string(), Box::new(body))
            }

            // Wide accents (also single-argument, treated as accents)
            "\\widehat" | "\\widetilde" => {
                let body = self.parse_arg();
                let accent = if cmd == "\\widehat" { "\\hat" } else { "\\tilde" };
                LatexNode::Accent(accent.to_string(), Box::new(body))
            }

            // Over/under arrows
            "\\overrightarrow" | "\\overleftarrow" | "\\overleftrightarrow" => {
                let body = self.parse_arg();
                LatexNode::Accent(cmd.to_string(), Box::new(body))
            }

            // Overline / underline
            "\\overline" => {
                let body = self.parse_arg();
                LatexNode::UnaryFunc("overline".into(), Box::new(body))
            }
            "\\underline" => {
                let body = self.parse_arg();
                LatexNode::UnaryFunc("underline".into(), Box::new(body))
            }

            // Cancel
            "\\cancel" => {
                let body = self.parse_arg();
                LatexNode::UnaryFunc("cancel".into(), Box::new(body))
            }
            "\\bcancel" => {
                let body = self.parse_arg();
                LatexNode::CancelInverted(Box::new(body))
            }
            "\\xcancel" => {
                let body = self.parse_arg();
                LatexNode::CancelCross(Box::new(body))
            }

            // Over/underbrace with optional annotation
            "\\overbrace" => {
                let body = self.parse_arg();
                LatexNode::OverUnderBrace("overbrace".into(), Box::new(body))
            }
            "\\underbrace" => {
                let body = self.parse_arg();
                LatexNode::OverUnderBrace("underbrace".into(), Box::new(body))
            }
            "\\overbracket" => {
                let body = self.parse_arg();
                LatexNode::OverUnderBrace("overbracket".into(), Box::new(body))
            }
            "\\underbracket" => {
                let body = self.parse_arg();
                LatexNode::OverUnderBrace("underbracket".into(), Box::new(body))
            }

            // Display styles
            "\\displaystyle" => {
                let body = self.parse_arg();
                LatexNode::DisplayStyle("display".into(), Box::new(body))
            }
            "\\textstyle" => {
                let body = self.parse_arg();
                LatexNode::DisplayStyle("inline".into(), Box::new(body))
            }
            "\\scriptstyle" => {
                let body = self.parse_arg();
                LatexNode::DisplayStyle("script".into(), Box::new(body))
            }
            "\\scriptscriptstyle" => {
                let body = self.parse_arg();
                LatexNode::DisplayStyle("sscript".into(), Box::new(body))
            }

            // Spacing commands
            "\\," => LatexNode::TypstIdent("thin".into()),
            "\\:" | "\\>" => LatexNode::TypstIdent("med".into()),
            "\\;" => LatexNode::TypstIdent("thick".into()),
            "\\!" => LatexNode::TypstIdent("thin".into()), // negative thin space
            "\\quad" => LatexNode::TypstIdent("quad".into()),
            "\\qquad" => LatexNode::TypstIdent("wide".into()),
            "\\ " => LatexNode::Space,

            // Escaped characters
            "\\{" => LatexNode::Char('{'),
            "\\}" => LatexNode::Char('}'),
            "\\#" => LatexNode::Char('#'),
            "\\%" => LatexNode::Char('%'),
            "\\&" => LatexNode::Char('&'),
            "\\$" => LatexNode::Char('$'),
            "\\_" => LatexNode::Char('_'),

            // Environments
            "\\begin" => {
                let env_name = extract_text(&self.parse_arg());
                self.parse_environment(&env_name)
            }

            // Phantom (invisible, but occupies space)
            "\\phantom" | "\\hphantom" | "\\vphantom" => {
                let _body = self.parse_arg();
                LatexNode::Raw(String::new())
            }

            // \not prefix (negation)
            "\\not" => {
                let next = self.parse_atom();
                LatexNode::Not(Box::new(next))
            }

            // Everything else: look up in symbol/operator maps
            _ => LatexNode::Command(cmd.to_string()),
        }
    }

    /// Parse `\left<delim> ... \right<delim>`.
    fn parse_left_right(&mut self) -> LatexNode {
        let open = self.parse_delimiter();
        let content = self.parse_sequence_until_right();
        let close = if self.peek() == &Token::Command("\\right".into()) {
            self.advance();
            self.parse_delimiter()
        } else {
            ".".into()
        };
        LatexNode::LeftRight(open, close, content)
    }

    /// Parse until `\right` (at the same nesting level).
    fn parse_sequence_until_right(&mut self) -> Vec<LatexNode> {
        let mut nodes = Vec::new();
        loop {
            match self.peek() {
                Token::Command(cmd) if cmd == "\\right" => break,
                Token::Eof => break,
                Token::Space => {
                    self.advance();
                    if !nodes.is_empty() {
                        nodes.push(LatexNode::Space);
                    }
                }
                _ => {
                    nodes.push(self.parse_expr());
                }
            }
        }
        trim_spaces(&mut nodes);
        nodes
    }

    /// Parse a delimiter character after `\left`, `\right`, or `\middle`.
    fn parse_delimiter(&mut self) -> String {
        self.skip_space();
        match self.peek().clone() {
            Token::Char(c) => {
                self.advance();
                c.to_string()
            }
            Token::Command(cmd) => {
                self.advance();
                match cmd.as_str() {
                    "\\{" | "\\lbrace" => "{".into(),
                    "\\}" | "\\rbrace" => "}".into(),
                    "\\|" | "\\Vert" => "||".into(),
                    "\\vert" => "|".into(),
                    "\\langle" => "angle.l".into(),
                    "\\rangle" => "angle.r".into(),
                    "\\lfloor" => "floor.l".into(),
                    "\\rfloor" => "floor.r".into(),
                    "\\lceil" => "ceil.l".into(),
                    "\\rceil" => "ceil.r".into(),
                    "\\llbracket" => "bracket.l.double".into(),
                    "\\rrbracket" => "bracket.r.double".into(),
                    _ => cmd,
                }
            }
            _ => ".".into(),
        }
    }

    /// Parse a LaTeX environment: matrix, cases, aligned, etc.
    fn parse_environment(&mut self, name: &str) -> LatexNode {
        match name {
            "pmatrix" | "bmatrix" | "Bmatrix" | "vmatrix" | "Vmatrix" | "matrix"
            | "smallmatrix" => {
                let rows = self.parse_env_rows(name);
                LatexNode::Matrix(name.to_string(), rows)
            }
            "cases" | "rcases" => {
                let rows = self.parse_env_rows(name);
                LatexNode::Cases(name.to_string(), rows)
            }
            "aligned" | "align" | "align*" | "split" | "gathered" => {
                let rows = self.parse_env_rows(name);
                LatexNode::Aligned(name.to_string(), rows)
            }
            "equation" | "equation*" | "displaymath" => {
                let content = self.parse_env_body(name);
                LatexNode::Group(content)
            }
            _ => {
                // Unknown environment: parse body and emit raw
                let content = self.parse_env_body(name);
                LatexNode::Group(content)
            }
        }
    }

    /// Parse rows of a matrix/cases/aligned environment until `\end{name}`.
    fn parse_env_rows(&mut self, name: &str) -> Vec<Vec<Vec<LatexNode>>> {
        let end_cmd = format!("\\end");
        let mut rows = Vec::new();
        let mut current_row: Vec<Vec<LatexNode>> = Vec::new();
        let mut current_cell: Vec<LatexNode> = Vec::new();

        loop {
            match self.peek() {
                Token::Command(cmd) if cmd == &end_cmd => {
                    // Finalize
                    if !current_cell.is_empty() || !current_row.is_empty() {
                        trim_spaces(&mut current_cell);
                        current_row.push(current_cell);
                        rows.push(current_row);
                    }
                    self.advance(); // consume \end
                    let env_name = extract_text(&self.parse_arg());
                    debug_assert_eq!(env_name, name, "mismatched \\end{{{}}}", env_name);
                    break;
                }
                Token::Eof => {
                    if !current_cell.is_empty() || !current_row.is_empty() {
                        trim_spaces(&mut current_cell);
                        current_row.push(current_cell);
                        rows.push(current_row);
                    }
                    break;
                }
                Token::Ampersand => {
                    self.advance();
                    trim_spaces(&mut current_cell);
                    current_row.push(std::mem::take(&mut current_cell));
                }
                Token::Linebreak => {
                    self.advance();
                    trim_spaces(&mut current_cell);
                    current_row.push(std::mem::take(&mut current_cell));
                    rows.push(std::mem::take(&mut current_row));
                }
                Token::Space => {
                    self.advance();
                    if !current_cell.is_empty() {
                        current_cell.push(LatexNode::Space);
                    }
                }
                _ => {
                    current_cell.push(self.parse_expr());
                }
            }
        }
        rows
    }

    /// Parse the body of an environment until `\end{name}`.
    fn parse_env_body(&mut self, _name: &str) -> Vec<LatexNode> {
        let end_cmd = "\\end".to_string();
        let mut nodes = Vec::new();

        loop {
            match self.peek() {
                Token::Command(cmd) if cmd == &end_cmd => {
                    self.advance();
                    let _env_name = self.parse_arg();
                    break;
                }
                Token::Eof => break,
                Token::Space => {
                    self.advance();
                    if !nodes.is_empty() {
                        nodes.push(LatexNode::Space);
                    }
                }
                _ => {
                    nodes.push(self.parse_expr());
                }
            }
        }
        trim_spaces(&mut nodes);
        nodes
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Attach subscript to a base, merging with existing Attach nodes.
fn attach_sub(base: LatexNode, sub: LatexNode) -> LatexNode {
    match base {
        LatexNode::Attach { base: b, sub: None, sup } => {
            LatexNode::Attach { base: b, sub: Some(Box::new(sub)), sup }
        }
        LatexNode::OverUnderBrace(kind, body) => {
            // \underbrace{x+y}_{text} → underbrace annotation
            LatexNode::OverUnderBraceAnnotated(kind, body, Box::new(sub))
        }
        _ => {
            LatexNode::Attach { base: Box::new(base), sub: Some(Box::new(sub)), sup: None }
        }
    }
}

/// Attach superscript to a base, merging with existing Attach nodes.
fn attach_sup(base: LatexNode, sup: LatexNode) -> LatexNode {
    match base {
        LatexNode::Attach { base: b, sub, sup: None } => {
            LatexNode::Attach { base: b, sub, sup: Some(Box::new(sup)) }
        }
        LatexNode::OverUnderBrace(kind, body) => {
            // \overbrace{x+y}^{text} → overbrace annotation
            LatexNode::OverUnderBraceAnnotated(kind, body, Box::new(sup))
        }
        _ => {
            LatexNode::Attach { base: Box::new(base), sub: None, sup: Some(Box::new(sup)) }
        }
    }
}

/// Extract plain text from a node tree (for \text{}, \operatorname{}, env names).
fn extract_text(node: &LatexNode) -> String {
    match node {
        LatexNode::Letter(c) => c.to_string(),
        LatexNode::Number(n) => n.clone(),
        LatexNode::Char(c) => c.to_string(),
        LatexNode::Space => " ".into(),
        LatexNode::Group(children) => {
            children.iter().map(extract_text).collect()
        }
        LatexNode::Command(cmd) => cmd.clone(),
        _ => String::new(),
    }
}

/// Trim leading and trailing Space nodes.
fn trim_spaces(nodes: &mut Vec<LatexNode>) {
    while nodes.first() == Some(&LatexNode::Space) {
        nodes.remove(0);
    }
    while nodes.last() == Some(&LatexNode::Space) {
        nodes.pop();
    }
}

/// Public parse function.
pub fn parse(input: &str) -> Vec<LatexNode> {
    let mut parser = Parser::new(input);
    parser.parse()
}
