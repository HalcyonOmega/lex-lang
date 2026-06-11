//! AST nodes. Grows with each milestone; keep nodes small and keep spans on
//! anything an error might need to point at.

use crate::diag::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessConvention {
    /// Default: shared read borrow (`&T` in Rust).
    Read,
    /// Mutable borrow (`&mut T`).
    Mutate,
    /// Ownership transfer (`T` by value).
    Move,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Int,
    Float,
    Bool,
    String,
    List(Box<Type>),
    Shared(Box<Type>),
    Named(String),
}

#[derive(Debug)]
pub struct Program {
    pub items: Vec<Item>,
}

#[derive(Debug)]
pub enum Item {
    Func(Func),
    Struct(StructDef),
    Const(ConstDef),
}

#[derive(Debug)]
pub struct Func {
    pub is_pub: bool,
    pub name: String,
    pub name_span: Span,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub is_view_return: bool,
    pub body: Vec<Stmt>,
}

#[derive(Debug)]
pub struct Param {
    pub convention: AccessConvention,
    pub name: String,
    pub name_span: Span,
    pub ty: Type,
    pub ty_span: Span,
}

#[derive(Debug)]
pub struct StructDef {
    pub is_pub: bool,
    pub name: String,
    pub name_span: Span,
    pub fields: Vec<Field>,
}

#[derive(Debug)]
pub struct Field {
    pub is_stored_ref: bool,
    pub stored_ref_label: Option<String>,
    pub name: String,
    pub name_span: Span,
    pub ty: Type,
    pub ty_span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstAttr {
    ForceStatic,
    ForceInline,
}

#[derive(Debug)]
pub struct ConstDef {
    pub name: String,
    pub name_span: Span,
    pub value: Expr,
    pub attrs: Vec<ConstAttr>,
    pub rust_kind: RustConstKind,
}

#[derive(Debug)]
pub enum Stmt {
    Call(Call),
    Val(Binding),
    Return(Expr, Span),
    Loop(Vec<Stmt>, Span),
    Unsafe(Vec<Stmt>, Span),
}

#[derive(Debug)]
pub struct Binding {
    pub mutable: bool,
    pub name: String,
    pub name_span: Span,
    pub ty: Option<Type>,
    pub init: Expr,
}

#[derive(Debug)]
pub struct Call {
    pub name: String,
    pub name_span: Span,
    pub args: Vec<CallArg>,
}

#[derive(Debug, Default)]
pub struct CallArgFlags {
    pub implicit_clone: bool,
    pub shared_auto_clone: bool,
}

#[derive(Debug)]
pub struct CallArg {
    pub convention: AccessConvention,
    pub expr: Expr,
    pub span: Span,
    pub flags: CallArgFlags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustConstKind {
    Const,
    Static,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Str(String),
    Int(i64),
    Ident(String, Span),
    Deref(Box<Expr>, Span),
    Member(Box<Expr>, String, Span),
}
