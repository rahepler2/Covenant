"""Covenant compiler CLI entry point.

Usage:
    covenant parse <file.cov>       Parse and display the AST
    covenant check <file.cov>       Run Stage 1 verification (parse + intent check)
    covenant tokenize <file.cov>    Display the token stream (debug)
"""

from __future__ import annotations

import json
import sys
from dataclasses import asdict
from pathlib import Path

from covenant.lexer.lexer import Lexer, LexerError
from covenant.parser.parser import Parser, ParseError


def main(argv: list[str] | None = None) -> int:
    args = argv if argv is not None else sys.argv[1:]

    if len(args) < 1:
        print(__doc__.strip())
        return 1

    command = args[0]

    if command in ("--help", "-h"):
        print(__doc__.strip())
        return 0

    if command == "--version":
        from covenant import __version__
        print(f"covenant {__version__}")
        return 0

    if len(args) < 2:
        print(f"Error: command '{command}' requires a file argument")
        return 1

    filepath = Path(args[1])
    if not filepath.exists():
        print(f"Error: file not found: {filepath}")
        return 1

    source = filepath.read_text(encoding="utf-8")
    filename = str(filepath)

    if command == "tokenize":
        return _cmd_tokenize(source, filename)
    elif command == "parse":
        return _cmd_parse(source, filename)
    elif command == "check":
        return _cmd_check(source, filename)
    else:
        print(f"Error: unknown command '{command}'")
        print(__doc__.strip())
        return 1


def _cmd_tokenize(source: str, filename: str) -> int:
    """Display the token stream."""
    try:
        tokens = Lexer(source, filename).tokenize()
    except LexerError as e:
        print(f"Lexer error: {e}")
        return 1

    for tok in tokens:
        print(tok)
    return 0


def _cmd_parse(source: str, filename: str) -> int:
    """Parse the file and display the AST summary."""
    try:
        tokens = Lexer(source, filename).tokenize()
        program = Parser(tokens, filename).parse()
    except (LexerError, ParseError) as e:
        print(f"Error: {e}")
        return 1

    _print_program(program)
    return 0


def _cmd_check(source: str, filename: str) -> int:
    """Stage 1 verification: parse + structural checks."""
    try:
        tokens = Lexer(source, filename).tokenize()
        program = Parser(tokens, filename).parse()
    except (LexerError, ParseError) as e:
        print(f"FAIL: {e}")
        return 1

    issues = _check_program(program, filename)
    if issues:
        for issue in issues:
            print(f"  WARNING: {issue}")
        print(f"\n{filename}: {len(issues)} warning(s)")
    else:
        print(f"{filename}: OK")
    return 0


def _check_program(program, filename: str) -> list[str]:
    """Run basic structural checks on the parsed program."""
    issues = []

    for contract in program.contracts:
        loc = f"{filename}:{contract.loc.line}"

        if contract.body is None:
            issues.append(f"{loc}: contract '{contract.name}' has no body")

        if contract.precondition is None:
            issues.append(
                f"{loc}: contract '{contract.name}' has no precondition — "
                f"every contract should declare what must be true before execution"
            )

        if contract.postcondition is None:
            issues.append(
                f"{loc}: contract '{contract.name}' has no postcondition — "
                f"every contract should declare what will be true after execution"
            )

        if contract.effects is None:
            issues.append(
                f"{loc}: contract '{contract.name}' has no effects declaration — "
                f"every contract must declare its side effects"
            )

    if program.header is None:
        issues.append(f"{filename}: no file header — consider adding intent and risk declarations")
    else:
        if program.header.intent is None:
            issues.append(f"{filename}: no intent declaration")
        if program.header.risk is None:
            issues.append(f"{filename}: no risk level declared")

    return issues


def _print_program(program) -> None:
    """Pretty-print a program AST."""
    if program.header:
        h = program.header
        if h.intent:
            print(f"Intent: \"{h.intent.text}\"")
        if h.scope:
            print(f"Scope:  {h.scope.path}")
        if h.risk:
            print(f"Risk:   {h.risk.level.name.lower()}")
        if h.requires:
            print(f"Requires: {', '.join(h.requires.capabilities)}")
        print()

    for td in program.type_defs:
        print(f"Type: {td.name} = {td.base_type}")
        for f in td.fields:
            print(f"  field: {f.name}: {_type_str(f.type_expr)}")
        for fc in td.flow_constraints:
            print(f"  flow: {fc}")
        print()

    for sd in program.shared_decls:
        print(f"Shared: {sd.name}: {sd.type_name}")
        print(f"  access: {sd.access}, isolation: {sd.isolation}, audit: {sd.audit}")
        print()

    for c in program.contracts:
        params = ", ".join(f"{p.name}: {_type_str(p.type_expr)}" for p in c.params)
        ret = _type_str(c.return_type) if c.return_type else "?"
        print(f"Contract: {c.name}({params}) -> {ret}")
        if c.precondition:
            print(f"  preconditions: {len(c.precondition.conditions)}")
        if c.postcondition:
            print(f"  postconditions: {len(c.postcondition.conditions)}")
        if c.effects:
            print(f"  effects: {len(c.effects.declarations)}")
        if c.permissions:
            print(f"  permissions: defined")
        if c.body:
            print(f"  body: {len(c.body.statements)} statement(s)")
        if c.on_failure:
            print(f"  on_failure: {len(c.on_failure.statements)} statement(s)")
        print()


def _type_str(type_expr) -> str:
    """Format a type expression as a string."""
    from covenant.ast.nodes import SimpleType, AnnotatedType
    if isinstance(type_expr, AnnotatedType):
        base = _type_str(type_expr.base)
        anns = ", ".join(type_expr.annotations)
        return f"{base} [{anns}]"
    if isinstance(type_expr, SimpleType):
        return type_expr.name
    return str(type_expr)


if __name__ == "__main__":
    sys.exit(main())
