"""Covenant lexer — hand-written tokenizer with indentation-sensitive scanning.

Design decisions:
- Fixed 2-space indentation (tabs are errors).
- INDENT/DEDENT tokens emitted for each level change.
- Comments (-- ...) are discarded, not tokenized.
- Produces a flat token stream consumed by the parser.
"""

from __future__ import annotations

from covenant.lexer.tokens import KEYWORDS, Token, TokenType


class LexerError(Exception):
    """Raised on lexical errors with source location."""

    def __init__(self, message: str, line: int, column: int, file: str = "<unknown>"):
        self.line = line
        self.column = column
        self.file = file
        super().__init__(f"{file}:{line}:{column}: {message}")


class Lexer:
    """Tokenizes Covenant source code into a stream of `Token` objects.

    Usage::

        lexer = Lexer(source_text, filename="example.cov")
        tokens = lexer.tokenize()
    """

    INDENT_WIDTH = 2

    def __init__(self, source: str, filename: str = "<unknown>") -> None:
        self.source = source
        self.filename = filename
        self.pos = 0
        self.line = 1
        self.column = 1
        self.tokens: list[Token] = []
        self.indent_stack: list[int] = [0]

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    def tokenize(self) -> list[Token]:
        """Tokenize the entire source and return the token list."""
        self.tokens = []
        self.indent_stack = [0]
        self.pos = 0
        self.line = 1
        self.column = 1

        while self.pos < len(self.source):
            self._scan_line()

        # Emit remaining DEDENTs at EOF
        while len(self.indent_stack) > 1:
            self.indent_stack.pop()
            self.tokens.append(self._make_token(TokenType.DEDENT, ""))

        self.tokens.append(self._make_token(TokenType.EOF, ""))
        return self.tokens

    # ------------------------------------------------------------------
    # Line-level scanning
    # ------------------------------------------------------------------

    def _scan_line(self) -> None:
        """Process one logical line (indentation + content + newline)."""
        # Skip completely blank lines
        if self._at_end():
            return

        # Measure leading spaces
        indent = 0
        line_start_pos = self.pos
        while not self._at_end() and self._peek() == " ":
            indent += 1
            self._advance()

        # Skip blank lines and comment-only lines
        if self._at_end() or self._peek() == "\n":
            if not self._at_end():
                self._advance()  # consume newline
            return
        if self._peek() == "-" and self._peek_ahead(1) == "-":
            self._skip_comment()
            if not self._at_end() and self._peek() == "\n":
                self._advance()
            return

        # Tab check
        if not self._at_end() and self._peek() == "\t":
            raise LexerError(
                "Tabs are not allowed — use 2-space indentation",
                self.line, self.column, self.filename,
            )

        # Validate indent is a multiple of INDENT_WIDTH
        if indent % self.INDENT_WIDTH != 0:
            raise LexerError(
                f"Indentation must be a multiple of {self.INDENT_WIDTH} spaces, "
                f"got {indent}",
                self.line, self.column, self.filename,
            )

        # Emit INDENT / DEDENT tokens
        current = self.indent_stack[-1]
        if indent > current:
            self.indent_stack.append(indent)
            self.tokens.append(self._make_token(TokenType.INDENT, ""))
        elif indent < current:
            while self.indent_stack[-1] > indent:
                self.indent_stack.pop()
                self.tokens.append(self._make_token(TokenType.DEDENT, ""))
            if self.indent_stack[-1] != indent:
                raise LexerError(
                    f"Dedent to level {indent} does not match any outer indentation level",
                    self.line, self.column, self.filename,
                )

        # Scan tokens on this line
        while not self._at_end() and self._peek() != "\n":
            self._skip_spaces()
            if self._at_end() or self._peek() == "\n":
                break
            # Skip comments
            if self._peek() == "-" and self._peek_ahead(1) == "-":
                self._skip_comment()
                break
            self._scan_token()

        # Consume newline
        if not self._at_end() and self._peek() == "\n":
            self.tokens.append(self._make_token(TokenType.NEWLINE, "\n"))
            self._advance()

    # ------------------------------------------------------------------
    # Token scanning
    # ------------------------------------------------------------------

    def _scan_token(self) -> None:
        """Scan a single token at the current position."""
        ch = self._peek()

        # String literal
        if ch == '"':
            self._scan_string()
            return

        # Number literal
        if ch.isdigit():
            self._scan_number()
            return

        # Two-character operators
        if ch == "-" and self._peek_ahead(1) == ">":
            self.tokens.append(self._make_token(TokenType.ARROW, "->"))
            self._advance()
            self._advance()
            return

        if ch == "=" and self._peek_ahead(1) == "=":
            self.tokens.append(self._make_token(TokenType.EQUALS, "=="))
            self._advance()
            self._advance()
            return

        if ch == "!" and self._peek_ahead(1) == "=":
            self.tokens.append(self._make_token(TokenType.NOT_EQUALS, "!="))
            self._advance()
            self._advance()
            return

        if ch == "<" and self._peek_ahead(1) == "=":
            self.tokens.append(self._make_token(TokenType.LESS_EQUAL, "<="))
            self._advance()
            self._advance()
            return

        if ch == ">" and self._peek_ahead(1) == "=":
            self.tokens.append(self._make_token(TokenType.GREATER_EQUAL, ">="))
            self._advance()
            self._advance()
            return

        # Single-character tokens
        single_char_tokens = {
            "(": TokenType.LPAREN,
            ")": TokenType.RPAREN,
            "[": TokenType.LBRACKET,
            "]": TokenType.RBRACKET,
            ",": TokenType.COMMA,
            ":": TokenType.COLON,
            ".": TokenType.DOT,
            "+": TokenType.PLUS,
            "-": TokenType.MINUS,
            "*": TokenType.STAR,
            "/": TokenType.SLASH,
            "<": TokenType.LESS_THAN,
            ">": TokenType.GREATER_THAN,
            "=": TokenType.ASSIGN,
        }

        if ch in single_char_tokens:
            self.tokens.append(self._make_token(single_char_tokens[ch], ch))
            self._advance()
            return

        # Identifiers and keywords
        if ch.isalpha() or ch == "_":
            self._scan_identifier()
            return

        raise LexerError(
            f"Unexpected character: {ch!r}",
            self.line, self.column, self.filename,
        )

    def _scan_string(self) -> None:
        """Scan a double-quoted string literal."""
        start_line = self.line
        start_col = self.column
        self._advance()  # consume opening quote
        chars: list[str] = []

        while not self._at_end() and self._peek() != '"':
            if self._peek() == "\n":
                raise LexerError(
                    "Unterminated string literal",
                    start_line, start_col, self.filename,
                )
            if self._peek() == "\\" and not self._at_end():
                self._advance()  # consume backslash
                escaped = self._peek()
                escape_map = {"n": "\n", "t": "\t", "\\": "\\", '"': '"'}
                chars.append(escape_map.get(escaped, escaped))
            else:
                chars.append(self._peek())
            self._advance()

        if self._at_end():
            raise LexerError(
                "Unterminated string literal",
                start_line, start_col, self.filename,
            )

        self._advance()  # consume closing quote
        value = "".join(chars)
        self.tokens.append(Token(TokenType.STRING, value, start_line, start_col, self.filename))

    def _scan_number(self) -> None:
        """Scan an integer or float literal."""
        start_col = self.column
        num_chars: list[str] = []

        while not self._at_end() and (self._peek().isdigit() or self._peek() == "."):
            num_chars.append(self._peek())
            self._advance()

        value = "".join(num_chars)
        if "." in value:
            token_type = TokenType.FLOAT
        else:
            token_type = TokenType.INTEGER

        self.tokens.append(Token(token_type, value, self.line, start_col, self.filename))

    def _scan_identifier(self) -> None:
        """Scan an identifier or keyword."""
        start_col = self.column
        chars: list[str] = []

        while not self._at_end() and (self._peek().isalnum() or self._peek() == "_"):
            chars.append(self._peek())
            self._advance()

        word = "".join(chars)
        token_type = KEYWORDS.get(word, TokenType.IDENTIFIER)
        self.tokens.append(Token(token_type, word, self.line, start_col, self.filename))

    # ------------------------------------------------------------------
    # Helpers
    # ------------------------------------------------------------------

    def _peek(self) -> str:
        """Return the current character without consuming it."""
        return self.source[self.pos]

    def _peek_ahead(self, offset: int) -> str | None:
        """Return a character at an offset ahead, or None if past end."""
        idx = self.pos + offset
        if idx >= len(self.source):
            return None
        return self.source[idx]

    def _advance(self) -> str:
        """Consume and return the current character."""
        ch = self.source[self.pos]
        self.pos += 1
        if ch == "\n":
            self.line += 1
            self.column = 1
        else:
            self.column += 1
        return ch

    def _at_end(self) -> bool:
        return self.pos >= len(self.source)

    def _skip_spaces(self) -> None:
        """Skip horizontal whitespace (spaces only, not newlines)."""
        while not self._at_end() and self._peek() == " ":
            self._advance()

    def _skip_comment(self) -> None:
        """Skip from -- to end of line."""
        while not self._at_end() and self._peek() != "\n":
            self._advance()

    def _make_token(self, token_type: TokenType, value: str) -> Token:
        return Token(token_type, value, self.line, self.column, self.filename)
