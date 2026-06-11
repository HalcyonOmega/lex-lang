//! Semantic checks. Everything here exists so that codegen can stay "dumb"
//! (invariant I3): by the time a Program reaches codegen, it must be
//! impossible for the generated Rust to fail to compile (invariant I2).
//!
//! M1: type inference, mutability, comparison distribution (S25),
//! definite-return analysis. M2: ownership — moves, call-site `mut`/`take`,
//! view returns, use-after-move, and borrow rules that keep generated Rust
//! sound without surfacing Rust concepts to users.

use crate::ast::{
    AccessConvention, BinOp, Binding, Call, ConstAttr, ElseBranch, Expr, IfStmt, Item, Program,
    RustConstKind, Stmt, StrPart, Type, UnOp,
};
use crate::diag::{Diagnostic, Span};
use crate::syntax;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct FuncSig {
    pub params: Vec<(AccessConvention, Type)>,
    pub return_type: Option<Type>,
    #[allow(dead_code)]
    pub is_view_return: bool,
}

#[derive(Debug, Clone)]
struct LocalInfo {
    ty: Type,
    mutable: bool,
    /// Set when the name is a parameter (with its access convention).
    param_conv: Option<AccessConvention>,
    /// Loop nesting depth where the name was declared (for move-in-loop).
    decl_loop_depth: usize,
}

pub fn check(prog: &mut Program) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let mut funcs: HashMap<String, FuncSig> = HashMap::new();
    let mut structs: HashMap<String, Vec<(Option<String>, Type)>> = HashMap::new();
    let mut consts: HashMap<String, Type> = HashMap::new();

    // --- collect signatures ---------------------------------------------
    for item in &prog.items {
        match item {
            Item::Func(f) => {
                if f.name == syntax::BUILTIN_PRINT {
                    diags.push(Diagnostic::error(
                        "E0106",
                        format!(
                            "the name `{}` is built in and can't be redefined",
                            syntax::BUILTIN_PRINT
                        ),
                        format!("`{}` is provided by the language itself", syntax::BUILTIN_PRINT),
                        "choose a different name for this function".to_string(),
                        Some(f.name_span),
                    ));
                } else if funcs.contains_key(&f.name) {
                    diags.push(Diagnostic::error(
                        "E0105",
                        format!("`{}` is defined twice", f.name),
                        "every function needs a unique name so calls aren't ambiguous".to_string(),
                        "rename or remove one of the definitions".to_string(),
                        Some(f.name_span),
                    ));
                } else {
                    funcs.insert(
                        f.name.clone(),
                        FuncSig {
                            params: f
                                .params
                                .iter()
                                .map(|p| (p.convention, p.ty.clone()))
                                .collect(),
                            return_type: f.return_type.clone(),
                            is_view_return: f.is_view_return,
                        },
                    );
                }
            }
            Item::Struct(s) => {
                if structs.contains_key(&s.name) {
                    diags.push(Diagnostic::error(
                        "E0105",
                        format!("`{}` is defined twice", s.name),
                        "every struct needs a unique name".to_string(),
                        "rename or remove one of the definitions".to_string(),
                        Some(s.name_span),
                    ));
                } else {
                    structs.insert(
                        s.name.clone(),
                        s.fields
                            .iter()
                            .map(|f| (f.stored_ref_label.clone(), f.ty.clone()))
                            .collect(),
                    );
                }
                let ref_fields: Vec<_> = s.fields.iter().filter(|f| f.is_stored_ref).collect();
                if ref_fields.len() >= 2 {
                    let unlabeled = ref_fields
                        .iter()
                        .filter(|f| f.stored_ref_label.is_none())
                        .count();
                    if unlabeled >= 2 {
                        diags.push(Diagnostic::error(
                            "E0207",
                            "this struct has more than one stored reference without a label"
                                .to_string(),
                            "when two `ref` fields may come from different places, each needs a label like `ref[src]`".to_string(),
                            "add labels: `ref[a] x: String` and `ref[b] y: String`".to_string(),
                            Some(s.name_span),
                        ));
                    }
                }
            }
            Item::Const(c) => {
                if consts.contains_key(&c.name) {
                    diags.push(Diagnostic::error(
                        "E0105",
                        format!("`{}` is defined twice", c.name),
                        "every const needs a unique name".to_string(),
                        "rename or remove one of the definitions".to_string(),
                        Some(c.name_span),
                    ));
                } else {
                    let ty = match &c.value {
                        Expr::Int(_, _) => Some(Type::Int),
                        Expr::Float(_, _) => Some(Type::Float),
                        Expr::Bool(_, _) => Some(Type::Bool),
                        _ => None,
                    };
                    match ty {
                        Some(t) => {
                            consts.insert(c.name.clone(), t);
                        }
                        None => {
                            diags.push(Diagnostic::error(
                                "E0109",
                                "a const holds a plain number or `true`/`false` for now"
                                    .to_string(),
                                "richer const values arrive with later milestones".to_string(),
                                "give the const a number, like `const LIMIT = 10;`".to_string(),
                                Some(c.value.span()),
                            ));
                        }
                    }
                }
            }
        }
    }

    match funcs.get("main") {
        None => {
            diags.push(Diagnostic::error(
                "E0101",
                "this program has no `main` function".to_string(),
                "running a program starts at `fn main`, and this file doesn't define one"
                    .to_string(),
                "add one to this file: fn main() { ... }".to_string(),
                None,
            ));
        }
        Some(sig) => {
            if !sig.params.is_empty() || sig.return_type.is_some() {
                let span = prog.items.iter().find_map(|i| match i {
                    Item::Func(f) if f.name == "main" => Some(f.name_span),
                    _ => None,
                });
                diags.push(Diagnostic::error(
                    "E0122",
                    "`main` takes no parameters and returns nothing".to_string(),
                    "`main` is where running starts; nothing calls it with values".to_string(),
                    "write it as: fn main() { ... }".to_string(),
                    span,
                ));
            }
        }
    }

    // Const address-taken analysis (rule 9, M2 scaffolding).
    let const_names: Vec<String> = consts.keys().cloned().collect();
    let mut address_taken: HashSet<String> = HashSet::new();
    for item in &prog.items {
        if let Item::Func(f) = item {
            walk_stmts_for_const_refs(&f.body, &const_names, &mut address_taken);
        }
    }
    for item in &mut prog.items {
        if let Item::Const(c) = item {
            let force_static = c.attrs.contains(&ConstAttr::ForceStatic);
            c.rust_kind = if force_static || address_taken.contains(&c.name) {
                RustConstKind::Static
            } else {
                RustConstKind::Const
            };
        }
    }

    // --- per-function checks ----------------------------------------------
    for item in &mut prog.items {
        if let Item::Func(f) = item {
            let mut ck = Checker {
                funcs: &funcs,
                structs: &structs,
                consts: &consts,
                diags: Vec::new(),
                scopes: vec![HashMap::new()],
                moved: HashMap::new(),
                loop_depth: 0,
                in_unsafe: false,
                ret: f.return_type.clone(),
                view_return: f.is_view_return,
                fn_name: f.name.clone(),
            };
            for p in &f.params {
                ck.check_declared_type(&p.ty, p.ty_span);
                if ck.lookup(&p.name).is_some() {
                    ck.diags.push(already_defined(&p.name, p.name_span));
                } else {
                    ck.scopes.last_mut().unwrap().insert(
                        p.name.clone(),
                        LocalInfo {
                            ty: p.ty.clone(),
                            mutable: matches!(p.convention, AccessConvention::Mutate),
                            param_conv: Some(p.convention),
                            decl_loop_depth: 0,
                        },
                    );
                }
            }
            ck.check_block(&mut f.body, false);

            // Definite return (E0114): a promised value must come back on
            // every path, or rustc would reject the generated code (I2).
            if f.return_type.is_some() && !block_definitely_returns(&f.body) {
                let rt = f.return_type.clone().unwrap();
                ck.diags.push(Diagnostic::error(
                    "E0114",
                    format!(
                        "`{}` promises to return {}, but a path can reach the end without `return`",
                        f.name,
                        rt.show()
                    ),
                    "every way through the function must hand back a value".to_string(),
                    format!("add a final `return ...;`, or an `{}` branch that returns", syntax::KW_ELSE),
                    Some(f.name_span),
                ));
            }
            diags.extend(ck.diags);
        }
    }

    diags
}

fn already_defined(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "E0118",
        format!("the name `{}` is already taken here", name),
        "inside one function, each name refers to exactly one thing".to_string(),
        format!(
            "pick a different name, or assign to the existing one with `{} = ...`",
            name
        ),
        Some(span),
    )
}

struct Checker<'a> {
    funcs: &'a HashMap<String, FuncSig>,
    structs: &'a HashMap<String, Vec<(Option<String>, Type)>>,
    consts: &'a HashMap<String, Type>,
    diags: Vec<Diagnostic>,
    scopes: Vec<HashMap<String, LocalInfo>>,
    /// name -> span of the use that gave the value away.
    moved: HashMap<String, Span>,
    loop_depth: usize,
    in_unsafe: bool,
    ret: Option<Type>,
    /// `-> view T` on this function (borrowed return).
    view_return: bool,
    fn_name: String,
}

impl<'a> Checker<'a> {
    fn lookup(&self, name: &str) -> Option<&LocalInfo> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }

    fn declare(&mut self, name: &str, name_span: Span, info: LocalInfo) {
        if self.lookup(name).is_some() || self.consts.contains_key(name) {
            self.diags.push(already_defined(name, name_span));
        }
        self.moved.remove(name);
        self.scopes
            .last_mut()
            .unwrap()
            .insert(name.to_string(), info);
    }

    fn check_declared_type(&mut self, ty: &Type, span: Span) {
        match ty {
            Type::Named(n) if !self.structs.contains_key(n) => {
                self.diags.push(Diagnostic::error(
                    "E0119",
                    format!("there's no type called `{}`", n),
                    format!(
                        "the types are `{}`, `{}`, `{}`, and `{}` (plus structs you define)",
                        syntax::TYPE_INT,
                        syntax::TYPE_FLOAT,
                        syntax::TYPE_BOOL,
                        syntax::TYPE_STRING
                    ),
                    "check the spelling, or define the struct first".to_string(),
                    Some(span),
                ));
            }
            Type::List(inner) | Type::Shared(inner) => self.check_declared_type(inner, span),
            _ => {}
        }
    }

    // --- statements -----------------------------------------------------

    fn check_block(&mut self, stmts: &mut [Stmt], new_scope: bool) {
        if new_scope {
            self.scopes.push(HashMap::new());
        }
        for stmt in stmts.iter_mut() {
            self.check_stmt(stmt);
        }
        if new_scope {
            self.scopes.pop();
        }
    }

    /// Check two alternative branches with independent move states, then
    /// keep the union (a value moved in either branch counts as gone).
    fn check_branches(&mut self, branches: &mut [&mut Vec<Stmt>]) {
        let before = self.moved.clone();
        let mut after = self.moved.clone();
        for body in branches.iter_mut() {
            self.moved = before.clone();
            self.check_block(body, true);
            for (k, v) in self.moved.drain() {
                after.entry(k).or_insert(v);
            }
        }
        self.moved = after;
    }

    fn check_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::Val(b) => self.check_binding(b),
            Stmt::Assign {
                name,
                name_span,
                op,
                op_span,
                value,
            } => {
                let vt = self.infer(value);
                self.note_move_if_direct_ident(value);
                let Some(info) = self.lookup(name).cloned() else {
                    if self.consts.contains_key(name.as_str()) {
                        self.diags.push(Diagnostic::error(
                            "E0111",
                            format!("`{}` is a const and can never change", name),
                            "a const is fixed for the whole program".to_string(),
                            format!("use a `{}` binding if it needs to change", syntax::KW_VAR),
                            Some(*name_span),
                        ));
                    } else {
                        self.unknown_name(name, *name_span);
                    }
                    return;
                };
                let assignable = info.mutable;
                if !assignable {
                    let what = if info.param_conv.is_some() {
                        format!("the parameter `{}` can't be changed here", name)
                    } else {
                        format!("`{}` was made with `{}`, so it can't change", name, syntax::KW_VAL)
                    };
                    let fix = if info.param_conv.is_some() {
                        format!(
                            "mark the parameter `{} {}: {}` if the function should change it",
                            syntax::KW_MUTATE,
                            name,
                            info.ty.name()
                        )
                    } else {
                        format!("declare it with `{} {} = ...` instead", syntax::KW_VAR, name)
                    };
                    self.diags.push(Diagnostic::error(
                        "E0111",
                        what,
                        format!(
                            "only `{}` bindings (and `{}` parameters) can be changed",
                            syntax::KW_VAR,
                            syntax::KW_MUTATE
                        ),
                        fix,
                        Some(*name_span),
                    ));
                }
                self.moved.remove(name);
                if let Some(op) = op {
                    // S17: compound assignment.
                    let ok = match op {
                        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
                            matches!(info.ty, Type::Int | Type::Float)
                        }
                        _ => matches!(info.ty, Type::Int),
                    };
                    if !ok {
                        self.diags.push(Diagnostic::error(
                            "E0109",
                            format!(
                                "`{}{}` doesn't work on {}",
                                op.spell(),
                                "=",
                                info.ty.show()
                            ),
                            compound_why(*op),
                            "use a value of the right type, or a different operation".to_string(),
                            Some(*op_span),
                        ));
                        return;
                    }
                }
                if let (Some(vt), false) = (vt.clone(), info.ty == Type::Named(String::new())) {
                    if vt != info.ty {
                        self.diags.push(Diagnostic::error(
                            "E0108",
                            format!(
                                "`{}` holds {}, but this value is {}",
                                name,
                                info.ty.show(),
                                vt.show()
                            ),
                            "a binding keeps one type for its whole life".to_string(),
                            type_fix_hint(&info.ty, &vt),
                            Some(value.span()),
                        ));
                    }
                }
            }
            Stmt::Expr(expr) => {
                if let Expr::Call(call) = expr {
                    self.check_call(call, false);
                } else {
                    // Member-call statement, e.g. `x.clone();` — checked as
                    // an expression; the value is dropped.
                    self.infer(expr);
                }
            }
            Stmt::Return(expr, span) => {
                match (&mut *expr, self.ret.clone()) {
                    (Some(e), Some(rt)) => {
                        let et = self.infer(e);
                        // Returning a borrowed parameter would move out of a
                        // borrow in the generated Rust (I2) — require a copy.
                        if let Expr::Ident(n, nspan) = &*e {
                            if let Some(info) = self.lookup(n) {
                                if !self.view_return
                                    && !info.ty.is_scalar()
                                    && matches!(
                                        info.param_conv,
                                        Some(AccessConvention::Read)
                                            | Some(AccessConvention::Mutate)
                                    )
                                {
                                    self.diags.push(Diagnostic::error(
                                        "E0120",
                                        format!(
                                            "`{}` is only borrowed here, so it can't be given back as-is",
                                            n
                                        ),
                                        "this function reads the value but doesn't own it"
                                            .to_string(),
                                        format!(
                                            "return a copy: `return {}.clone();` — or take ownership with `{} {}: {}`",
                                            n,
                                            syntax::KW_MOVE,
                                            n,
                                            info.ty.name()
                                        ),
                                        Some(*nspan),
                                    ));
                                }
                            }
                        }
                        if self.view_return && !self.expr_ok_for_view_return(e) {
                            self.diags.push(Diagnostic::error(
                                "E0206",
                                "this value can't be handed back as a shared view".to_string(),
                                "a `view` return may only point at a parameter, a whole-number or yes/no name, or a const — not at fresh text you just made here".to_string(),
                                "return a parameter or const, copy with `.clone()` into an owned return type, or change `-> view` to `->`".to_string(),
                                Some(e.span()),
                            ));
                        }
                        self.note_move_if_direct_ident(e);
                        if let Some(et) = et {
                            if et != rt {
                                self.diags.push(Diagnostic::error(
                                    "E0113",
                                    format!(
                                        "`{}` promises to return {}, but this returns {}",
                                        self.fn_name,
                                        rt.show(),
                                        et.show()
                                    ),
                                    "the value handed back must match the type after `->`"
                                        .to_string(),
                                    type_fix_hint(&rt, &et),
                                    Some(e.span()),
                                ));
                            }
                        }
                    }
                    (Some(e), None) => {
                        let ty_name = self.infer_name_or(e, "Int");
                        self.diags.push(Diagnostic::error(
                            "E0113",
                            format!("`{}` doesn't return a value", self.fn_name),
                            "a function only hands back a value if it declares one with `-> Type`"
                                .to_string(),
                            format!(
                                "remove the value (`return;`), or declare `-> {}` on the function",
                                ty_name
                            ),
                            Some(e.span()),
                        ));
                    }
                    (None, Some(rt)) => {
                        self.diags.push(Diagnostic::error(
                            "E0113",
                            format!(
                                "`{}` promises to return {}, but this `return` is empty",
                                self.fn_name,
                                rt.show()
                            ),
                            "the value handed back must match the type after `->`".to_string(),
                            "add the value: `return ...;`".to_string(),
                            Some(*span),
                        ));
                    }
                    (None, None) => {}
                }
            }
            Stmt::If(ifs) => self.check_if(ifs),
            Stmt::While { cond, body, span: _ } => {
                self.require_bool(cond, "a `while` condition");
                self.loop_depth += 1;
                self.check_block(body, true);
                self.loop_depth -= 1;
            }
            Stmt::For {
                var,
                var_span,
                start,
                end,
                body,
                span: _,
            } => {
                for (e, which) in [(&mut *start, "start"), (&mut *end, "end")] {
                    let t = self.infer(e);
                    if let Some(t) = t {
                        if t != Type::Int {
                            self.diags.push(Diagnostic::error(
                                "E0109",
                                format!(
                                    "the {} of a `for` range must be {}, not {}",
                                    which,
                                    Type::Int.show(),
                                    t.show()
                                ),
                                "`for` counts whole numbers between two ends (both included, S22)"
                                    .to_string(),
                                "use Int values for both ends, like `1..10`".to_string(),
                                Some(e.span()),
                            ));
                        }
                    }
                }
                self.loop_depth += 1;
                self.scopes.push(HashMap::new());
                let vs = *var_span;
                let v = var.clone();
                if self.lookup(&v).is_some() || self.consts.contains_key(&v) {
                    self.diags.push(already_defined(&v, vs));
                }
                self.scopes.last_mut().unwrap().insert(
                    v,
                    LocalInfo {
                        ty: Type::Int,
                        mutable: false,
                        param_conv: None,
                        decl_loop_depth: self.loop_depth,
                    },
                );
                for s in body.iter_mut() {
                    self.check_stmt(s);
                }
                self.scopes.pop();
                self.loop_depth -= 1;
            }
            Stmt::Switch {
                subject,
                arms,
                else_body,
                span: _,
            } => {
                self.infer(subject);
                for arm in arms.iter_mut() {
                    self.require_bool(&mut arm.cond, "a `switch` arm's condition");
                }
                let mut branches: Vec<&mut Vec<Stmt>> = Vec::new();
                for arm in arms.iter_mut() {
                    branches.push(&mut arm.body);
                }
                branches.push(else_body);
                self.check_branches(&mut branches);
            }
            Stmt::Break(span) => {
                if self.loop_depth == 0 {
                    self.diags.push(loop_control_outside(syntax::KW_BREAK, *span));
                }
            }
            Stmt::Continue(span) => {
                if self.loop_depth == 0 {
                    self.diags
                        .push(loop_control_outside(syntax::KW_CONTINUE, *span));
                }
            }
            Stmt::Loop(inner, _) => {
                self.loop_depth += 1;
                self.check_block(inner, true);
                self.loop_depth -= 1;
            }
            Stmt::Unsafe(inner, _) => {
                let prev = self.in_unsafe;
                self.in_unsafe = true;
                self.check_block(inner, true);
                self.in_unsafe = prev;
            }
        }
    }

    fn check_if(&mut self, ifs: &mut IfStmt) {
        self.require_bool(&mut ifs.cond, "an `if` condition");
        match &mut ifs.else_branch {
            None => {
                let before = self.moved.clone();
                let mut after = before.clone();
                self.check_block(&mut ifs.then_body, true);
                for (k, v) in self.moved.drain() {
                    after.entry(k).or_insert(v);
                }
                self.moved = after;
            }
            Some(ElseBranch::Else(else_body)) => {
                let before = self.moved.clone();
                let mut after = before.clone();
                self.check_block(&mut ifs.then_body, true);
                for (k, v) in self.moved.drain() {
                    after.entry(k).or_insert(v);
                }
                self.moved = before.clone();
                self.check_block(else_body, true);
                for (k, v) in self.moved.drain() {
                    after.entry(k).or_insert(v);
                }
                self.moved = after;
            }
            Some(ElseBranch::ElseIf(next)) => {
                let before = self.moved.clone();
                let mut after = before.clone();
                self.check_block(&mut ifs.then_body, true);
                for (k, v) in self.moved.drain() {
                    after.entry(k).or_insert(v);
                }
                self.moved = before;
                self.check_if(next);
                for (k, v) in self.moved.drain() {
                    after.entry(k).or_insert(v);
                }
                self.moved = after;
            }
        }
    }

    fn check_binding(&mut self, b: &mut Binding) {
        let mut annot_valid = true;
        if let (Some(ty), Some(span)) = (&b.ty, b.ty_span) {
            let t = ty.clone();
            self.check_declared_type(&t, span);
            if matches!(&t, Type::Named(n) if !self.structs.contains_key(n)) {
                // The annotation doesn't name a real type; fall back to the
                // value's own type to avoid a cascade of follow-up errors.
                annot_valid = false;
            }
        }
        let it = self.infer(&mut b.init);

        // `val a = b;` moves `b` when the type isn't a scalar (M2 model:
        // assignment moves). Borrowed parameters can't be moved at all.
        if let Expr::Ident(n, nspan) = &b.init {
            if let Some(info) = self.lookup(n) {
                if !info.ty.is_scalar() {
                    if matches!(
                        info.param_conv,
                        Some(AccessConvention::Read) | Some(AccessConvention::Mutate)
                    ) {
                        self.diags.push(Diagnostic::error(
                            "E0120",
                            format!("`{}` is only borrowed here, so it can't be moved", n),
                            "this function reads the value but doesn't own it".to_string(),
                            format!("copy it instead: `{} {} = {}.clone();`", if b.mutable { syntax::KW_VAR } else { syntax::KW_VAL }, b.name, n),
                            Some(*nspan),
                        ));
                    } else {
                        self.mark_moved(n.clone(), *nspan);
                    }
                }
            }
        }

        let final_ty = match (&b.ty, it) {
            (Some(_), Some(actual)) if !annot_valid => actual,
            (Some(annot), Some(actual)) => {
                if *annot != actual {
                    self.diags.push(Diagnostic::error(
                        "E0108",
                        format!(
                            "`{}` says it holds {}, but the value is {}",
                            b.name,
                            annot.show(),
                            actual.show()
                        ),
                        "the type written after `:` must match the value".to_string(),
                        type_fix_hint(annot, &actual),
                        Some(b.init.span()),
                    ));
                }
                annot.clone()
            }
            (Some(annot), None) => annot.clone(),
            (None, Some(actual)) => actual,
            (None, None) => Type::Int, // an error was already reported
        };
        self.declare(
            &b.name,
            b.name_span,
            LocalInfo {
                ty: final_ty,
                mutable: b.mutable,
                param_conv: None,
                decl_loop_depth: self.loop_depth,
            },
        );
    }

    // --- expressions ------------------------------------------------------

    fn require_bool(&mut self, e: &mut Expr, what: &str) {
        if let Some(t) = self.infer(e) {
            if t != Type::Bool {
                self.diags.push(Diagnostic::error(
                    "E0110",
                    format!("{} must be {}, but this is {}", what, Type::Bool.show(), t.show()),
                    "the program needs a clear yes or no here".to_string(),
                    "compare the value to something, e.g. `x > 0` or `name == \"ok\"`"
                        .to_string(),
                    Some(e.span()),
                ));
            }
        }
    }

    fn unknown_name(&mut self, name: &str, span: Span) {
        let mut fix = format!(
            "declare it first: `{} {} = ...;`",
            syntax::KW_VAL,
            name
        );
        let mut best: Option<(String, usize)> = None;
        let candidates: Vec<String> = self
            .scopes
            .iter()
            .flat_map(|s| s.keys().cloned())
            .chain(self.consts.keys().cloned())
            .collect();
        for cand in candidates {
            let d = edit_distance(name, &cand);
            if d <= 2 && best.as_ref().map_or(true, |(_, bd)| d < *bd) {
                best = Some((cand, d));
            }
        }
        if let Some((cand, _)) = best {
            fix = format!("did you mean `{}`?", cand);
        }
        self.diags.push(Diagnostic::error(
            "E0107",
            format!("nothing named `{}` exists here", name),
            "a name must be declared before it's used".to_string(),
            fix,
            Some(span),
        ));
    }

    /// Whether `e` may be returned through `-> view T` (reference-safe).
    fn expr_ok_for_view_return(&self, e: &Expr) -> bool {
        match e {
            Expr::Ident(name, _) => {
                if self.consts.contains_key(name) {
                    return true;
                }
                if let Some(info) = self.lookup(name) {
                    return info.ty.is_scalar() || info.param_conv.is_some();
                }
                false
            }
            _ => false,
        }
    }

    fn mark_moved(&mut self, name: String, span: Span) {
        if let Some(info) = self.lookup(&name) {
            if info.decl_loop_depth < self.loop_depth {
                self.diags.push(Diagnostic::error(
                    "E0121",
                    format!("`{}` is given away inside a loop that may run again", name),
                    "after a value is given away it's gone, but the next time around the loop would need it again".to_string(),
                    format!("give away a copy instead: `{}.clone()`", name),
                    Some(span),
                ));
                return;
            }
        }
        self.moved.insert(name, span);
    }

    /// `x = y;` / `val a = y;` / `return y;` where `y` is a plain name of a
    /// non-scalar type gives the value away (assignment moves, see C1).
    fn note_move_if_direct_ident(&mut self, e: &Expr) {
        if let Expr::Ident(n, span) = e {
            if let Some(info) = self.lookup(n) {
                if !info.ty.is_scalar() && info.param_conv.is_none() {
                    self.mark_moved(n.clone(), *span);
                }
            }
        }
    }

    fn infer_name_or(&mut self, e: &mut Expr, fallback: &str) -> String {
        self.infer(e).map(|t| t.name()).unwrap_or_else(|| fallback.to_string())
    }

    /// Infer and check an expression. Returns None when a problem was
    /// already reported (avoids error cascades).
    fn infer(&mut self, e: &mut Expr) -> Option<Type> {
        match e {
            Expr::Int(_, _) => Some(Type::Int),
            Expr::Float(_, _) => Some(Type::Float),
            Expr::Bool(_, _) => Some(Type::Bool),
            Expr::Str(parts, _) => {
                for p in parts.iter_mut() {
                    if let StrPart::Interp(inner) = p {
                        let t = self.infer(inner);
                        if let Some(t) = t {
                            if !matches!(
                                t,
                                Type::Int | Type::Float | Type::Bool | Type::String
                            ) {
                                self.diags.push(Diagnostic::error(
                                    "E0112",
                                    format!("{} can't be put into text yet", t.show()),
                                    "interpolation shows Int, Float, Bool, and String values"
                                        .to_string(),
                                    "show one of its parts instead".to_string(),
                                    Some(inner.span()),
                                ));
                            }
                        }
                    }
                }
                Some(Type::String)
            }
            Expr::Ident(name, span) => {
                if let Some(moved_at) = self.moved.get(name).copied() {
                    let (line_note, _) = (moved_at, ());
                    let _ = line_note;
                    self.diags.push(Diagnostic::error(
                        "E0121",
                        format!("`{}` was given away earlier, so it can't be used here", name),
                        "after a value moves somewhere else, the old name no longer holds it"
                            .to_string(),
                        format!("give away a copy instead (`{}.clone()`) where it moved", name),
                        Some(*span),
                    ));
                    self.moved.remove(name); // report once
                    return None;
                }
                if let Some(info) = self.lookup(name) {
                    return Some(info.ty.clone());
                }
                if let Some(t) = self.consts.get(name) {
                    return Some(t.clone());
                }
                self.unknown_name(name, *span);
                None
            }
            Expr::Call(call) => {
                let span = call.name_span;
                match self.check_call(call, true) {
                    Some(Some(t)) => Some(t),
                    Some(None) => {
                        self.diags.push(Diagnostic::error(
                            "E0116",
                            format!("`{}` doesn't hand back a value", call.name),
                            "only calls that declare `-> Type` can be used as a value"
                                .to_string(),
                            format!(
                                "call `{}` on its own line, or give it a return type",
                                call.name
                            ),
                            Some(span),
                        ));
                        None
                    }
                    None => None,
                }
            }
            Expr::Unary(op, inner, span) => {
                let t = self.infer(inner)?;
                match op {
                    UnOp::Neg => {
                        if matches!(t, Type::Int | Type::Float) {
                            Some(t)
                        } else {
                            self.diags.push(Diagnostic::error(
                                "E0109",
                                format!("`-` needs a number, but this is {}", t.show()),
                                "only Int and Float values can be negated".to_string(),
                                "use a number here".to_string(),
                                Some(*span),
                            ));
                            None
                        }
                    }
                    UnOp::Not => {
                        if t == Type::Bool {
                            Some(Type::Bool)
                        } else {
                            self.diags.push(Diagnostic::error(
                                "E0109",
                                format!("`!` needs {}, but this is {}", Type::Bool.show(), t.show()),
                                "`!` flips a yes to a no and back".to_string(),
                                "compare the value to something first, e.g. `!(x > 0)`"
                                    .to_string(),
                                Some(*span),
                            ));
                            None
                        }
                    }
                }
            }
            Expr::Binary(op, lhs, rhs, span) => {
                let (op, span) = (*op, *span);
                self.infer_binary(op, lhs, rhs, span)
            }
            Expr::Deref(inner, span) => {
                if !self.in_unsafe {
                    self.diags.push(Diagnostic::error(
                        "E0208",
                        "`*` isn't allowed here".to_string(),
                        "dereferencing with `*` is only for expert code inside `unsafe`"
                            .to_string(),
                        "remove `*`, or wrap this code in `unsafe { ... }`".to_string(),
                        Some(*span),
                    ));
                }
                self.infer(inner)
            }
            Expr::Member(inner, member, span) => {
                let t = self.infer(inner);
                if member == "clone" {
                    t
                } else {
                    self.diags.push(Diagnostic::error(
                        "E0117",
                        format!("`.{}` doesn't exist yet", member),
                        "fields and methods arrive in M3; today the only one is `.clone()`"
                            .to_string(),
                        format!("remove `.{}` for now", member),
                        Some(*span),
                    ));
                    None
                }
            }
        }
    }

    /// Binary operators, including comparison distribution (S25):
    /// `day == "mon" || "tue"` re-applies the nearest comparison.
    fn infer_binary(
        &mut self,
        op: BinOp,
        lhs: &mut Box<Expr>,
        rhs: &mut Box<Expr>,
        span: Span,
    ) -> Option<Type> {
        if matches!(op, BinOp::And | BinOp::Or) {
            let lt = self.infer(lhs);
            if let Some(lt) = lt {
                if lt != Type::Bool {
                    self.diags.push(Diagnostic::error(
                        "E0110",
                        format!(
                            "the left side of `{}` must be {}, but this is {}",
                            op.spell(),
                            Type::Bool.show(),
                            lt.show()
                        ),
                        "logic joins yes/no values".to_string(),
                        "compare the value to something first".to_string(),
                        Some(lhs.span()),
                    ));
                }
            }
            let rt = self.infer(rhs);
            if let Some(rt) = rt {
                if rt != Type::Bool {
                    // S25: a plain value re-applies the nearest comparison.
                    if let Some((subject, cmp_op)) = rightmost_comparison(lhs) {
                        let rhs_span = rhs.span();
                        let new_span = Span::new(subject.span().start, rhs_span.end);
                        let old_rhs = std::mem::replace(
                            rhs.as_mut(),
                            Expr::Bool(false, rhs_span),
                        );
                        **rhs = Expr::Binary(
                            cmp_op,
                            Box::new(subject),
                            Box::new(old_rhs),
                            new_span,
                        );
                        // Re-check the rebuilt comparison; this reports a
                        // mismatch (E0109) if the value's type doesn't fit.
                        self.infer_rebuilt(rhs);
                    } else {
                        self.diags.push(Diagnostic::error(
                            "E0110",
                            format!(
                                "the right side of `{}` must be {}, but this is {}",
                                op.spell(),
                                Type::Bool.show(),
                                rt.show()
                            ),
                            format!(
                                "right after a comparison, a plain value repeats it (`x == 1 {} 2` means `x == 1 {} x == 2`, S25) — but there's no comparison before this one",
                                op.spell(),
                                op.spell()
                            ),
                            "compare the value to something, e.g. `x == 2`".to_string(),
                            Some(rhs.span()),
                        ));
                    }
                }
            }
            return Some(Type::Bool);
        }

        let lt = self.infer(lhs);
        let rt = self.infer(rhs);
        let (lt, rt) = (lt?, rt?);

        match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
                if lt == rt && matches!(lt, Type::Int | Type::Float) {
                    Some(lt)
                } else if lt == Type::String && op == BinOp::Add {
                    self.diags.push(Diagnostic::error(
                        "E0109",
                        "text isn't joined with `+`".to_string(),
                        "there's one way to build text: interpolation (S8)".to_string(),
                        "write the pieces inside one string: \"{a}{b}\"".to_string(),
                        Some(span),
                    ));
                    None
                } else if (lt == Type::Int && rt == Type::Float)
                    || (lt == Type::Float && rt == Type::Int)
                {
                    self.diags.push(Diagnostic::error(
                        "E0109",
                        format!(
                            "`{}` can't mix {} and {}",
                            op.spell(),
                            lt.show(),
                            rt.show()
                        ),
                        "Lex never converts numbers silently; the two sides must match"
                            .to_string(),
                        "make both sides the same kind of number (write `2.0` instead of `2`, or drop the `.0`)".to_string(),
                        Some(span),
                    ));
                    None
                } else {
                    self.op_mismatch(op, &lt, &rt, span);
                    None
                }
            }
            BinOp::Rem | BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl
            | BinOp::Shr => {
                if lt == Type::Int && rt == Type::Int {
                    Some(Type::Int)
                } else {
                    self.diags.push(Diagnostic::error(
                        "E0109",
                        format!(
                            "`{}` works on {} only, but this has {} and {}",
                            op.spell(),
                            Type::Int.show(),
                            lt.show(),
                            rt.show()
                        ),
                        compound_why(op),
                        "use whole numbers here".to_string(),
                        Some(span),
                    ));
                    None
                }
            }
            BinOp::Eq | BinOp::Ne => {
                if lt == rt {
                    Some(Type::Bool)
                } else {
                    self.op_mismatch(op, &lt, &rt, span);
                    None
                }
            }
            BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                if lt == rt && matches!(lt, Type::Int | Type::Float) {
                    Some(Type::Bool)
                } else if lt == rt && lt == Type::String {
                    self.diags.push(Diagnostic::error(
                        "E0109",
                        format!("text isn't ordered with `{}`", op.spell()),
                        "comparing text for order isn't supported yet".to_string(),
                        "compare with `==` or `!=`, or compare lengths/numbers instead"
                            .to_string(),
                        Some(span),
                    ));
                    None
                } else {
                    self.op_mismatch(op, &lt, &rt, span);
                    None
                }
            }
            BinOp::And | BinOp::Or => unreachable!(),
        }
    }

    /// Re-infer a node we just built ourselves (S25); it can still report
    /// a type mismatch, but never duplicates earlier errors because both
    /// halves were already clean.
    fn infer_rebuilt(&mut self, e: &mut Expr) {
        self.infer(e);
    }

    fn op_mismatch(&mut self, op: BinOp, lt: &Type, rt: &Type, span: Span) {
        self.diags.push(Diagnostic::error(
            "E0109",
            format!(
                "`{}` can't compare or combine {} and {}",
                op.spell(),
                lt.show(),
                rt.show()
            ),
            "the two sides of an operator must be the same type".to_string(),
            "make both sides the same type".to_string(),
            Some(span),
        ));
    }

    // --- calls -----------------------------------------------------------

    /// Check a call. Returns:
    ///   None             — problem already reported
    ///   Some(None)       — fine, no value handed back
    ///   Some(Some(ty))   — fine, hands back `ty`
    fn check_call(&mut self, call: &mut Call, _as_value: bool) -> Option<Option<Type>> {
        if call.name == syntax::FOREIGN_PRINTLN {
            self.diags.push(Diagnostic::error(
                "E0011",
                format!(
                    "{} calls it `{}`, not `{}`",
                    syntax::LANG_NAME,
                    syntax::BUILTIN_PRINT,
                    syntax::FOREIGN_PRINTLN
                ),
                format!("`{}` already ends the line for you", syntax::BUILTIN_PRINT),
                format!(
                    "replace `{}` with `{}`",
                    syntax::FOREIGN_PRINTLN,
                    syntax::BUILTIN_PRINT
                ),
                Some(call.name_span),
            ));
            // Recover: treat it as print.
            for arg in call.args.iter_mut() {
                self.infer(&mut arg.expr);
            }
            return None;
        }

        if call.name == syntax::BUILTIN_PRINT {
            if call.args.len() != 1 {
                self.diags.push(Diagnostic::error(
                    "E0103",
                    format!("`{}` needs exactly one thing to print", syntax::BUILTIN_PRINT),
                    "printing nothing isn't meaningful".to_string(),
                    format!("e.g. {}(\"hello\")", syntax::BUILTIN_PRINT),
                    Some(call.name_span),
                ));
                for arg in call.args.iter_mut() {
                    self.infer(&mut arg.expr);
                }
                return None;
            }
            let arg = &mut call.args[0];
            if let Some(t) = self.infer(&mut arg.expr) {
                if !matches!(t, Type::Int | Type::Float | Type::Bool | Type::String) {
                    self.diags.push(Diagnostic::error(
                        "E0112",
                        format!("`{}` doesn't know how to show {}", syntax::BUILTIN_PRINT, t.show()),
                        "print shows Int, Float, Bool, and String values".to_string(),
                        "print one of its parts instead".to_string(),
                        Some(arg.expr.span()),
                    ));
                }
            }
            return Some(None);
        }

        let Some(sig) = self.funcs.get(&call.name).cloned() else {
            let mut fix = format!(
                "define it first ({} {}() {{ ... }}), or call one that exists",
                syntax::KW_FN,
                call.name
            );
            let mut best: Option<(&str, usize)> = None;
            for cand in self
                .funcs
                .keys()
                .map(|s| s.as_str())
                .chain([syntax::BUILTIN_PRINT])
            {
                let d = edit_distance(&call.name, cand);
                if d <= 2 && best.map_or(true, |(_, bd)| d < bd) {
                    best = Some((cand, d));
                }
            }
            if let Some((cand, _)) = best {
                fix = format!("did you mean `{}`?", cand);
            }
            self.diags.push(Diagnostic::error(
                "E0102",
                format!("nothing named `{}` exists here", call.name),
                format!(
                    "only functions that have been defined (or built in, like `{}`) can be called",
                    syntax::BUILTIN_PRINT
                ),
                fix,
                Some(call.name_span),
            ));
            for arg in call.args.iter_mut() {
                self.infer(&mut arg.expr);
            }
            return None;
        };

        if call.args.len() != sig.params.len() {
            self.diags.push(Diagnostic::error(
                "E0104",
                format!(
                    "`{}` expects {} argument{}, got {}",
                    call.name,
                    sig.params.len(),
                    if sig.params.len() == 1 { "" } else { "s" },
                    call.args.len()
                ),
                "every argument must match a parameter".to_string(),
                format!("check the definition of `{}`", call.name),
                Some(call.name_span),
            ));
        }

        let mut mut_borrowed: HashSet<String> = HashSet::new();
        let mut read_borrowed: HashSet<String> = HashSet::new();

        for (i, arg) in call.args.iter_mut().enumerate() {
            if let Expr::Ident(name, span) = &arg.expr {
                if mut_borrowed.contains(name) {
                    self.diags.push(aliasing_while_mut(name, *span));
                } else if arg.convention == AccessConvention::Mutate
                    && read_borrowed.contains(name)
                {
                    self.diags.push(aliasing_mut_after_read(name, *span));
                }
            }
            let arg_ty = self.infer(&mut arg.expr);
            let Some((param_conv, param_ty)) = sig.params.get(i) else {
                continue;
            };

            if let Some(arg_ty) = &arg_ty {
                if arg_ty != param_ty && !matches!(param_ty, Type::Named(_)) {
                    self.diags.push(Diagnostic::error(
                        "E0112",
                        format!(
                            "`{}` wants {} for argument {}, but this is {}",
                            call.name,
                            param_ty.show(),
                            i + 1,
                            arg_ty.show()
                        ),
                        "every argument must match its parameter's type".to_string(),
                        type_fix_hint(param_ty, arg_ty),
                        Some(arg.expr.span()),
                    ));
                }
            }

            match (param_conv, arg.convention) {
                (AccessConvention::Move, AccessConvention::Read) => {
                    if let Expr::Ident(name, span) = &arg.expr {
                        if is_cloneable(param_ty, self.structs) {
                            arg.flags.implicit_clone = true;
                            self.diags.push(Diagnostic::lint(
                                "L0201",
                                format!(
                                    "implicit clone of `{}`; write `{} {}` to transfer ownership or `.clone()` to silence this warning",
                                    name,
                                    syntax::KW_MOVE,
                                    name
                                ),
                                format!(
                                    "`{}` expects to take ownership of this value",
                                    call.name
                                ),
                                format!(
                                    "write `{} {}` to move, or `{} .clone()` to copy explicitly",
                                    syntax::KW_MOVE,
                                    name,
                                    name
                                ),
                                Some(*span),
                            ));
                        } else {
                            self.diags.push(Diagnostic::error(
                                "E0201",
                                format!(
                                    "`{}` needs `{}` here — this value can't be copied",
                                    call.name,
                                    syntax::KW_MOVE
                                ),
                                format!(
                                    "parameter `{}` takes ownership; passing `{}` without `{}` would have to copy it, but this type can't be copied",
                                    i + 1,
                                    name,
                                    syntax::KW_MOVE
                                ),
                                format!(
                                    "write `{} {}` to transfer ownership",
                                    syntax::KW_MOVE,
                                    name
                                ),
                                Some(*span),
                            ));
                        }
                    }
                }
                (AccessConvention::Move, AccessConvention::Move) => {
                    // The value is given away for real.
                    if let Expr::Ident(name, span) = &arg.expr {
                        if !param_ty.is_scalar() {
                            self.mark_moved(name.clone(), *span);
                        }
                    }
                }
                (AccessConvention::Mutate, AccessConvention::Read) => {
                    if let Expr::Ident(name, span) = &arg.expr {
                        self.diags.push(Diagnostic::error(
                            "E0202",
                            format!(
                                "parameter `{}` requires `{}` at the call site",
                                name,
                                syntax::KW_MUTATE
                            ),
                            format!(
                                "`{}` needs to change this value while it borrows it",
                                call.name
                            ),
                            format!(
                                "write `{} {}` when calling `{}`",
                                syntax::KW_MUTATE,
                                name,
                                call.name
                            ),
                            Some(*span),
                        ));
                    }
                }
                (AccessConvention::Mutate, AccessConvention::Mutate) => {
                    // `mut x` at the call site: x itself must be changeable.
                    if let Expr::Ident(name, span) = &arg.expr {
                        if let Some(info) = self.lookup(name) {
                            if !info.mutable {
                                self.diags.push(Diagnostic::error(
                                    "E0111",
                                    format!(
                                        "`{}` was made with `{}`, so it can't be changed",
                                        name,
                                        syntax::KW_VAL
                                    ),
                                    format!(
                                        "`{}` will change this value, so it must be a `{}`",
                                        call.name,
                                        syntax::KW_VAR
                                    ),
                                    format!("declare it with `{} {} = ...`", syntax::KW_VAR, name),
                                    Some(*span),
                                ));
                            }
                        }
                    }
                }
                (
                    AccessConvention::Read | AccessConvention::Mutate,
                    AccessConvention::Move,
                ) => {
                    self.diags.push(Diagnostic::error(
                        "E0203",
                        format!(
                            "`{}` passed to a parameter that does not consume",
                            syntax::KW_MOVE
                        ),
                        "only `take` parameters accept a moved value at the call site"
                            .to_string(),
                        format!(
                            "remove `{}` or change the parameter to `take`",
                            syntax::KW_MOVE
                        ),
                        Some(arg.span),
                    ));
                }
                _ => {}
            }

            if arg.convention == AccessConvention::Mutate {
                if let Expr::Ident(name, _) = &arg.expr {
                    mut_borrowed.insert(name.clone());
                }
            }
            if let (Some((param_conv, param_ty)), Expr::Ident(name, _)) =
                (sig.params.get(i), &arg.expr)
            {
                if matches!(param_conv, AccessConvention::Read)
                    && arg.convention == AccessConvention::Read
                    && !param_ty.is_scalar()
                {
                    read_borrowed.insert(name.clone());
                }
            }

            if self.loop_depth > 0 {
                if let Expr::Ident(name, span) = &arg.expr {
                    if let Some(info) = self.lookup(name) {
                        if matches!(info.ty, Type::Shared(_)) {
                            arg.flags.shared_auto_clone = true;
                            self.diags.push(Diagnostic::lint(
                                "L0202",
                                format!(
                                    "auto-cloned `{}` inside a loop; consider hoisting or caching",
                                    name
                                ),
                                "shared handles are cloned when used across a loop boundary"
                                    .to_string(),
                                format!(
                                    "hoist `{}` before the loop, or clone once outside",
                                    name
                                ),
                                Some(*span),
                            ));
                        }
                    }
                }
            }
        }

        Some(sig.return_type.clone())
    }
}

/// Find the comparison that distribution (S25) should re-apply: descend the
/// right spine of `&&`/`||` chains; clone the comparison's left side.
fn rightmost_comparison(e: &Expr) -> Option<(Expr, BinOp)> {
    match e {
        Expr::Binary(op, _, rhs, _) if matches!(op, BinOp::And | BinOp::Or) => {
            rightmost_comparison(rhs)
        }
        Expr::Binary(op, lhs, _, _) if op.is_comparison() => Some(((**lhs).clone(), *op)),
        _ => None,
    }
}

fn compound_why(op: BinOp) -> String {
    match op {
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
            "`+ - * /` work on Int and Float".to_string()
        }
        _ => format!("`{}` is a whole-number operation (Int only)", op.spell()),
    }
}

fn type_fix_hint(want: &Type, got: &Type) -> String {
    match (want, got) {
        (Type::Float, Type::Int) => "write the number with a decimal part, like `2.0`".to_string(),
        (Type::Int, Type::Float) => "drop the decimal part, like `2`".to_string(),
        (Type::String, _) => "put the value in text with interpolation: \"{x}\"".to_string(),
        _ => format!("use {} here", want.show()),
    }
}

fn aliasing_while_mut(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "E0204",
        format!(
            "`{}` is being changed in this call, so it can't be used again here",
            name
        ),
        "while something is being changed, nobody else may be looking at it"
            .to_string(),
        format!(
            "pass `{} {}` only once, or copy first with `{} .clone()`",
            syntax::KW_MUTATE,
            name,
            name
        ),
        Some(span),
    )
}

fn aliasing_mut_after_read(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "E0204",
        format!(
            "`{}` is already shared in this call, so it can't be changed here too",
            name
        ),
        "while something is being looked at, nobody else may be changing it"
            .to_string(),
        format!(
            "drop the extra use of `{}`, or copy first with `{} .clone()`",
            name,
            name
        ),
        Some(span),
    )
}

fn loop_control_outside(kw: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "E0115",
        format!("`{}` only works inside a loop", kw),
        format!(
            "`{}` and `{}` steer the nearest `{}` or `{}` loop",
            syntax::KW_BREAK,
            syntax::KW_CONTINUE,
            syntax::KW_WHILE,
            syntax::KW_FOR
        ),
        "move this inside a loop, or remove it".to_string(),
        Some(span),
    )
}

/// Does this block definitely hit a `return` on every path?
fn block_definitely_returns(stmts: &[Stmt]) -> bool {
    stmts.iter().any(stmt_definitely_returns)
}

fn stmt_definitely_returns(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(_, _) => true,
        Stmt::If(ifs) => if_definitely_returns(ifs),
        Stmt::Switch {
            arms, else_body, ..
        } => {
            arms.iter().all(|a| block_definitely_returns(&a.body))
                && block_definitely_returns(else_body)
        }
        _ => false,
    }
}

fn if_definitely_returns(ifs: &IfStmt) -> bool {
    if !block_definitely_returns(&ifs.then_body) {
        return false;
    }
    match &ifs.else_branch {
        Some(ElseBranch::Else(b)) => block_definitely_returns(b),
        Some(ElseBranch::ElseIf(next)) => if_definitely_returns(next),
        None => false,
    }
}

fn is_cloneable(ty: &Type, structs: &HashMap<String, Vec<(Option<String>, Type)>>) -> bool {
    match ty {
        Type::Int | Type::Bool | Type::Float | Type::String => true,
        Type::List(inner) | Type::Shared(inner) => is_cloneable(inner, structs),
        Type::Named(name) => {
            // Built-in cloneable; user structs without Clone are not.
            name != "NoClone"
        }
    }
}

fn walk_stmts_for_const_refs(
    stmts: &[Stmt],
    const_names: &[String],
    taken: &mut HashSet<String>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::Expr(e) => walk_expr_for_const_refs(e, const_names, taken),
            Stmt::Val(b) => walk_expr_for_const_refs(&b.init, const_names, taken),
            Stmt::Assign { value, .. } => walk_expr_for_const_refs(value, const_names, taken),
            Stmt::Return(Some(e), _) => walk_expr_for_const_refs(e, const_names, taken),
            Stmt::Return(None, _) => {}
            Stmt::If(ifs) => walk_if_for_const_refs(ifs, const_names, taken),
            Stmt::While { cond, body, .. } => {
                walk_expr_for_const_refs(cond, const_names, taken);
                walk_stmts_for_const_refs(body, const_names, taken);
            }
            Stmt::For {
                start, end, body, ..
            } => {
                walk_expr_for_const_refs(start, const_names, taken);
                walk_expr_for_const_refs(end, const_names, taken);
                walk_stmts_for_const_refs(body, const_names, taken);
            }
            Stmt::Switch {
                subject,
                arms,
                else_body,
                ..
            } => {
                walk_expr_for_const_refs(subject, const_names, taken);
                for a in arms {
                    walk_expr_for_const_refs(&a.cond, const_names, taken);
                    walk_stmts_for_const_refs(&a.body, const_names, taken);
                }
                walk_stmts_for_const_refs(else_body, const_names, taken);
            }
            Stmt::Break(_) | Stmt::Continue(_) => {}
            Stmt::Loop(inner, _) | Stmt::Unsafe(inner, _) => {
                walk_stmts_for_const_refs(inner, const_names, taken);
            }
        }
    }
}

fn walk_if_for_const_refs(ifs: &IfStmt, const_names: &[String], taken: &mut HashSet<String>) {
    walk_expr_for_const_refs(&ifs.cond, const_names, taken);
    walk_stmts_for_const_refs(&ifs.then_body, const_names, taken);
    match &ifs.else_branch {
        Some(ElseBranch::Else(b)) => walk_stmts_for_const_refs(b, const_names, taken),
        Some(ElseBranch::ElseIf(next)) => walk_if_for_const_refs(next, const_names, taken),
        None => {}
    }
}

fn walk_expr_for_const_refs(expr: &Expr, const_names: &[String], taken: &mut HashSet<String>) {
    match expr {
        Expr::Ident(name, _) => {
            if const_names.iter().any(|c| c == name) {
                taken.insert(name.clone());
            }
        }
        Expr::Str(parts, _) => {
            for p in parts {
                if let StrPart::Interp(e) = p {
                    walk_expr_for_const_refs(e, const_names, taken);
                }
            }
        }
        Expr::Call(c) => {
            for a in &c.args {
                walk_expr_for_const_refs(&a.expr, const_names, taken);
            }
        }
        Expr::Unary(_, inner, _) | Expr::Deref(inner, _) | Expr::Member(inner, _, _) => {
            walk_expr_for_const_refs(inner, const_names, taken)
        }
        Expr::Binary(_, l, r, _) => {
            walk_expr_for_const_refs(l, const_names, taken);
            walk_expr_for_const_refs(r, const_names, taken);
        }
        Expr::Int(_, _) | Expr::Float(_, _) | Expr::Bool(_, _) => {}
    }
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i];
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur.push((prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost));
        }
        prev = cur;
    }
    prev[b.len()]
}
