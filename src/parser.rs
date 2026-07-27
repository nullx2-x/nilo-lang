use std::collections::HashSet;

use crate::ast::{
    AssignTarget, Block, Expr, ExprKind, Field, Literal, Parameter, Program, Span, Stmt, TypeRef,
};
use crate::error::{NiloError, Result};
use crate::token::{Token, TokenKind, TokenLiteral};

pub struct Parser<'a> {
    tokens: Vec<Token>,
    index: usize,
    filename: String,
    source: &'a str,
    function_depth: usize,
    loop_depth: usize,
}

impl<'a> Parser<'a> {
    #[must_use]
    pub fn new(tokens: Vec<Token>, filename: impl Into<String>, source: &'a str) -> Self {
        Self {
            tokens,
            index: 0,
            filename: filename.into(),
            source,
            function_depth: 0,
            loop_depth: 0,
        }
    }

    pub fn parse(mut self) -> Result<Program> {
        let mut statements = Vec::new();
        while !self.check(TokenKind::Eof) {
            statements.push(self.declaration()?);
        }
        Ok(Program { statements })
    }

    fn declaration(&mut self) -> Result<Stmt> {
        let export_token = if self.matches(TokenKind::Export) {
            Some(self.previous().clone())
        } else {
            None
        };
        let exported = export_token.is_some();

        if self.matches(TokenKind::Import) {
            if exported {
                return Err(self.error(
                    export_token.as_ref().expect("export token exists"),
                    "import declarations cannot be exported",
                ));
            }
            return self.import_statement(self.previous().span);
        }
        if self.matches(TokenKind::From) {
            if exported {
                return Err(self.error(
                    export_token.as_ref().expect("export token exists"),
                    "from-import declarations cannot be exported",
                ));
            }
            return self.from_import_statement(self.previous().span);
        }
        if self.matches(TokenKind::Let) {
            return self.let_declaration(exported, self.previous().span);
        }
        if self.matches(TokenKind::Func) {
            return self.function_declaration(exported, self.previous().span);
        }
        if self.matches(TokenKind::Type) {
            return self.type_declaration(exported, self.previous().span);
        }
        if let Some(token) = export_token {
            return Err(self.error(&token, "export must be followed by let, func, or type"));
        }
        self.statement()
    }

    fn import_statement(&mut self, start: Span) -> Result<Stmt> {
        let path = self.consume_string("expected a quoted module path after import")?;
        let alias = if self.matches(TokenKind::As) {
            Some(
                self.consume(TokenKind::Identifier, "expected an alias after 'as'")?
                    .lexeme,
            )
        } else {
            None
        };
        let end = self.consume(TokenKind::Semicolon, "expected ';' after import")?;
        Ok(Stmt::Import {
            path,
            alias,
            span: start.merge(end.span),
        })
    }

    fn from_import_statement(&mut self, start: Span) -> Result<Stmt> {
        let path = self.consume_string("expected a quoted module path after from")?;
        self.consume(TokenKind::Import, "expected 'import' after module path")?;
        let mut names = Vec::new();
        let mut seen = HashSet::new();
        loop {
            let name = self
                .consume(TokenKind::Identifier, "expected an imported name")?
                .lexeme;
            if !seen.insert(name.clone()) {
                return Err(self.error(self.previous(), format!("duplicate import name '{name}'")));
            }
            names.push(name);
            if !self.matches(TokenKind::Comma) {
                break;
            }
        }
        let end = self.consume(TokenKind::Semicolon, "expected ';' after import")?;
        Ok(Stmt::FromImport {
            path,
            names,
            span: start.merge(end.span),
        })
    }

    fn let_declaration(&mut self, exported: bool, start: Span) -> Result<Stmt> {
        let name = self.consume(TokenKind::Identifier, "expected a variable name")?;
        let ty = if self.matches(TokenKind::Colon) {
            Some(self.type_ref()?)
        } else {
            None
        };
        self.consume(TokenKind::Assign, "expected '=' in variable declaration")?;
        let value = self.expression()?;
        let end = self.consume(TokenKind::Semicolon, "expected ';' after declaration")?;
        Ok(Stmt::Let {
            name: name.lexeme,
            ty,
            value,
            exported,
            span: start.merge(end.span),
        })
    }

    fn function_declaration(&mut self, exported: bool, start: Span) -> Result<Stmt> {
        let name = self.consume(TokenKind::Identifier, "expected a function name")?;
        self.consume(TokenKind::LeftParen, "expected '(' after function name")?;
        let mut params = Vec::new();
        let mut seen = HashSet::new();
        if !self.check(TokenKind::RightParen) {
            loop {
                if params.len() >= 255 {
                    return Err(self.error(self.current(), "functions may have at most 255 parameters"));
                }
                let parameter = self.consume(TokenKind::Identifier, "expected a parameter name")?;
                if !seen.insert(parameter.lexeme.clone()) {
                    return Err(self.error(
                        &parameter,
                        format!("duplicate parameter '{}'", parameter.lexeme),
                    ));
                }
                let ty = if self.matches(TokenKind::Colon) {
                    Some(self.type_ref()?)
                } else {
                    None
                };
                params.push(Parameter {
                    name: parameter.lexeme,
                    ty,
                    span: parameter.span,
                });
                if !self.matches(TokenKind::Comma) {
                    break;
                }
                if self.check(TokenKind::RightParen) {
                    break;
                }
            }
        }
        self.consume(TokenKind::RightParen, "expected ')' after parameters")?;
        let return_type = if self.matches(TokenKind::Arrow) {
            Some(self.type_ref()?)
        } else {
            None
        };

        self.function_depth += 1;
        let body_result = self.block();
        self.function_depth -= 1;
        let (body, end) = body_result?;
        Ok(Stmt::Function {
            name: name.lexeme,
            params,
            return_type,
            body,
            exported,
            span: start.merge(end),
        })
    }

    fn type_declaration(&mut self, exported: bool, start: Span) -> Result<Stmt> {
        let name = self.consume(TokenKind::Identifier, "expected a type name")?;
        self.consume(TokenKind::LeftBrace, "expected '{' after type name")?;
        let mut fields = Vec::new();
        let mut seen = HashSet::new();
        while !self.check(TokenKind::RightBrace) && !self.check(TokenKind::Eof) {
            let field = self.consume(TokenKind::Identifier, "expected a field name")?;
            if !seen.insert(field.lexeme.clone()) {
                return Err(self.error(&field, format!("duplicate field '{}'", field.lexeme)));
            }
            let ty = if self.matches(TokenKind::Colon) {
                Some(self.type_ref()?)
            } else {
                None
            };
            fields.push(Field {
                name: field.lexeme,
                ty,
                span: field.span,
            });
            if !self.matches(TokenKind::Comma) && !self.matches(TokenKind::Semicolon) {
                if !self.check(TokenKind::RightBrace) {
                    return Err(self.error(self.current(), "expected ',', ';', or '}' after field"));
                }
            }
        }
        let end = self.consume(TokenKind::RightBrace, "expected '}' after type fields")?;
        self.matches(TokenKind::Semicolon);
        Ok(Stmt::TypeDecl {
            name: name.lexeme,
            fields,
            exported,
            span: start.merge(end.span),
        })
    }

    fn statement(&mut self) -> Result<Stmt> {
        if self.matches(TokenKind::Return) {
            return self.return_statement(self.previous().span);
        }
        if self.matches(TokenKind::If) {
            return self.if_statement(self.previous().span);
        }
        if self.matches(TokenKind::While) {
            return self.while_statement(self.previous().span);
        }
        if self.matches(TokenKind::For) {
            return self.for_statement(self.previous().span);
        }
        if self.matches(TokenKind::Break) {
            let token = self.previous().clone();
            if self.loop_depth == 0 {
                return Err(self.error(&token, "break may only be used inside a loop"));
            }
            let end = self.consume(TokenKind::Semicolon, "expected ';' after break")?;
            return Ok(Stmt::Break {
                span: token.span.merge(end.span),
            });
        }
        if self.matches(TokenKind::Continue) {
            let token = self.previous().clone();
            if self.loop_depth == 0 {
                return Err(self.error(&token, "continue may only be used inside a loop"));
            }
            let end = self.consume(TokenKind::Semicolon, "expected ';' after continue")?;
            return Ok(Stmt::Continue {
                span: token.span.merge(end.span),
            });
        }
        self.expression_or_assignment_statement()
    }

    fn return_statement(&mut self, start: Span) -> Result<Stmt> {
        if self.function_depth == 0 {
            return Err(self.error(self.previous(), "return may only be used inside a function"));
        }
        let value = if self.check(TokenKind::Semicolon) {
            None
        } else {
            Some(self.expression()?)
        };
        let end = self.consume(TokenKind::Semicolon, "expected ';' after return value")?;
        Ok(Stmt::Return {
            value,
            span: start.merge(end.span),
        })
    }

    fn if_statement(&mut self, start: Span) -> Result<Stmt> {
        self.consume(TokenKind::LeftParen, "expected '(' after if")?;
        let condition = self.expression()?;
        self.consume(TokenKind::RightParen, "expected ')' after condition")?;
        let (then_block, then_end) = self.block()?;
        let mut end = then_end;
        let else_block = if self.matches(TokenKind::Else) {
            if self.matches(TokenKind::If) {
                let nested_start = self.previous().span;
                let nested = self.if_statement(nested_start)?;
                end = nested.span();
                Some(vec![nested])
            } else {
                let (block, block_end) = self.block()?;
                end = block_end;
                Some(block)
            }
        } else {
            None
        };
        Ok(Stmt::If {
            condition,
            then_block,
            else_block,
            span: start.merge(end),
        })
    }

    fn while_statement(&mut self, start: Span) -> Result<Stmt> {
        self.consume(TokenKind::LeftParen, "expected '(' after while")?;
        let condition = self.expression()?;
        self.consume(TokenKind::RightParen, "expected ')' after condition")?;
        self.loop_depth += 1;
        let body_result = self.block();
        self.loop_depth -= 1;
        let (body, end) = body_result?;
        Ok(Stmt::While {
            condition,
            body,
            span: start.merge(end),
        })
    }

    fn for_statement(&mut self, start: Span) -> Result<Stmt> {
        let name = self.consume(TokenKind::Identifier, "expected a loop variable after for")?;
        self.consume(TokenKind::In, "expected 'in' after loop variable")?;
        let iterable = self.expression()?;
        self.loop_depth += 1;
        let body_result = self.block();
        self.loop_depth -= 1;
        let (body, end) = body_result?;
        Ok(Stmt::For {
            name: name.lexeme,
            iterable,
            body,
            span: start.merge(end),
        })
    }

    fn expression_or_assignment_statement(&mut self) -> Result<Stmt> {
        let expression = self.expression()?;
        if self.matches(TokenKind::Assign) {
            let target = self.assignment_target(expression)?;
            let value = self.expression()?;
            let end = self.consume(TokenKind::Semicolon, "expected ';' after assignment")?;
            let span = target.span().merge(end.span);
            return Ok(Stmt::Assign {
                target,
                value,
                span,
            });
        }
        let end = self.consume(TokenKind::Semicolon, "expected ';' after expression")?;
        let span = expression.span.merge(end.span);
        Ok(Stmt::Expression { expression, span })
    }

    fn assignment_target(&self, expression: Expr) -> Result<AssignTarget> {
        let span = expression.span;
        match expression.kind {
            ExprKind::Variable { name } => Ok(AssignTarget::Variable { name, span }),
            ExprKind::Get { object, name } => Ok(AssignTarget::Property { object, name, span }),
            ExprKind::Index { object, index } => Ok(AssignTarget::Index {
                object,
                index,
                span,
            }),
            _ => Err(NiloError::parse("invalid assignment target").at(
                &self.filename,
                span,
                Some(self.source),
            )),
        }
    }

    fn block(&mut self) -> Result<(Block, Span)> {
        self.consume(TokenKind::LeftBrace, "expected '{'")?;
        let mut statements = Vec::new();
        while !self.check(TokenKind::RightBrace) && !self.check(TokenKind::Eof) {
            statements.push(self.declaration()?);
        }
        let end = self.consume(TokenKind::RightBrace, "expected '}' after block")?;
        Ok((statements, end.span))
    }

    fn expression(&mut self) -> Result<Expr> {
        self.or()
    }

    fn or(&mut self) -> Result<Expr> {
        let mut expression = self.and()?;
        while self.matches(TokenKind::OrOr) {
            let operator = self.previous().clone();
            let right = self.and()?;
            let span = expression.span.merge(right.span);
            expression = Expr::new(
                ExprKind::Binary {
                    left: Box::new(expression),
                    operator: operator.lexeme,
                    right: Box::new(right),
                },
                span,
            );
        }
        Ok(expression)
    }

    fn and(&mut self) -> Result<Expr> {
        let mut expression = self.equality()?;
        while self.matches(TokenKind::AndAnd) {
            let operator = self.previous().clone();
            let right = self.equality()?;
            let span = expression.span.merge(right.span);
            expression = Expr::new(
                ExprKind::Binary {
                    left: Box::new(expression),
                    operator: operator.lexeme,
                    right: Box::new(right),
                },
                span,
            );
        }
        Ok(expression)
    }

    fn equality(&mut self) -> Result<Expr> {
        let mut expression = self.comparison()?;
        while self.matches_any(&[TokenKind::EqualEqual, TokenKind::BangEqual]) {
            let operator = self.previous().clone();
            let right = self.comparison()?;
            let span = expression.span.merge(right.span);
            expression = Expr::new(
                ExprKind::Binary {
                    left: Box::new(expression),
                    operator: operator.lexeme,
                    right: Box::new(right),
                },
                span,
            );
        }
        Ok(expression)
    }

    fn comparison(&mut self) -> Result<Expr> {
        let mut expression = self.term()?;
        while self.matches_any(&[
            TokenKind::Less,
            TokenKind::LessEqual,
            TokenKind::Greater,
            TokenKind::GreaterEqual,
        ]) {
            let operator = self.previous().clone();
            let right = self.term()?;
            let span = expression.span.merge(right.span);
            expression = Expr::new(
                ExprKind::Binary {
                    left: Box::new(expression),
                    operator: operator.lexeme,
                    right: Box::new(right),
                },
                span,
            );
        }
        Ok(expression)
    }

    fn term(&mut self) -> Result<Expr> {
        let mut expression = self.factor()?;
        while self.matches_any(&[TokenKind::Plus, TokenKind::Minus]) {
            let operator = self.previous().clone();
            let right = self.factor()?;
            let span = expression.span.merge(right.span);
            expression = Expr::new(
                ExprKind::Binary {
                    left: Box::new(expression),
                    operator: operator.lexeme,
                    right: Box::new(right),
                },
                span,
            );
        }
        Ok(expression)
    }

    fn factor(&mut self) -> Result<Expr> {
        let mut expression = self.unary()?;
        while self.matches_any(&[TokenKind::Star, TokenKind::Slash, TokenKind::Percent]) {
            let operator = self.previous().clone();
            let right = self.unary()?;
            let span = expression.span.merge(right.span);
            expression = Expr::new(
                ExprKind::Binary {
                    left: Box::new(expression),
                    operator: operator.lexeme,
                    right: Box::new(right),
                },
                span,
            );
        }
        Ok(expression)
    }

    fn unary(&mut self) -> Result<Expr> {
        if self.matches_any(&[TokenKind::Bang, TokenKind::Minus]) {
            let operator = self.previous().clone();
            let right = self.unary()?;
            let span = operator.span.merge(right.span);
            return Ok(Expr::new(
                ExprKind::Unary {
                    operator: operator.lexeme,
                    right: Box::new(right),
                },
                span,
            ));
        }
        self.postfix()
    }

    fn postfix(&mut self) -> Result<Expr> {
        let mut expression = self.primary()?;
        loop {
            if self.matches(TokenKind::LeftParen) {
                let mut args = Vec::new();
                if !self.check(TokenKind::RightParen) {
                    loop {
                        if args.len() >= 255 {
                            return Err(self.error(
                                self.current(),
                                "calls may have at most 255 arguments",
                            ));
                        }
                        args.push(self.expression()?);
                        if !self.matches(TokenKind::Comma) {
                            break;
                        }
                        if self.check(TokenKind::RightParen) {
                            break;
                        }
                    }
                }
                let end = self.consume(TokenKind::RightParen, "expected ')' after arguments")?;
                let span = expression.span.merge(end.span);
                expression = Expr::new(
                    ExprKind::Call {
                        callee: Box::new(expression),
                        args,
                    },
                    span,
                );
            } else if self.matches(TokenKind::Dot) {
                let name = self.consume(TokenKind::Identifier, "expected a property name after '.'")?;
                let span = expression.span.merge(name.span);
                expression = Expr::new(
                    ExprKind::Get {
                        object: Box::new(expression),
                        name: name.lexeme,
                    },
                    span,
                );
            } else if self.matches(TokenKind::LeftBracket) {
                let index = self.expression()?;
                let end = self.consume(TokenKind::RightBracket, "expected ']' after index")?;
                let span = expression.span.merge(end.span);
                expression = Expr::new(
                    ExprKind::Index {
                        object: Box::new(expression),
                        index: Box::new(index),
                    },
                    span,
                );
            } else {
                break;
            }
        }
        Ok(expression)
    }

    fn primary(&mut self) -> Result<Expr> {
        if self.matches(TokenKind::Int) {
            let token = self.previous().clone();
            let value = match token.literal {
                Some(TokenLiteral::Int(value)) => value,
                _ => unreachable!("integer token must carry an integer"),
            };
            return Ok(Expr::new(
                ExprKind::Literal {
                    value: Literal::Int(value),
                },
                token.span,
            ));
        }
        if self.matches(TokenKind::Float) {
            let token = self.previous().clone();
            let value = match token.literal {
                Some(TokenLiteral::Float(value)) => value,
                _ => unreachable!("float token must carry a float"),
            };
            return Ok(Expr::new(
                ExprKind::Literal {
                    value: Literal::Float(value),
                },
                token.span,
            ));
        }
        if self.matches(TokenKind::String) {
            let token = self.previous().clone();
            let value = match token.literal {
                Some(TokenLiteral::String(value)) => value,
                _ => unreachable!("string token must carry a string"),
            };
            return Ok(Expr::new(
                ExprKind::Literal {
                    value: Literal::String(value),
                },
                token.span,
            ));
        }
        if self.matches(TokenKind::True) {
            return Ok(Expr::new(
                ExprKind::Literal {
                    value: Literal::Bool(true),
                },
                self.previous().span,
            ));
        }
        if self.matches(TokenKind::False) {
            return Ok(Expr::new(
                ExprKind::Literal {
                    value: Literal::Bool(false),
                },
                self.previous().span,
            ));
        }
        if self.matches(TokenKind::Nil) {
            return Ok(Expr::new(
                ExprKind::Literal {
                    value: Literal::Nil,
                },
                self.previous().span,
            ));
        }
        if self.matches(TokenKind::Identifier) {
            let token = self.previous().clone();
            return Ok(Expr::new(
                ExprKind::Variable { name: token.lexeme },
                token.span,
            ));
        }
        if self.matches(TokenKind::LeftBracket) {
            return self.list_literal(self.previous().span);
        }
        if self.matches(TokenKind::LeftBrace) {
            return self.map_literal(self.previous().span);
        }
        if self.matches(TokenKind::LeftParen) {
            let start = self.previous().span;
            let mut expression = self.expression()?;
            let end = self.consume(TokenKind::RightParen, "expected ')' after expression")?;
            expression.span = start.merge(end.span);
            return Ok(expression);
        }
        Err(self.error(self.current(), "expected an expression"))
    }

    fn list_literal(&mut self, start: Span) -> Result<Expr> {
        let mut values = Vec::new();
        if !self.check(TokenKind::RightBracket) {
            loop {
                values.push(self.expression()?);
                if !self.matches(TokenKind::Comma) {
                    break;
                }
                if self.check(TokenKind::RightBracket) {
                    break;
                }
            }
        }
        let end = self.consume(TokenKind::RightBracket, "expected ']' after list")?;
        Ok(Expr::new(
            ExprKind::List { values },
            start.merge(end.span),
        ))
    }

    fn map_literal(&mut self, start: Span) -> Result<Expr> {
        let mut entries = Vec::new();
        if !self.check(TokenKind::RightBrace) {
            loop {
                let key = self.expression()?;
                self.consume(TokenKind::Colon, "expected ':' between map key and value")?;
                let value = self.expression()?;
                entries.push((key, value));
                if !self.matches(TokenKind::Comma) {
                    break;
                }
                if self.check(TokenKind::RightBrace) {
                    break;
                }
            }
        }
        let end = self.consume(TokenKind::RightBrace, "expected '}' after map")?;
        Ok(Expr::new(
            ExprKind::Map { entries },
            start.merge(end.span),
        ))
    }

    fn type_ref(&mut self) -> Result<TypeRef> {
        let name = self.consume(TokenKind::Identifier, "expected a type name")?;
        let mut args = Vec::new();
        let mut end = name.span;
        if self.matches(TokenKind::Less) {
            if self.check(TokenKind::Greater) {
                return Err(self.error(self.current(), "generic type arguments cannot be empty"));
            }
            loop {
                args.push(self.type_ref()?);
                if !self.matches(TokenKind::Comma) {
                    break;
                }
            }
            end = self
                .consume(TokenKind::Greater, "expected '>' after type arguments")?
                .span;
        }
        let nullable = self.matches(TokenKind::Question);
        if nullable {
            end = self.previous().span;
        }
        Ok(TypeRef {
            name: name.lexeme,
            args,
            nullable,
            span: name.span.merge(end),
        })
    }

    fn consume_string(&mut self, message: &str) -> Result<String> {
        let token = self.consume(TokenKind::String, message)?;
        match token.literal {
            Some(TokenLiteral::String(value)) => Ok(value),
            _ => unreachable!("string token must carry a string"),
        }
    }

    fn matches(&mut self, kind: TokenKind) -> bool {
        if self.check(kind) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn matches_any(&mut self, kinds: &[TokenKind]) -> bool {
        if kinds.iter().any(|kind| self.check(*kind)) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn consume(&mut self, kind: TokenKind, message: &str) -> Result<Token> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            Err(self.error(self.current(), message))
        }
    }

    fn check(&self, kind: TokenKind) -> bool {
        self.current().kind == kind
    }

    fn advance(&mut self) -> Token {
        let token = self.current().clone();
        if token.kind != TokenKind::Eof {
            self.index += 1;
        }
        token
    }

    fn current(&self) -> &Token {
        &self.tokens[self.index]
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.index.saturating_sub(1)]
    }

    fn error(&self, token: &Token, message: impl Into<String>) -> NiloError {
        NiloError::parse(message).at(&self.filename, token.span, Some(self.source))
    }
}

#[cfg(test)]
mod tests {
    use crate::lexer::Lexer;

    use super::Parser;

    fn parse(source: &str) -> crate::error::Result<crate::ast::Program> {
        let tokens = Lexer::new(source, "<test>").tokenize()?;
        Parser::new(tokens, "<test>", source).parse()
    }

    #[test]
    fn parses_types_control_flow_and_assignment_targets() {
        let program = parse(
            r#"
            type User { name: str; tags: list<str>; }
            let user: User? = nil;
            let items: list<int> = [1, 2, 3];
            items[1] = 20;
            if (user == nil) { items[0] = 5; } else if (true) { items[0] = 6; }
            "#,
        )
        .expect("source should parse");
        assert_eq!(program.statements.len(), 5);
    }

    #[test]
    fn rejects_break_outside_loop() {
        let error = parse("break;").expect_err("break should fail");
        assert!(error.render().contains("inside a loop"));
    }
}
