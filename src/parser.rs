//! Parser: tokens -> AST. Hand-written recursive descent.

use crate::ast::{
    AccessConvention, Binding, Call, CallArg, ConstAttr, ConstDef, Expr, Field, Func, Item,
    Param, Program, Stmt, StructDef, Type,
};
use crate::diag::Diagnostic;
use crate::lexer::{describe, TokKind, Token};
use crate::syntax;

pub fn parse(toks: &[Token]) -> Result<Program, Diagnostic> {
    let mut p = Parser { toks, pos: 0 };
    p.program()
}

struct Parser<'a> {
    toks: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> &Token {
        &self.toks[self.pos.min(self.toks.len() - 1)]
    }

    fn bump(&mut self) -> Token {
        let t = self.peek().clone();
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn program(&mut self) -> Result<Program, Diagnostic> {
        let mut items = Vec::new();
        loop {
            match &self.peek().kind {
                TokKind::Eof => break,
                TokKind::KwFn | TokKind::KwPub => items.push(Item::Func(self.func()?)),
                TokKind::KwStruct => items.push(Item::Struct(self.struct_def(false)?)),
                TokKind::KwConst | TokKind::At => items.push(Item::Const(self.const_def()?)),
                other => {
                    return Err(Diagnostic::error(
                        "E0003",
                        format!(
                            "expected `{}`, `{}`, or `{}` here, found {}",
                            syntax::KW_FN,
                            syntax::KW_STRUCT,
                            syntax::KW_CONST,
                            describe(other)
                        ),
                        "at the top level of a file, only definitions can appear".to_string(),
                        format!(
                            "define a function ({} main() {{ ... }}), struct, or const",
                            syntax::KW_FN
                        ),
                        Some(self.peek().span),
                    ));
                }
            }
        }
        Ok(Program { items })
    }

    fn func(&mut self) -> Result<Func, Diagnostic> {
        let is_pub = matches!(self.peek().kind, TokKind::KwPub);
        if is_pub {
            self.bump();
        }
        self.expect_kw(TokKind::KwFn, "to start a function definition")?;
        let (name, name_span) = self.expect_ident("after `fn`")?;
        self.expect(TokKind::LParen, "after the function name")?;
        let mut params = Vec::new();
        if !matches!(self.peek().kind, TokKind::RParen) {
            loop {
                params.push(self.param()?);
                if matches!(self.peek().kind, TokKind::RParen) {
                    break;
                }
                self.expect(TokKind::Comma, "between parameters")?;
            }
        }
        self.expect(TokKind::RParen, "to close the parameter list")?;

        let mut return_type = None;
        let mut is_view_return = false;
        if matches!(self.peek().kind, TokKind::Arrow) {
            self.bump();
            if matches!(self.peek().kind, TokKind::KwView) {
                is_view_return = true;
                self.bump();
            }
            let (ty, _) = self.type_()?;
            return_type = Some(ty);
        }

        self.expect(TokKind::LBrace, "to open the function body")?;
        let body = self.block_stmts()?;
        Ok(Func {
            is_pub,
            name,
            name_span,
            params,
            return_type,
            is_view_return,
            body,
        })
    }

    fn param(&mut self) -> Result<Param, Diagnostic> {
        let convention = self.parse_access_prefix()?;
        let (name, name_span) = self.expect_ident("for a parameter name")?;
        self.expect(TokKind::Colon, "after a parameter name")?;
        let (ty, ty_span) = self.type_()?;
        Ok(Param {
            convention,
            name,
            name_span,
            ty,
            ty_span,
        })
    }

    fn struct_def(&mut self, nested: bool) -> Result<StructDef, Diagnostic> {
        let is_pub = if nested {
            false
        } else {
            matches!(self.peek().kind, TokKind::KwPub)
        };
        if is_pub {
            self.bump();
        }
        self.expect_kw(TokKind::KwStruct, "to start a struct definition")?;
        let (name, name_span) = self.expect_ident("after `struct`")?;
        self.expect(TokKind::LBrace, "to open the struct body")?;
        let mut fields = Vec::new();
        while !matches!(self.peek().kind, TokKind::RBrace) {
            fields.push(self.field()?);
            if matches!(self.peek().kind, TokKind::Comma | TokKind::Semi) {
                self.bump();
            }
        }
        self.bump(); // }
        Ok(StructDef {
            is_pub,
            name,
            name_span,
            fields,
        })
    }

    fn field(&mut self) -> Result<Field, Diagnostic> {
        let mut is_stored_ref = false;
        let mut stored_ref_label = None;
        if matches!(self.peek().kind, TokKind::KwStored) {
            is_stored_ref = true;
            self.bump();
            if matches!(self.peek().kind, TokKind::LBracket) {
                self.bump();
                let (label, _) = self.expect_ident("inside `ref[...]`")?;
                stored_ref_label = Some(label);
                self.expect(TokKind::RBracket, "after a ref label")?;
            }
        }
        let (name, name_span) = self.expect_ident("for a field name")?;
        self.expect(TokKind::Colon, "after a field name")?;
        let (ty, ty_span) = self.type_()?;
        Ok(Field {
            is_stored_ref,
            stored_ref_label,
            name,
            name_span,
            ty,
            ty_span,
        })
    }

    fn const_def(&mut self) -> Result<ConstDef, Diagnostic> {
        let mut attrs = Vec::new();
        while matches!(self.peek().kind, TokKind::At) {
            self.bump();
            let (attr_name, _) = self.expect_ident("after `@`")?;
            match attr_name.as_str() {
                "static" => attrs.push(ConstAttr::ForceStatic),
                "inline" => attrs.push(ConstAttr::ForceInline),
                other => {
                    return Err(Diagnostic::error(
                        "E0003",
                        format!("`@{}` isn't a known attribute on a const", other),
                        "only `@static` and `@inline` are supported on const declarations"
                            .to_string(),
                        "remove the attribute or use `@static` or `@inline`".to_string(),
                        Some(self.peek().span),
                    ));
                }
            }
        }
        self.expect_kw(TokKind::KwConst, "to start a const declaration")?;
        let (name, name_span) = self.expect_ident("after `const`")?;
        self.expect(TokKind::Eq, "after the const name")?;
        let value = self.expr()?;
        self.expect(TokKind::Semi, "after a const value")?;
        Ok(ConstDef {
            name,
            name_span,
            value,
            attrs,
            rust_kind: crate::ast::RustConstKind::Const,
        })
    }

    fn block_stmts(&mut self) -> Result<Vec<Stmt>, Diagnostic> {
        let mut body = Vec::new();
        loop {
            match &self.peek().kind {
                TokKind::RBrace => {
                    self.bump();
                    break;
                }
                TokKind::Eof => {
                    return Err(Diagnostic::error(
                        "E0003",
                        "expected `}` to close this block, found the end of the file".to_string(),
                        "every `{` needs a matching `}`".to_string(),
                        "add a closing `}`".to_string(),
                        Some(self.peek().span),
                    ));
                }
                _ => body.push(self.stmt()?),
            }
        }
        Ok(body)
    }

    fn stmt(&mut self) -> Result<Stmt, Diagnostic> {
        match &self.peek().kind {
            TokKind::KwVal | TokKind::KwVar | TokKind::KwMutate => {
                let binding = self.binding()?;
                self.finish_stmt()?;
                Ok(Stmt::Val(binding))
            }
            TokKind::KwReturn => {
                let span = self.bump().span;
                let expr = self.expr()?;
                self.finish_stmt()?;
                Ok(Stmt::Return(expr, span))
            }
            TokKind::KwLoop => {
                let span = self.bump().span;
                self.expect(TokKind::LBrace, "after `loop`")?;
                let inner = self.block_stmts()?;
                Ok(Stmt::Loop(inner, span))
            }
            TokKind::KwUnsafe => {
                let span = self.bump().span;
                self.expect(TokKind::LBrace, "after `unsafe`")?;
                let inner = self.block_stmts()?;
                Ok(Stmt::Unsafe(inner, span))
            }
            TokKind::Ident(_) => {
                let call = self.call()?;
                self.finish_stmt()?;
                Ok(Stmt::Call(call))
            }
            other => Err(Diagnostic::error(
                "E0003",
                format!("expected a statement, found {}", describe(other)),
                "inside a function body, write a call, binding, or `return`".to_string(),
                format!(
                    "e.g. {}(\"hello\"); or {} x = 1;",
                    syntax::BUILTIN_PRINT,
                    syntax::KW_VAL
                ),
                Some(self.peek().span),
            )),
        }
    }

    fn binding(&mut self) -> Result<Binding, Diagnostic> {
        let mutable = match self.peek().kind {
            TokKind::KwMutate => {
                self.bump();
                true
            }
            TokKind::KwVar => {
                self.bump();
                true
            }
            TokKind::KwVal => {
                self.bump();
                false
            }
            _ => unreachable!(),
        };
        let (name, name_span) = self.expect_ident("after a binding keyword")?;
        let ty = if matches!(self.peek().kind, TokKind::Colon) {
            self.bump();
            Some(self.type_()?.0)
        } else {
            None
        };
        self.expect(TokKind::Eq, "in a binding")?;
        let init = self.expr()?;
        Ok(Binding {
            mutable,
            name,
            name_span,
            ty,
            init,
        })
    }

    fn call(&mut self) -> Result<Call, Diagnostic> {
        let (name, name_span) = self.expect_ident("to call a function")?;
        self.expect(TokKind::LParen, &format!("after `{}` to call it", name))?;
        let mut args = Vec::new();
        if !matches!(self.peek().kind, TokKind::RParen) {
            loop {
                args.push(self.call_arg()?);
                if matches!(self.peek().kind, TokKind::RParen) {
                    break;
                }
                self.expect(TokKind::Comma, "between arguments")?;
            }
        }
        self.expect(TokKind::RParen, "to finish the call")?;
        Ok(Call {
            name,
            name_span,
            args,
        })
    }

    fn call_arg(&mut self) -> Result<CallArg, Diagnostic> {
        let convention = self.parse_access_prefix()?;
        let span = self.peek().span;
        let expr = self.expr()?;
        Ok(CallArg {
            convention,
            expr,
            span,
            flags: Default::default(),
        })
    }

    fn parse_access_prefix(&mut self) -> Result<AccessConvention, Diagnostic> {
        match self.peek().kind {
            TokKind::KwMutate => {
                self.bump();
                Ok(AccessConvention::Mutate)
            }
            TokKind::KwMove => {
                self.bump();
                Ok(AccessConvention::Move)
            }
            _ => Ok(AccessConvention::Read),
        }
    }

    fn expr(&mut self) -> Result<Expr, Diagnostic> {
        if matches!(self.peek().kind, TokKind::Star) {
            let span = self.bump().span;
            let inner = self.expr()?;
            return Ok(Expr::Deref(Box::new(inner), span));
        }
        let mut expr = self.expr_primary()?;
        loop {
            if matches!(self.peek().kind, TokKind::Dot) {
                self.bump();
                let (member, member_span) = self.expect_ident("after `.`")?;
                let span = member_span;
                expr = Expr::Member(Box::new(expr), member, span);
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn expr_primary(&mut self) -> Result<Expr, Diagnostic> {
        match self.peek().kind.clone() {
            TokKind::Str(s) => {
                self.bump();
                Ok(Expr::Str(s))
            }
            TokKind::Int(n) => {
                self.bump();
                Ok(Expr::Int(n))
            }
            TokKind::Ident(name) => {
                let span = self.bump().span;
                Ok(Expr::Ident(name, span))
            }
            other => Err(Diagnostic::error(
                "E0003",
                format!("expected a value, found {}", describe(&other)),
                "a value can be a name, a number, or quoted text".to_string(),
                "e.g. `x`, `42`, or `\"hello\"`".to_string(),
                Some(self.peek().span),
            )),
        }
    }

    fn type_(&mut self) -> Result<(Type, crate::diag::Span), Diagnostic> {
        let start = self.peek().span;
        let base = match self.peek().kind.clone() {
            TokKind::Ident(name) => {
                self.bump();
                match name.as_str() {
                    syntax::TYPE_INT => Type::Int,
                    syntax::TYPE_FLOAT => Type::Float,
                    syntax::TYPE_BOOL => Type::Bool,
                    syntax::TYPE_STRING => Type::String,
                    syntax::TYPE_LIST => {
                        self.expect(TokKind::LBracket, "after `List`")?;
                        let (inner, _) = self.type_()?;
                        self.expect(TokKind::RBracket, "after a list element type")?;
                        Type::List(Box::new(inner))
                    }
                    syntax::TYPE_SHARED => {
                        self.expect(TokKind::LBracket, "after `Shared`")?;
                        let (inner, _) = self.type_()?;
                        self.expect(TokKind::RBracket, "after a shared element type")?;
                        Type::Shared(Box::new(inner))
                    }
                    other => Type::Named(other.to_string()),
                }
            }
            other => {
                return Err(Diagnostic::error(
                    "E0003",
                    format!("expected a type name, found {}", describe(&other)),
                    "types look like `Int`, `String`, or `List[Int]`".to_string(),
                    "e.g. `x: Int` or `items: List[String]`".to_string(),
                    Some(self.peek().span),
                ));
            }
        };
        Ok((base, start))
    }

    fn finish_stmt(&mut self) -> Result<(), Diagnostic> {
        match &self.peek().kind {
            TokKind::Semi => {
                self.bump();
                Ok(())
            }
            TokKind::RBrace => Ok(()),
            other => Err(Diagnostic::error(
                "E0003",
                format!(
                    "expected `{}` after this statement, found {}",
                    syntax::STMT_SEP,
                    describe(other)
                ),
                format!(
                    "statements inside a function body end with `{}`",
                    syntax::STMT_SEP
                ),
                format!("add `{}` after the statement", syntax::STMT_SEP),
                Some(self.peek().span),
            )),
        }
    }

    fn expect_kw(&mut self, want: TokKind, where_: &str) -> Result<(), Diagnostic> {
        if std::mem::discriminant(&self.peek().kind) == std::mem::discriminant(&want) {
            self.bump();
            Ok(())
        } else {
            Err(Diagnostic::error(
                "E0003",
                format!(
                    "expected {} {}, found {}",
                    describe(&want),
                    where_,
                    describe(&self.peek().kind)
                ),
                "the structure here isn't what the compiler expected".to_string(),
                format!("use `{}` {}", describe(&want), where_),
                Some(self.peek().span),
            ))
        }
    }

    fn expect(&mut self, want: TokKind, where_: &str) -> Result<(), Diagnostic> {
        self.expect_kw(want, where_)
    }

    fn expect_ident(&mut self, where_: &str) -> Result<(String, crate::diag::Span), Diagnostic> {
        match self.bump() {
            Token {
                kind: TokKind::Ident(name),
                span,
            } => Ok((name, span)),
            t => Err(Diagnostic::error(
                "E0003",
                format!("expected a name {}, found {}", where_, describe(&t.kind)),
                "names start with a letter or `_`".to_string(),
                "e.g. `main`, `count`, `_tmp`".to_string(),
                Some(t.span),
            )),
        }
    }
}
