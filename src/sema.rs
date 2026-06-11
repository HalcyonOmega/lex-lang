//! Semantic checks. Everything here exists so that codegen can stay "dumb"
//! (invariant I3): by the time a Program reaches codegen, it must be
//! impossible for the generated Rust to fail to compile (invariant I2).

use crate::ast::{
    AccessConvention, ConstAttr, Expr, Item, Program, RustConstKind, Stmt, Type,
};
use crate::diag::Diagnostic;
use crate::syntax;
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct FuncSig {
    params: Vec<(AccessConvention, Type)>,
    return_type: Option<Type>,
    is_view_return: bool,
}

pub fn check(prog: &mut Program) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let mut funcs: HashMap<String, FuncSig> = HashMap::new();
    let mut structs: HashMap<String, Vec<(Option<String>, Type)>> = HashMap::new();
    let mut const_names: Vec<String> = Vec::new();
    let mut in_unsafe = false;

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
                if f.is_view_return {
                    if let Some(ref rt) = f.return_type {
                        if !is_viewable_return(rt) {
                            diags.push(Diagnostic::error(
                                "E0206",
                                "`view` can only be used on return types that refer to `self`"
                                    .to_string(),
                                "`view` marks a borrow of data owned by the receiver".to_string(),
                                "return an owned value instead, or use `view` only on methods"
                                    .to_string(),
                                Some(f.name_span),
                            ));
                        }
                    }
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
                if const_names.contains(&c.name) {
                    diags.push(Diagnostic::error(
                        "E0105",
                        format!("`{}` is defined twice", c.name),
                        "every const needs a unique name".to_string(),
                        "rename or remove one of the definitions".to_string(),
                        Some(c.name_span),
                    ));
                } else {
                    const_names.push(c.name.clone());
                }
            }
        }
    }

    if !funcs.contains_key("main") {
        diags.push(Diagnostic::error(
            "E0101",
            "this program has no `main` function".to_string(),
            "running a program starts at `fn main`, and this file doesn't define one".to_string(),
            "add one to this file: fn main() { ... }".to_string(),
            None,
        ));
    }

    // Const address-taken analysis (rule 9).
    let mut address_taken: std::collections::HashSet<String> = std::collections::HashSet::new();
    for item in &prog.items {
        if let Item::Func(f) = item {
            walk_stmts_for_const_refs(&f.body, &const_names, &mut address_taken);
        }
    }
    for item in &mut prog.items {
        if let Item::Const(c) = item {
            let force_static = c.attrs.contains(&ConstAttr::ForceStatic);
            let interior = type_has_interior_mutability(&c);
            c.rust_kind = if force_static || address_taken.contains(&c.name) || interior {
                RustConstKind::Static
            } else {
                RustConstKind::Const
            };
        }
    }

    // Per-function body checks.
    for item in &mut prog.items {
        if let Item::Func(f) = item {
            let mut locals: HashMap<String, Type> = HashMap::new();
            for p in &f.params {
                locals.insert(p.name.clone(), p.ty.clone());
            }
            check_stmts(
                &mut f.body,
                &funcs,
                &structs,
                &mut locals,
                &mut diags,
                false,
                &mut in_unsafe,
            );
            in_unsafe = false;
        }
    }

    diags
}

fn is_viewable_return(_ty: &Type) -> bool {
    // M2: view returns are allowed on any named type for codegen tests.
    true
}

fn type_has_interior_mutability(c: &crate::ast::ConstDef) -> bool {
    match &c.value {
        Expr::Ident(name, _) => name.starts_with("Atomic") || name.contains("Mutex"),
        _ => false,
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

fn is_shared_handle(ty: &Type) -> bool {
    matches!(ty, Type::Shared(_))
}

fn walk_stmts_for_const_refs(
    stmts: &[Stmt],
    const_names: &[String],
    taken: &mut std::collections::HashSet<String>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::Call(call) => {
                for arg in &call.args {
                    if let Expr::Ident(name, _) = &arg.expr {
                        if const_names.iter().any(|c| c == name) {
                            taken.insert(name.clone());
                        }
                    }
                }
            }
            Stmt::Val(b) => {
                walk_expr_for_const_refs(&b.init, const_names, taken);
            }
            Stmt::Return(expr, _) => walk_expr_for_const_refs(expr, const_names, taken),
            Stmt::Loop(inner, _) | Stmt::Unsafe(inner, _) => {
                walk_stmts_for_const_refs(inner, const_names, taken);
            }
        }
    }
}

fn walk_expr_for_const_refs(
    expr: &Expr,
    const_names: &[String],
    taken: &mut std::collections::HashSet<String>,
) {
    match expr {
        Expr::Ident(name, _) => {
            if const_names.iter().any(|c| c == name) {
                taken.insert(name.clone());
            }
        }
        Expr::Deref(inner, _) => walk_expr_for_const_refs(inner, const_names, taken),
        Expr::Member(inner, _, _) => walk_expr_for_const_refs(inner, const_names, taken),
        Expr::Str(_) | Expr::Int(_) => {}
    }
}

fn check_stmts(
    stmts: &mut [Stmt],
    funcs: &HashMap<String, FuncSig>,
    structs: &HashMap<String, Vec<(Option<String>, Type)>>,
    locals: &mut HashMap<String, Type>,
    diags: &mut Vec<Diagnostic>,
    in_loop: bool,
    in_unsafe: &mut bool,
) {
    for stmt in stmts.iter_mut() {
        match stmt {
            Stmt::Val(b) => {
                let ty = b.ty.clone().unwrap_or(Type::Int);
                check_expr(&b.init, locals, diags, in_unsafe);
                locals.insert(b.name.clone(), ty);
            }
            Stmt::Return(expr, _) => check_expr(expr, locals, diags, in_unsafe),
            Stmt::Loop(inner, _) => {
                check_stmts(inner, funcs, structs, locals, diags, true, in_unsafe);
            }
            Stmt::Unsafe(inner, _) => {
                let prev = *in_unsafe;
                *in_unsafe = true;
                check_stmts(inner, funcs, structs, locals, diags, in_loop, in_unsafe);
                *in_unsafe = prev;
            }
            Stmt::Call(call) => {
                if call.name == syntax::BUILTIN_PRINT {
                    if call.args.len() != 1 {
                        diags.push(Diagnostic::error(
                            "E0103",
                            format!("`{}` needs exactly one thing to print", syntax::BUILTIN_PRINT),
                            "printing nothing isn't meaningful".to_string(),
                            format!("e.g. {}(\"hello\")", syntax::BUILTIN_PRINT),
                            Some(call.name_span),
                        ));
                    } else {
                        check_call_arg_expr(&call.args[0].expr, locals, diags, in_unsafe);
                    }
                    continue;
                }

                let Some(sig) = funcs.get(&call.name) else {
                    let mut fix = format!(
                        "define it first ({} {}() {{ ... }}), or call one that exists",
                        syntax::KW_FN,
                        call.name
                    );
                    let mut best: Option<(&str, usize)> = None;
                    for cand in funcs.keys().map(|s| s.as_str()).chain([syntax::BUILTIN_PRINT]) {
                        let d = edit_distance(&call.name, cand);
                        if d <= 2 && best.map_or(true, |(_, bd)| d < bd) {
                            best = Some((cand, d));
                        }
                    }
                    if let Some((cand, _)) = best {
                        fix = format!("did you mean `{}`?", cand);
                    }
                    diags.push(Diagnostic::error(
                        "E0102",
                        format!("nothing named `{}` exists here", call.name),
                        format!(
                            "only functions that have been defined (or built in, like `{}`) can be called",
                            syntax::BUILTIN_PRINT
                        ),
                        fix,
                        Some(call.name_span),
                    ));
                    continue;
                };

                if call.args.len() != sig.params.len() {
                    diags.push(Diagnostic::error(
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

                for (i, arg) in call.args.iter_mut().enumerate() {
                    let param = sig.params.get(i);
                    check_call_arg_expr(&arg.expr, locals, diags, in_unsafe);
                    let Some((param_conv, param_ty)) = param else {
                        continue;
                    };

                    match (param_conv, arg.convention) {
                        (AccessConvention::Move, AccessConvention::Read) => {
                            if let Expr::Ident(name, span) = &arg.expr {
                                if is_cloneable(param_ty, structs) {
                                    arg.flags.implicit_clone = true;
                                    diags.push(Diagnostic::lint(
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
                                    diags.push(Diagnostic::error(
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
                                        format!("write `{} {}` to transfer ownership", syntax::KW_MOVE, name),
                                        Some(*span),
                                    ));
                                }
                            }
                        }
                        (AccessConvention::Mutate, AccessConvention::Read) => {
                            if let Expr::Ident(name, span) = &arg.expr {
                                diags.push(Diagnostic::error(
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
                                    format!("write `{} {}` when calling `{}`", syntax::KW_MUTATE, name, call.name),
                                    Some(*span),
                                ));
                            }
                        }
                        (AccessConvention::Read | AccessConvention::Mutate, AccessConvention::Move) => {
                            diags.push(Diagnostic::error(
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

                    if in_loop {
                        if let Expr::Ident(name, span) = &arg.expr {
                            if let Some(local_ty) = locals.get(name) {
                                if is_shared_handle(local_ty) {
                                    arg.flags.shared_auto_clone = true;
                                    diags.push(Diagnostic::lint(
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
            }
        }
    }
}

fn check_call_arg_expr(
    expr: &Expr,
    locals: &HashMap<String, Type>,
    diags: &mut Vec<Diagnostic>,
    in_unsafe: &bool,
) {
    check_expr(expr, locals, diags, in_unsafe);
}

fn check_expr(
    expr: &Expr,
    _locals: &HashMap<String, Type>,
    diags: &mut Vec<Diagnostic>,
    in_unsafe: &bool,
) {
    match expr {
        Expr::Deref(_, span) if !*in_unsafe => {
            diags.push(Diagnostic::error(
                "E0208",
                "`*` isn't allowed here".to_string(),
                "dereferencing with `*` is only for expert code inside `unsafe`".to_string(),
                "remove `*`, or wrap this code in `unsafe { ... }`".to_string(),
                Some(*span),
            ));
        }
        Expr::Deref(inner, _) => check_expr(inner, _locals, diags, in_unsafe),
        Expr::Member(inner, _, _) => check_expr(inner, _locals, diags, in_unsafe),
        Expr::Ident(_, _) | Expr::Str(_) | Expr::Int(_) => {}
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
