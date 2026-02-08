pub mod tokens;

use tokens::{keyword_type, Token, TokenType};
use std::fmt;

#[derive(Debug)]
pub struct LexerError {
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub file: String,
}

impl fmt::Display for LexerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}: {}", self.file, self.line, self.column, self.message)
    }
}

impl std::error::Error for LexerError {}

const INDENT_WIDTH: usize = 2;

pub struct Lexer {
    source: Vec<char>,
    filename: String,
    pos: usize,
    line: usize,
    column: usize,
    tokens: Vec<Token>,
    indent_stack: Vec<usize>,
}

impl Lexer {
    pub fn new(source: &str, filename: &str) -> Self {
        Self {
            source: source.chars().collect(),
            filename: filename.to_string(),
            pos: 0,
            line: 1,
            column: 1,
            tokens: Vec::new(),
            indent_stack: vec![0],
        }
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, LexerError> {
        while self.pos < self.source.len() {
            self.scan_line()?;
        }

        // Emit remaining DEDENTs at EOF
        while self.indent_stack.len() > 1 {
            self.indent_stack.pop();
            self.tokens.push(self.make_token(TokenType::Dedent, ""));
        }

        self.tokens.push(self.make_token(TokenType::Eof, ""));
        Ok(self.tokens)
    }

    fn scan_line(&mut self) -> Result<(), LexerError> {
        if self.at_end() {
            return Ok(());
        }

        // Measure leading spaces
        let mut indent = 0;
        while !self.at_end() && self.peek() == ' ' {
            indent += 1;
            self.advance();
        }

        // Skip blank lines and comment-only lines
        if self.at_end() || self.peek() == '\n' {
            if !self.at_end() {
                self.advance();
            }
            return Ok(());
        }
        if self.peek() == '-' && self.peek_ahead(1) == Some('-') {
            self.skip_comment();
            if !self.at_end() && self.peek() == '\n' {
                self.advance();
            }
            return Ok(());
        }

        // Tab check
        if !self.at_end() && self.peek() == '\t' {
            return Err(LexerError {
                message: "Tabs are not allowed — use 2-space indentation".to_string(),
                line: self.line,
                column: self.column,
                file: self.filename.clone(),
            });
        }

        // Validate indent is a multiple of INDENT_WIDTH
        if indent % INDENT_WIDTH != 0 {
            return Err(LexerError {
                message: format!(
                    "Indentation must be a multiple of {} spaces, got {}",
                    INDENT_WIDTH, indent
                ),
                line: self.line,
                column: self.column,
                file: self.filename.clone(),
            });
        }

        // Emit INDENT / DEDENT tokens
        let current = *self.indent_stack.last().unwrap();
        if indent > current {
            self.indent_stack.push(indent);
            self.tokens.push(self.make_token(TokenType::Indent, ""));
        } else if indent < current {
            while *self.indent_stack.last().unwrap() > indent {
                self.indent_stack.pop();
                self.tokens.push(self.make_token(TokenType::Dedent, ""));
            }
            if *self.indent_stack.last().unwrap() != indent {
                return Err(LexerError {
                    message: format!(
                        "Dedent to level {} does not match any outer indentation level",
                        indent
                    ),
                    line: self.line,
                    column: self.column,
                    file: self.filename.clone(),
                });
            }
        }

        // Scan tokens on this line
        while !self.at_end() && self.peek() != '\n' {
            self.skip_spaces();
            if self.at_end() || self.peek() == '\n' {
                break;
            }
            if self.peek() == '-' && self.peek_ahead(1) == Some('-') {
                self.skip_comment();
                break;
            }
            self.scan_token()?;
        }

        // Consume newline
        if !self.at_end() && self.peek() == '\n' {
            self.tokens.push(self.make_token(TokenType::Newline, "\n"));
            self.advance();
        }

        Ok(())
    }

    fn scan_token(&mut self) -> Result<(), LexerError> {
        let ch = self.peek();

        // String literal
        if ch == '"' {
            return self.scan_string();
        }

        // Number literal
        if ch.is_ascii_digit() {
            return self.scan_number();
        }

        // Two-character operators
        if ch == '-' && self.peek_ahead(1) == Some('>') {
            self.tokens.push(self.make_token(TokenType::Arrow, "->"));
            self.advance();
            self.advance();
            return Ok(());
        }
        if ch == '=' && self.peek_ahead(1) == Some('=') {
            self.tokens.push(self.make_token(TokenType::Equals, "=="));
            self.advance();
            self.advance();
            return Ok(());
        }
        if ch == '!' && self.peek_ahead(1) == Some('=') {
            self.tokens.push(self.make_token(TokenType::NotEquals, "!="));
            self.advance();
            self.advance();
            return Ok(());
        }
        if ch == '<' && self.peek_ahead(1) == Some('=') {
            self.tokens.push(self.make_token(TokenType::LessEqual, "<="));
            self.advance();
            self.advance();
            return Ok(());
        }
        if ch == '>' && self.peek_ahead(1) == Some('=') {
            self.tokens.push(self.make_token(TokenType::GreaterEqual, ">="));
            self.advance();
            self.advance();
            return Ok(());
        }

        // Single-character tokens
        let single = match ch {
            '(' => Some(TokenType::LParen),
            ')' => Some(TokenType::RParen),
            '[' => Some(TokenType::LBracket),
            ']' => Some(TokenType::RBracket),
            ',' => Some(TokenType::Comma),
            ':' => Some(TokenType::Colon),
            '.' => Some(TokenType::Dot),
            '+' => Some(TokenType::Plus),
            '-' => Some(TokenType::Minus),
            '*' => Some(TokenType::Star),
            '/' => Some(TokenType::Slash),
            '<' => Some(TokenType::LessThan),
            '>' => Some(TokenType::GreaterThan),
            '=' => Some(TokenType::Assign),
            _ => None,
        };

        if let Some(tt) = single {
            let s = ch.to_string();
            self.tokens.push(self.make_token(tt, &s));
            self.advance();
            return Ok(());
        }

        // Identifiers and keywords
        if ch.is_alphabetic() || ch == '_' {
            return self.scan_identifier();
        }

        Err(LexerError {
            message: format!("Unexpected character: {:?}", ch),
            line: self.line,
            column: self.column,
            file: self.filename.clone(),
        })
    }

    fn scan_string(&mut self) -> Result<(), LexerError> {
        let start_line = self.line;
        let start_col = self.column;
        self.advance(); // consume opening quote
        let mut chars = String::new();

        while !self.at_end() && self.peek() != '"' {
            if self.peek() == '\n' {
                return Err(LexerError {
                    message: "Unterminated string literal".to_string(),
                    line: start_line,
                    column: start_col,
                    file: self.filename.clone(),
                });
            }
            if self.peek() == '\\' && !self.at_end() {
                self.advance(); // consume backslash
                if !self.at_end() {
                    let escaped = self.peek();
                    match escaped {
                        'n' => chars.push('\n'),
                        't' => chars.push('\t'),
                        '\\' => chars.push('\\'),
                        '"' => chars.push('"'),
                        other => chars.push(other),
                    }
                }
            } else {
                chars.push(self.peek());
            }
            self.advance();
        }

        if self.at_end() {
            return Err(LexerError {
                message: "Unterminated string literal".to_string(),
                line: start_line,
                column: start_col,
                file: self.filename.clone(),
            });
        }

        self.advance(); // consume closing quote
        self.tokens.push(Token {
            token_type: TokenType::StringLit,
            value: chars,
            line: start_line,
            column: start_col,
            file: self.filename.clone(),
        });
        Ok(())
    }

    fn scan_number(&mut self) -> Result<(), LexerError> {
        let start_col = self.column;
        let start_line = self.line;
        let mut num_chars = String::new();
        let mut dot_count = 0;

        while !self.at_end() && (self.peek().is_ascii_digit() || self.peek() == '.') {
            if self.peek() == '.' {
                dot_count += 1;
                if dot_count > 1 {
                    break; // Stop before second dot — treat it as field access
                }
                // Only consume dot if followed by a digit (otherwise it's field access like `1.method()`)
                if self.peek_ahead(1).map_or(true, |c| !c.is_ascii_digit()) {
                    break;
                }
            }
            num_chars.push(self.peek());
            self.advance();
        }

        if num_chars.is_empty() {
            return Err(LexerError {
                message: "Invalid number literal".to_string(),
                line: start_line,
                column: start_col,
                file: self.filename.clone(),
            });
        }

        let tt = if num_chars.contains('.') {
            TokenType::Float
        } else {
            TokenType::Integer
        };

        self.tokens.push(Token {
            token_type: tt,
            value: num_chars,
            line: self.line,
            column: start_col,
            file: self.filename.clone(),
        });
        Ok(())
    }

    fn scan_identifier(&mut self) -> Result<(), LexerError> {
        let start_col = self.column;
        let mut word = String::new();

        while !self.at_end() && (self.peek().is_alphanumeric() || self.peek() == '_') {
            word.push(self.peek());
            self.advance();
        }

        let tt = keyword_type(&word).unwrap_or(TokenType::Identifier);
        self.tokens.push(Token {
            token_type: tt,
            value: word,
            line: self.line,
            column: start_col,
            file: self.filename.clone(),
        });
        Ok(())
    }

    // ── Helpers ─────────────────────────────────────────────────────────

    fn peek(&self) -> char {
        self.source[self.pos]
    }

    fn peek_ahead(&self, offset: usize) -> Option<char> {
        let idx = self.pos + offset;
        if idx >= self.source.len() {
            None
        } else {
            Some(self.source[idx])
        }
    }

    fn advance(&mut self) -> char {
        let ch = self.source[self.pos];
        self.pos += 1;
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        ch
    }

    fn at_end(&self) -> bool {
        self.pos >= self.source.len()
    }

    fn skip_spaces(&mut self) {
        while !self.at_end() && self.peek() == ' ' {
            self.advance();
        }
    }

    fn skip_comment(&mut self) {
        while !self.at_end() && self.peek() != '\n' {
            self.advance();
        }
    }

    fn make_token(&self, token_type: TokenType, value: &str) -> Token {
        Token {
            token_type,
            value: value.to_string(),
            line: self.line,
            column: self.column,
            file: self.filename.clone(),
        }
    }
}
