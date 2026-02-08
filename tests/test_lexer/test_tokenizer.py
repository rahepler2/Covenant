"""Tests for the Covenant lexer/tokenizer."""

import pytest

from covenant.lexer.lexer import Lexer, LexerError
from covenant.lexer.tokens import TokenType


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def token_types(source: str) -> list[TokenType]:
    """Return just the token types (excluding NEWLINE/EOF for clarity)."""
    tokens = Lexer(source).tokenize()
    return [t.type for t in tokens]


def token_values(source: str) -> list[tuple[TokenType, str]]:
    """Return (type, value) pairs for all tokens."""
    tokens = Lexer(source).tokenize()
    return [(t.type, t.value) for t in tokens]


# ---------------------------------------------------------------------------
# Basic token recognition
# ---------------------------------------------------------------------------

class TestBasicTokens:
    def test_empty_source(self):
        tokens = Lexer("").tokenize()
        assert len(tokens) == 1
        assert tokens[0].type == TokenType.EOF

    def test_blank_lines_only(self):
        tokens = Lexer("\n\n\n").tokenize()
        assert tokens[-1].type == TokenType.EOF

    def test_keywords(self):
        source = "contract intent scope risk requires"
        tokens = Lexer(source).tokenize()
        types = [t.type for t in tokens if t.type not in (TokenType.NEWLINE, TokenType.EOF)]
        assert types == [
            TokenType.CONTRACT,
            TokenType.INTENT,
            TokenType.SCOPE,
            TokenType.RISK,
            TokenType.REQUIRES,
        ]

    def test_identifier(self):
        tokens = Lexer("myVariable").tokenize()
        non_structural = [t for t in tokens if t.type not in (TokenType.NEWLINE, TokenType.EOF)]
        assert len(non_structural) == 1
        assert non_structural[0].type == TokenType.IDENTIFIER
        assert non_structural[0].value == "myVariable"

    def test_string_literal(self):
        tokens = Lexer('"hello world"').tokenize()
        string_tokens = [t for t in tokens if t.type == TokenType.STRING]
        assert len(string_tokens) == 1
        assert string_tokens[0].value == "hello world"

    def test_string_escapes(self):
        tokens = Lexer(r'"hello\nworld"').tokenize()
        string_tokens = [t for t in tokens if t.type == TokenType.STRING]
        assert string_tokens[0].value == "hello\nworld"

    def test_integer_literal(self):
        tokens = Lexer("42").tokenize()
        int_tokens = [t for t in tokens if t.type == TokenType.INTEGER]
        assert len(int_tokens) == 1
        assert int_tokens[0].value == "42"

    def test_float_literal(self):
        tokens = Lexer("3.14").tokenize()
        float_tokens = [t for t in tokens if t.type == TokenType.FLOAT]
        assert len(float_tokens) == 1
        assert float_tokens[0].value == "3.14"

    def test_boolean_literals(self):
        tokens = Lexer("true false").tokenize()
        bool_tokens = [t for t in tokens if t.type in (TokenType.TRUE, TokenType.FALSE)]
        assert len(bool_tokens) == 2
        assert bool_tokens[0].type == TokenType.TRUE
        assert bool_tokens[1].type == TokenType.FALSE


# ---------------------------------------------------------------------------
# Operators and punctuation
# ---------------------------------------------------------------------------

class TestOperators:
    def test_arrow(self):
        tokens = Lexer("->").tokenize()
        assert any(t.type == TokenType.ARROW for t in tokens)

    def test_comparison_operators(self):
        source = "== != < <= > >="
        tokens = Lexer(source).tokenize()
        op_types = [t.type for t in tokens if t.type not in (TokenType.NEWLINE, TokenType.EOF)]
        assert op_types == [
            TokenType.EQUALS,
            TokenType.NOT_EQUALS,
            TokenType.LESS_THAN,
            TokenType.LESS_EQUAL,
            TokenType.GREATER_THAN,
            TokenType.GREATER_EQUAL,
        ]

    def test_arithmetic_operators(self):
        source = "+ - * /"
        tokens = Lexer(source).tokenize()
        op_types = [t.type for t in tokens if t.type not in (TokenType.NEWLINE, TokenType.EOF)]
        assert op_types == [
            TokenType.PLUS,
            TokenType.MINUS,
            TokenType.STAR,
            TokenType.SLASH,
        ]

    def test_brackets_and_parens(self):
        source = "( ) [ ]"
        tokens = Lexer(source).tokenize()
        op_types = [t.type for t in tokens if t.type not in (TokenType.NEWLINE, TokenType.EOF)]
        assert op_types == [
            TokenType.LPAREN,
            TokenType.RPAREN,
            TokenType.LBRACKET,
            TokenType.RBRACKET,
        ]

    def test_dot_comma_colon(self):
        source = ". , :"
        tokens = Lexer(source).tokenize()
        op_types = [t.type for t in tokens if t.type not in (TokenType.NEWLINE, TokenType.EOF)]
        assert op_types == [TokenType.DOT, TokenType.COMMA, TokenType.COLON]

    def test_assign_vs_equals(self):
        source = "= =="
        tokens = Lexer(source).tokenize()
        op_types = [t.type for t in tokens if t.type not in (TokenType.NEWLINE, TokenType.EOF)]
        assert op_types == [TokenType.ASSIGN, TokenType.EQUALS]


# ---------------------------------------------------------------------------
# Indentation
# ---------------------------------------------------------------------------

class TestIndentation:
    def test_single_indent(self):
        source = "contract\n  body\n"
        tokens = Lexer(source).tokenize()
        types = [t.type for t in tokens]
        assert TokenType.INDENT in types
        assert TokenType.DEDENT in types

    def test_nested_indent(self):
        source = "a\n  b\n    c\n"
        tokens = Lexer(source).tokenize()
        types = [t.type for t in tokens]
        indent_count = types.count(TokenType.INDENT)
        dedent_count = types.count(TokenType.DEDENT)
        assert indent_count == 2
        assert dedent_count == 2

    def test_dedent_multiple_levels(self):
        source = "a\n  b\n    c\nd\n"
        tokens = Lexer(source).tokenize()
        types = [t.type for t in tokens]
        # Going from indent 4 -> 0 should produce 2 DEDENTs
        dedent_count = types.count(TokenType.DEDENT)
        assert dedent_count == 2

    def test_odd_indentation_error(self):
        source = "a\n   b\n"  # 3 spaces, not a multiple of 2
        with pytest.raises(LexerError, match="multiple of 2"):
            Lexer(source).tokenize()

    def test_tab_error(self):
        source = "a\n\tb\n"
        with pytest.raises(LexerError, match="Tabs are not allowed"):
            Lexer(source).tokenize()

    def test_eof_produces_remaining_dedents(self):
        source = "a\n  b\n    c"
        tokens = Lexer(source).tokenize()
        # At EOF, remaining indentation should be closed
        dedent_count = sum(1 for t in tokens if t.type == TokenType.DEDENT)
        assert dedent_count == 2


# ---------------------------------------------------------------------------
# Comments
# ---------------------------------------------------------------------------

class TestComments:
    def test_comment_only_line(self):
        source = "-- this is a comment\n"
        tokens = Lexer(source).tokenize()
        non_eof = [t for t in tokens if t.type != TokenType.EOF]
        assert len(non_eof) == 0

    def test_inline_comment(self):
        source = "contract -- a comment\n"
        tokens = Lexer(source).tokenize()
        types = [t.type for t in tokens if t.type not in (TokenType.NEWLINE, TokenType.EOF)]
        assert types == [TokenType.CONTRACT]

    def test_comment_preserves_tokens_before_it(self):
        source = "a = 42 -- assign\n"
        tokens = Lexer(source).tokenize()
        types = [t.type for t in tokens if t.type not in (TokenType.NEWLINE, TokenType.EOF)]
        assert TokenType.IDENTIFIER in types
        assert TokenType.ASSIGN in types
        assert TokenType.INTEGER in types


# ---------------------------------------------------------------------------
# Source location tracking
# ---------------------------------------------------------------------------

class TestSourceLocations:
    def test_line_numbers(self):
        source = "a\nb\nc\n"
        tokens = Lexer(source).tokenize()
        idents = [t for t in tokens if t.type == TokenType.IDENTIFIER]
        assert idents[0].line == 1
        assert idents[1].line == 2
        assert idents[2].line == 3

    def test_column_numbers(self):
        source = "ab cd ef"
        tokens = Lexer(source).tokenize()
        idents = [t for t in tokens if t.type == TokenType.IDENTIFIER]
        assert idents[0].column == 1
        assert idents[1].column == 4
        assert idents[2].column == 7

    def test_filename_propagated(self):
        tokens = Lexer("x", filename="test.cov").tokenize()
        assert all(t.file == "test.cov" for t in tokens)


# ---------------------------------------------------------------------------
# Integration: real Covenant snippets
# ---------------------------------------------------------------------------

class TestRealSnippets:
    def test_intent_line(self):
        source = 'intent: "Transfer funds"\n'
        tokens = Lexer(source).tokenize()
        types = [t.type for t in tokens if t.type not in (TokenType.NEWLINE, TokenType.EOF)]
        assert types == [TokenType.INTENT, TokenType.COLON, TokenType.STRING]

    def test_risk_line(self):
        source = "risk: high\n"
        tokens = Lexer(source).tokenize()
        types = [t.type for t in tokens if t.type not in (TokenType.NEWLINE, TokenType.EOF)]
        assert types == [TokenType.RISK, TokenType.COLON, TokenType.HIGH]

    def test_contract_signature(self):
        source = "contract transfer(from: Account, to: Account) -> Result\n"
        tokens = Lexer(source).tokenize()
        types = [t.type for t in tokens if t.type not in (TokenType.NEWLINE, TokenType.EOF)]
        assert types == [
            TokenType.CONTRACT,
            TokenType.IDENTIFIER,  # transfer
            TokenType.LPAREN,
            TokenType.IDENTIFIER,  # from
            TokenType.COLON,
            TokenType.IDENTIFIER,  # Account
            TokenType.COMMA,
            TokenType.IDENTIFIER,  # to
            TokenType.COLON,
            TokenType.IDENTIFIER,  # Account
            TokenType.RPAREN,
            TokenType.ARROW,
            TokenType.IDENTIFIER,  # Result
        ]

    def test_effects_block(self):
        source = (
            "effects:\n"
            "  modifies [from.balance, to.balance]\n"
            "  emits TransferEvent\n"
            "  touches_nothing_else\n"
        )
        tokens = Lexer(source).tokenize()
        types = [t.type for t in tokens if t.type not in (TokenType.NEWLINE, TokenType.EOF)]
        assert TokenType.EFFECTS in types
        assert TokenType.MODIFIES in types
        assert TokenType.EMITS in types
        assert TokenType.TOUCHES_NOTHING_ELSE in types

    def test_old_expression(self):
        source = "old(from.balance) - amount\n"
        tokens = Lexer(source).tokenize()
        types = [t.type for t in tokens if t.type not in (TokenType.NEWLINE, TokenType.EOF)]
        assert types[0] == TokenType.OLD
        assert types[1] == TokenType.LPAREN

    def test_has_keyword(self):
        source = "owner has auth.session\n"
        tokens = Lexer(source).tokenize()
        types = [t.type for t in tokens if t.type not in (TokenType.NEWLINE, TokenType.EOF)]
        assert TokenType.HAS in types


# ---------------------------------------------------------------------------
# Error cases
# ---------------------------------------------------------------------------

class TestErrors:
    def test_unterminated_string(self):
        with pytest.raises(LexerError, match="Unterminated string"):
            Lexer('"hello').tokenize()

    def test_unexpected_character(self):
        with pytest.raises(LexerError, match="Unexpected character"):
            Lexer("@").tokenize()
